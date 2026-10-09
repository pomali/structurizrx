//! Cluster analysis: structure, communities and conformance (the review
//! counterpart that looks at the model as a whole rather than element by
//! element).
//!
//! The pipeline is always the same three steps:
//!
//! 1. **Project.** Pick the elements that are nodes ([`Level`]), optionally
//!    roll relationships between their descendants up onto them, and weight
//!    each edge by how many underlying relationships it stands for.
//! 2. **Analyse.** Structural facts (weakly connected components, cycles via
//!    strongly connected components, articulation points, bridges), per-node
//!    metrics (coupling, instability, PageRank, betweenness) and communities
//!    (Louvain).
//! 3. **Compare.** Check the detected communities against the structure the
//!    model *declares* — `group`, or the parent element when no group is set.
//!    This is the output worth reading: everything above is input to it.
//!
//! # Determinism
//!
//! Every result is reproducible run to run. Nodes are in
//! [`crate::index::Index`] order, adjacency is built from `Vec`s rather than
//! `HashMap` iteration, and Louvain visits nodes in that same fixed order and
//! breaks ties by lowest community index — so it needs no random seed at all,
//! rather than needing a fixed one.

use std::collections::{BTreeMap, HashMap, HashSet};

use petgraph::algo::{
    articulation_points::articulation_points, bridges, connected_components, page_rank, tarjan_scc,
};
use petgraph::graph::{DiGraph, NodeIndex, UnGraph};
use petgraph::visit::EdgeRef;
use serde::Serialize;
use structurizr_model::Workspace;

use crate::index::{build_index, Index};

/// Which elements become nodes of the analysed graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    SoftwareSystem,
    Container,
    Component,
}

impl Level {
    /// The element kind this level selects, also used as its stable name in
    /// API parameters and cache keys.
    pub fn kind_name(self) -> &'static str {
        self.kind()
    }

    fn kind(self) -> &'static str {
        match self {
            Level::SoftwareSystem => "softwareSystem",
            Level::Container => "container",
            Level::Component => "component",
        }
    }

    /// Parse a level name; unknown names fall back to containers, the level
    /// most analyses want.
    pub fn parse(value: &str) -> Level {
        match value {
            "softwareSystem" | "system" | "systems" => Level::SoftwareSystem,
            "component" | "components" => Level::Component,
            _ => Level::Container,
        }
    }
}

/// What to analyse and how.
#[derive(Debug, Clone)]
pub struct ClusterOptions {
    pub level: Level,
    /// Keep only elements carrying at least one of these tags (empty: keep all).
    pub include_tags: Vec<String>,
    /// Drop elements carrying any of these tags.
    pub exclude_tags: Vec<String>,
    /// Roll relationships between descendants up onto their ancestor at the
    /// analysed level. Without this, a container-level analysis sees only
    /// relationships declared container-to-container and misses everything
    /// modelled between components.
    pub implied: bool,
}

impl Default for ClusterOptions {
    fn default() -> Self {
        ClusterOptions {
            level: Level::Container,
            include_tags: Vec::new(),
            exclude_tags: Vec::new(),
            implied: true,
        }
    }
}

/// One analysed element.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterNode {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    /// The partition the model declares for this element: its `group`, or its
    /// parent's name when no group is set.
    pub declared: Option<String>,
    pub parent_name: Option<String>,
    /// Index into [`ClusterAnalysis::communities`].
    pub community: usize,
    /// Relationships arriving from other analysed nodes.
    pub afferent: usize,
    /// Relationships leaving towards other analysed nodes.
    pub efferent: usize,
    /// `efferent / (afferent + efferent)`: 0 is maximally depended upon, 1
    /// depends only outwards. `None` when the node has no edges at all.
    pub instability: Option<f64>,
    pub page_rank: f64,
    /// Fraction of shortest paths between other nodes that run through this
    /// one. High values are chokepoints.
    pub betweenness: f64,
    pub is_articulation_point: bool,
}

/// One edge of the projected graph.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterEdge {
    pub source_id: String,
    pub target_id: String,
    /// How many model relationships this edge stands for.
    pub weight: usize,
    /// True when no relationship was declared directly between these two
    /// elements — the edge exists because their descendants are related.
    pub implied: bool,
}

/// A detected community.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Community {
    pub id: usize,
    pub member_ids: Vec<String>,
    /// The declared partition most of its members belong to, if any.
    pub dominant_declared: Option<String>,
    /// Members whose declared partition is the dominant one.
    pub agreeing: usize,
}

/// An element the detected structure and the declared structure disagree about.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConformanceFinding {
    pub element_id: String,
    pub element_name: String,
    pub declared: String,
    /// What the rest of its community declares.
    pub clusters_with: String,
    pub community: usize,
    pub message: String,
}

/// A cycle: a strongly connected component with more than one member.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cycle {
    pub member_ids: Vec<String>,
    pub member_names: Vec<String>,
}

/// A single point of failure: an edge whose removal disconnects the graph.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bridge {
    pub source_id: String,
    pub target_id: String,
    pub source_name: String,
    pub target_name: String,
}

/// The whole analysis.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClusterAnalysis {
    pub level: String,
    pub nodes: Vec<ClusterNode>,
    pub edges: Vec<ClusterEdge>,
    pub communities: Vec<Community>,
    /// Newman modularity of the detected partition, in [-0.5, 1]. Higher means
    /// the communities are more cleanly separated; below about 0.3 the split
    /// is weak and its findings deserve less weight.
    pub modularity: f64,
    /// Count of weakly connected components — how many disconnected pieces the
    /// model falls into.
    pub components: usize,
    pub cycles: Vec<Cycle>,
    pub articulation_point_ids: Vec<String>,
    pub bridges: Vec<Bridge>,
    pub conformance: Vec<ConformanceFinding>,
}

/// Run the analysis.
pub fn cluster(workspace: &Workspace, options: &ClusterOptions) -> ClusterAnalysis {
    let idx = build_index(workspace);
    let members = select_nodes(&idx, options);

    if members.is_empty() {
        return ClusterAnalysis {
            level: options.level.kind().to_string(),
            ..Default::default()
        };
    }

    let position: HashMap<&str, usize> = members
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();

    let edges = project_edges(&idx, options, &position);

    // Two petgraph views over the same node numbering: direction matters for
    // cycles and coupling, but articulation points, bridges and communities
    // are undirected questions.
    let mut directed: DiGraph<(), f64> = DiGraph::new();
    let mut undirected: UnGraph<(), f64> = UnGraph::new_undirected();
    for _ in &members {
        directed.add_node(());
        undirected.add_node(());
    }
    for e in &edges {
        let s = NodeIndex::new(position[e.source_id.as_str()]);
        let t = NodeIndex::new(position[e.target_id.as_str()]);
        directed.add_edge(s, t, e.weight as f64);
        if s != t {
            undirected.add_edge(s, t, e.weight as f64);
        }
    }

    let (community_of, communities) = louvain(&undirected, members.len());
    let modularity = modularity(&undirected, &community_of);

    let ranks = page_rank(&directed, 0.85_f64, 50);
    let betweenness = betweenness(&directed, members.len());
    // petgraph returns its own hashbrown set; collect into std's so the rest
    // of this module stays on one set type.
    let cut_vertices: HashSet<NodeIndex> = articulation_points(&undirected).into_iter().collect();

    let nodes = members
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let entry = idx.element(id).expect("selected ids come from the index");
            let afferent = edges.iter().filter(|e| e.target_id == *id).count();
            let efferent = edges.iter().filter(|e| e.source_id == *id).count();
            let total = afferent + efferent;

            ClusterNode {
                id: id.clone(),
                name: entry.name.clone(),
                kind: entry.kind,
                declared: declared_partition(&idx, id),
                parent_name: entry
                    .parent_id
                    .as_ref()
                    .and_then(|p| idx.element(p))
                    .map(|p| p.name.clone()),
                community: community_of[i],
                afferent,
                efferent,
                instability: if total == 0 {
                    None
                } else {
                    Some(efferent as f64 / total as f64)
                },
                page_rank: ranks[i],
                betweenness: betweenness[i],
                is_articulation_point: cut_vertices.contains(&NodeIndex::new(i)),
            }
        })
        .collect::<Vec<_>>();

    let cycles = tarjan_scc(&directed)
        .into_iter()
        .filter(|group| group.len() > 1)
        .map(|group| {
            let mut ids: Vec<String> = group.iter().map(|n| members[n.index()].clone()).collect();
            ids.sort();
            Cycle {
                member_names: ids
                    .iter()
                    .map(|id| idx.element(id).map(|e| e.name.clone()).unwrap_or_default())
                    .collect(),
                member_ids: ids,
            }
        })
        .collect();

    let mut bridge_list: Vec<Bridge> = bridges(&undirected)
        .map(|e| {
            let (s, t) = (
                members[e.source().index()].clone(),
                members[e.target().index()].clone(),
            );
            Bridge {
                source_name: idx.element(&s).map(|x| x.name.clone()).unwrap_or_default(),
                target_name: idx.element(&t).map(|x| x.name.clone()).unwrap_or_default(),
                source_id: s,
                target_id: t,
            }
        })
        .collect();
    bridge_list.sort_by(|a, b| (&a.source_id, &a.target_id).cmp(&(&b.source_id, &b.target_id)));

    let mut articulation_ids: Vec<String> = cut_vertices
        .iter()
        .map(|n| members[n.index()].clone())
        .collect();
    articulation_ids.sort();

    let communities = summarise_communities(communities, &nodes);
    let conformance = conformance(&nodes, &communities);

    ClusterAnalysis {
        level: options.level.kind().to_string(),
        components: connected_components(&undirected),
        nodes,
        edges,
        communities,
        modularity,
        cycles,
        articulation_point_ids: articulation_ids,
        bridges: bridge_list,
        conformance,
    }
}

/// The partition the model declares: an explicit `group`, else the parent.
fn declared_partition(idx: &Index, id: &str) -> Option<String> {
    let entry = idx.element(id)?;
    if let Some(group) = entry.group.as_ref().filter(|g| !g.trim().is_empty()) {
        return Some(group.clone());
    }
    entry
        .parent_id
        .as_ref()
        .and_then(|p| idx.element(p))
        .map(|p| p.name.clone())
}

/// Elements at the requested level that survive the tag filters, in index order.
fn select_nodes(idx: &Index, options: &ClusterOptions) -> Vec<String> {
    idx.elements
        .iter()
        .filter(|e| e.kind == options.level.kind())
        .filter(|e| {
            options.include_tags.is_empty()
                || options.include_tags.iter().any(|t| e.tags.contains(t))
        })
        .filter(|e| !options.exclude_tags.iter().any(|t| e.tags.contains(t)))
        .map(|e| e.id.clone())
        .collect()
}

/// Collapse model relationships onto the analysed nodes.
///
/// With `implied`, a relationship is attributed to the nearest ancestor of each
/// end that is itself an analysed node, which is what lets a container-level
/// view see dependencies that were only ever modelled between components.
/// Self-edges (both ends inside the same node) are dropped: they say nothing
/// about coupling between nodes.
fn project_edges(
    idx: &Index,
    options: &ClusterOptions,
    position: &HashMap<&str, usize>,
) -> Vec<ClusterEdge> {
    let resolve = |id: &str| -> Option<String> {
        if position.contains_key(id) {
            return Some(id.to_string());
        }
        if !options.implied {
            return None;
        }
        let entry = idx.element(id)?;
        entry
            .ancestors
            .iter()
            .find(|a| position.contains_key(a.as_str()))
            .cloned()
    };

    // Keyed by (source, target) so parallel relationships become one weighted
    // edge; BTreeMap keeps the output order stable.
    let mut collapsed: BTreeMap<(usize, usize), (String, String, usize, bool)> = BTreeMap::new();

    for r in &idx.relationships {
        let (Some(source), Some(target)) = (resolve(&r.source_id), resolve(&r.dest_id)) else {
            continue;
        };
        if source == target {
            continue;
        }
        let direct = r.source_id == source && r.dest_id == target;
        let key = (position[source.as_str()], position[target.as_str()]);
        let entry = collapsed.entry(key).or_insert((source, target, 0, true));
        entry.2 += 1;
        entry.3 &= !direct;
    }

    collapsed
        .into_values()
        .map(|(source_id, target_id, weight, implied)| ClusterEdge {
            source_id,
            target_id,
            weight,
            implied,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Communities
// ---------------------------------------------------------------------------

/// Louvain community detection on the undirected weighted graph.
///
/// Deterministic by construction: nodes are visited in index order and ties are
/// broken towards the lowest community index, so no seed is involved and two
/// runs over the same workspace always agree.
fn louvain(graph: &UnGraph<(), f64>, node_count: usize) -> (Vec<usize>, Vec<Vec<usize>>) {
    // Adjacency as (neighbour, weight), plus each node's weighted degree.
    let mut adjacency: Vec<Vec<(usize, f64)>> = vec![Vec::new(); node_count];
    let mut self_loops = vec![0.0; node_count];
    for e in graph.edge_references() {
        let (s, t, w) = (e.source().index(), e.target().index(), *e.weight());
        if s == t {
            self_loops[s] += w;
        } else {
            adjacency[s].push((t, w));
            adjacency[t].push((s, w));
        }
    }

    let mut community: Vec<usize> = (0..node_count).collect();
    let total_weight: f64 = adjacency
        .iter()
        .flat_map(|n| n.iter().map(|(_, w)| *w))
        .sum::<f64>()
        / 2.0
        + self_loops.iter().sum::<f64>();

    if total_weight <= 0.0 {
        // No edges: every node is its own community.
        let groups = (0..node_count).map(|i| vec![i]).collect();
        return (community, groups);
    }

    let m2 = 2.0 * total_weight;
    let degree: Vec<f64> = (0..node_count)
        .map(|i| adjacency[i].iter().map(|(_, w)| *w).sum::<f64>() + 2.0 * self_loops[i])
        .collect();
    let mut community_degree: Vec<f64> = degree.clone();

    // Local moving. Repeat sweeps until a full pass moves nothing.
    loop {
        let mut moved = false;

        for node in 0..node_count {
            let current = community[node];
            community_degree[current] -= degree[node];

            // Weight from this node into each candidate community.
            let mut links: BTreeMap<usize, f64> = BTreeMap::new();
            links.entry(current).or_insert(0.0);
            for (neighbour, weight) in &adjacency[node] {
                *links.entry(community[*neighbour]).or_insert(0.0) += weight;
            }

            let mut best = current;
            let mut best_gain = f64::NEG_INFINITY;
            for (&candidate, &link_weight) in &links {
                // Modularity gain of placing `node` in `candidate`, up to the
                // constant factors shared by every candidate.
                let gain = link_weight - community_degree[candidate] * degree[node] / m2;
                if gain > best_gain {
                    best_gain = gain;
                    best = candidate;
                }
            }

            community_degree[best] += degree[node];
            if best != current {
                community[node] = best;
                moved = true;
            }
        }

        if !moved {
            break;
        }
    }

    // Renumber communities to 0..n in order of first appearance, and collect
    // their members.
    let mut renumbered: BTreeMap<usize, usize> = BTreeMap::new();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (node, slot) in community.iter_mut().enumerate() {
        let id = *renumbered.entry(*slot).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        *slot = id;
        groups[id].push(node);
    }

    (community, groups)
}

/// Newman modularity of a partition on the undirected weighted graph.
fn modularity(graph: &UnGraph<(), f64>, community: &[usize]) -> f64 {
    let mut total = 0.0;
    let mut degree = vec![0.0; community.len()];
    let mut internal: BTreeMap<usize, f64> = BTreeMap::new();
    let mut community_degree: BTreeMap<usize, f64> = BTreeMap::new();

    for e in graph.edge_references() {
        let (s, t, w) = (e.source().index(), e.target().index(), *e.weight());
        total += w;
        degree[s] += w;
        degree[t] += w;
        if community[s] == community[t] {
            *internal.entry(community[s]).or_insert(0.0) += w;
        }
    }

    if total <= 0.0 {
        return 0.0;
    }

    for (node, &c) in community.iter().enumerate() {
        *community_degree.entry(c).or_insert(0.0) += degree[node];
    }

    community_degree
        .iter()
        .map(|(c, &d)| {
            let inside = internal.get(c).copied().unwrap_or(0.0);
            inside / total - (d / (2.0 * total)).powi(2)
        })
        .sum()
}

// ---------------------------------------------------------------------------
// Betweenness
// ---------------------------------------------------------------------------

/// Brandes' betweenness centrality on the unweighted directed graph,
/// normalised by the number of ordered pairs so values are comparable between
/// workspaces of different sizes.
fn betweenness(graph: &DiGraph<(), f64>, node_count: usize) -> Vec<f64> {
    let mut score = vec![0.0; node_count];
    if node_count < 3 {
        return score;
    }

    let neighbours: Vec<Vec<usize>> = (0..node_count)
        .map(|i| {
            let mut out: Vec<usize> = graph
                .edges(NodeIndex::new(i))
                .map(|e| e.target().index())
                .collect();
            out.sort_unstable();
            out.dedup();
            out
        })
        .collect();

    for source in 0..node_count {
        let mut stack: Vec<usize> = Vec::new();
        let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); node_count];
        let mut paths = vec![0.0; node_count];
        let mut distance = vec![-1_i64; node_count];
        let mut queue = std::collections::VecDeque::new();

        paths[source] = 1.0;
        distance[source] = 0;
        queue.push_back(source);

        while let Some(v) = queue.pop_front() {
            stack.push(v);
            for &w in &neighbours[v] {
                if distance[w] < 0 {
                    distance[w] = distance[v] + 1;
                    queue.push_back(w);
                }
                if distance[w] == distance[v] + 1 {
                    paths[w] += paths[v];
                    predecessors[w].push(v);
                }
            }
        }

        let mut dependency = vec![0.0; node_count];
        while let Some(w) = stack.pop() {
            for &v in &predecessors[w] {
                dependency[v] += (paths[v] / paths[w]) * (1.0 + dependency[w]);
            }
            if w != source {
                score[w] += dependency[w];
            }
        }
    }

    let pairs = ((node_count - 1) * (node_count - 2)) as f64;
    for s in &mut score {
        *s /= pairs;
    }
    score
}

// ---------------------------------------------------------------------------
// Conformance
// ---------------------------------------------------------------------------

fn summarise_communities(groups: Vec<Vec<usize>>, nodes: &[ClusterNode]) -> Vec<Community> {
    groups
        .into_iter()
        .enumerate()
        .map(|(id, members)| {
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for &m in &members {
                if let Some(declared) = nodes[m].declared.as_deref() {
                    *counts.entry(declared).or_insert(0) += 1;
                }
            }
            // Ties break towards the alphabetically first name, so the answer
            // does not depend on iteration order.
            let dominant = counts
                .iter()
                .max_by_key(|(name, count)| (**count, std::cmp::Reverse(*name)))
                .map(|(name, count)| ((*name).to_string(), *count));

            Community {
                id,
                member_ids: members.iter().map(|&m| nodes[m].id.clone()).collect(),
                agreeing: dominant.as_ref().map(|(_, c)| *c).unwrap_or(0),
                dominant_declared: dominant.map(|(n, _)| n),
            }
        })
        .collect()
}

/// Elements whose declared partition disagrees with the company they keep.
///
/// Only reported where the community actually has a majority declaration to
/// disagree with: a community with no dominant group says nothing useful.
fn conformance(nodes: &[ClusterNode], communities: &[Community]) -> Vec<ConformanceFinding> {
    let mut findings = Vec::new();

    for node in nodes {
        let Some(declared) = node.declared.as_deref() else {
            continue;
        };
        let community = &communities[node.community];
        let Some(dominant) = community.dominant_declared.as_deref() else {
            continue;
        };
        if dominant == declared || community.member_ids.len() < 2 {
            continue;
        }
        // A community split evenly between declarations has no majority worth
        // reporting against.
        if community.agreeing * 2 <= community.member_ids.len() {
            continue;
        }

        findings.push(ConformanceFinding {
            element_id: node.id.clone(),
            element_name: node.name.clone(),
            declared: declared.to_string(),
            clusters_with: dominant.to_string(),
            community: node.community,
            message: format!(
                "'{}' is declared in '{}' but clusters with '{}'",
                node.name, declared, dominant
            ),
        });
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use structurizr_model::{Component, Container, Model, Relationship, SoftwareSystem, Workspace};

    fn container(id: &str, name: &str, group: Option<&str>, rels: Vec<(&str, &str)>) -> Container {
        Container {
            id: id.into(),
            name: name.into(),
            group: group.map(|g| g.to_string()),
            relationships: Some(
                rels.into_iter()
                    .map(|(rid, dest)| Relationship {
                        id: rid.into(),
                        source_id: id.into(),
                        destination_id: dest.into(),
                        ..Default::default()
                    })
                    .collect(),
            ),
            ..Default::default()
        }
    }

    /// Two triangles joined by a single edge: an unambiguous two-community
    /// graph, where the joining edge is also the only bridge.
    fn barbell() -> Workspace {
        let mut ws = Workspace {
            name: "T".into(),
            ..Default::default()
        };
        ws.model = Model {
            software_systems: Some(vec![SoftwareSystem {
                id: "1".into(),
                name: "Sys".into(),
                containers: Some(vec![
                    container("a", "A", Some("left"), vec![("r1", "b"), ("r2", "c")]),
                    container("b", "B", Some("left"), vec![("r3", "c")]),
                    container("c", "C", Some("left"), vec![("r4", "d")]),
                    container("d", "D", Some("right"), vec![("r5", "e"), ("r6", "f")]),
                    container("e", "E", Some("right"), vec![("r7", "f")]),
                    container("f", "F", Some("right"), vec![]),
                ]),
                ..Default::default()
            }]),
            ..Default::default()
        };
        ws
    }

    fn community_of<'a>(analysis: &'a ClusterAnalysis, id: &str) -> usize {
        analysis
            .nodes
            .iter()
            .find(|n| n.id == id)
            .unwrap()
            .community
    }

    #[test]
    fn communities_follow_the_dense_parts_of_the_graph() {
        let a = cluster(&barbell(), &ClusterOptions::default());

        assert_eq!(a.nodes.len(), 6);
        for pair in [("a", "b"), ("b", "c"), ("d", "e"), ("e", "f")] {
            assert_eq!(
                community_of(&a, pair.0),
                community_of(&a, pair.1),
                "{} and {} belong together",
                pair.0,
                pair.1
            );
        }
        assert_ne!(community_of(&a, "a"), community_of(&a, "f"));
        assert!(a.modularity > 0.3, "modularity was {}", a.modularity);
    }

    #[test]
    fn repeated_runs_produce_identical_results() {
        let ws = barbell();
        let a = cluster(&ws, &ClusterOptions::default());
        let b = cluster(&ws, &ClusterOptions::default());

        let signature = |x: &ClusterAnalysis| -> Vec<(String, usize)> {
            x.nodes
                .iter()
                .map(|n| (n.id.clone(), n.community))
                .collect()
        };
        assert_eq!(signature(&a), signature(&b));
        assert_eq!(a.modularity, b.modularity);
    }

    #[test]
    fn the_joining_edge_is_a_bridge_and_its_ends_are_cut_vertices() {
        let a = cluster(&barbell(), &ClusterOptions::default());

        assert_eq!(a.bridges.len(), 1);
        assert_eq!(a.bridges[0].source_id, "c");
        assert_eq!(a.bridges[0].target_id, "d");
        assert!(a.articulation_point_ids.contains(&"c".to_string()));
        assert!(a.articulation_point_ids.contains(&"d".to_string()));
        assert_eq!(a.components, 1);
    }

    #[test]
    fn cycles_are_reported_as_strongly_connected_components() {
        let mut ws = barbell();
        // Close a loop: f -> a makes the whole chain strongly connected.
        if let Some(systems) = ws.model.software_systems.as_mut() {
            let containers = systems[0].containers.as_mut().unwrap();
            containers[5].relationships = Some(vec![Relationship {
                id: "r8".into(),
                source_id: "f".into(),
                destination_id: "a".into(),
                ..Default::default()
            }]);
        }

        let a = cluster(&ws, &ClusterOptions::default());
        assert_eq!(a.cycles.len(), 1);
        assert_eq!(a.cycles[0].member_ids.len(), 6);
    }

    #[test]
    fn a_chain_has_no_cycles() {
        let a = cluster(&barbell(), &ClusterOptions::default());
        assert!(a.cycles.is_empty());
    }

    #[test]
    fn coupling_and_instability_follow_edge_direction() {
        let a = cluster(&barbell(), &ClusterOptions::default());

        let node = |id: &str| a.nodes.iter().find(|n| n.id == id).unwrap();
        // A only ever points outwards.
        assert_eq!(node("a").afferent, 0);
        assert_eq!(node("a").efferent, 2);
        assert_eq!(node("a").instability, Some(1.0));
        // F is only ever pointed at.
        assert_eq!(node("f").efferent, 0);
        assert_eq!(node("f").instability, Some(0.0));
    }

    #[test]
    fn conformance_reports_an_element_that_sits_with_the_wrong_group() {
        let mut ws = barbell();
        // Declare C as part of "right" while it stays wired into the left triangle.
        if let Some(systems) = ws.model.software_systems.as_mut() {
            let containers = systems[0].containers.as_mut().unwrap();
            containers[2].group = Some("right".into());
        }

        let a = cluster(&ws, &ClusterOptions::default());
        let finding = a
            .conformance
            .iter()
            .find(|f| f.element_id == "c")
            .expect("C's declaration disagrees with its community");
        assert_eq!(finding.declared, "right");
        assert_eq!(finding.clusters_with, "left");
    }

    #[test]
    fn a_model_that_agrees_with_itself_reports_nothing() {
        let a = cluster(&barbell(), &ClusterOptions::default());
        assert!(a.conformance.is_empty(), "{:?}", a.conformance);
    }

    #[test]
    fn implied_edges_roll_component_relationships_up_to_containers() {
        let mut ws = Workspace {
            name: "T".into(),
            ..Default::default()
        };
        ws.model = Model {
            software_systems: Some(vec![SoftwareSystem {
                id: "1".into(),
                name: "Sys".into(),
                containers: Some(vec![
                    Container {
                        id: "a".into(),
                        name: "A".into(),
                        components: Some(vec![Component {
                            id: "a1".into(),
                            name: "A1".into(),
                            relationships: Some(vec![Relationship {
                                id: "r1".into(),
                                source_id: "a1".into(),
                                destination_id: "b1".into(),
                                ..Default::default()
                            }]),
                            ..Default::default()
                        }]),
                        ..Default::default()
                    },
                    Container {
                        id: "b".into(),
                        name: "B".into(),
                        components: Some(vec![Component {
                            id: "b1".into(),
                            name: "B1".into(),
                            ..Default::default()
                        }]),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            }]),
            ..Default::default()
        };

        let rolled = cluster(&ws, &ClusterOptions::default());
        assert_eq!(rolled.edges.len(), 1);
        assert_eq!(rolled.edges[0].source_id, "a");
        assert_eq!(rolled.edges[0].target_id, "b");
        assert!(
            rolled.edges[0].implied,
            "no container-to-container relationship was declared"
        );

        let direct_only = cluster(
            &ws,
            &ClusterOptions {
                implied: false,
                ..Default::default()
            },
        );
        assert!(direct_only.edges.is_empty());
    }

    #[test]
    fn parallel_relationships_collapse_into_one_weighted_edge() {
        let mut ws = Workspace {
            name: "T".into(),
            ..Default::default()
        };
        ws.model = Model {
            software_systems: Some(vec![SoftwareSystem {
                id: "1".into(),
                name: "Sys".into(),
                containers: Some(vec![
                    container("a", "A", None, vec![("r1", "b"), ("r2", "b")]),
                    container("b", "B", None, vec![]),
                ]),
                ..Default::default()
            }]),
            ..Default::default()
        };

        let a = cluster(&ws, &ClusterOptions::default());
        assert_eq!(a.edges.len(), 1);
        assert_eq!(a.edges[0].weight, 2);
    }

    #[test]
    fn tag_filters_select_the_analysed_subset() {
        let mut ws = barbell();
        if let Some(systems) = ws.model.software_systems.as_mut() {
            let containers = systems[0].containers.as_mut().unwrap();
            containers[0].tags = Some("Element,Container,Legacy".into());
        }

        let excluded = cluster(
            &ws,
            &ClusterOptions {
                exclude_tags: vec!["Legacy".into()],
                ..Default::default()
            },
        );
        assert!(!excluded.nodes.iter().any(|n| n.id == "a"));

        let included = cluster(
            &ws,
            &ClusterOptions {
                include_tags: vec!["Legacy".into()],
                ..Default::default()
            },
        );
        assert_eq!(included.nodes.len(), 1);
    }

    #[test]
    fn an_empty_selection_yields_an_empty_analysis_rather_than_panicking() {
        let ws = Workspace {
            name: "T".into(),
            ..Default::default()
        };
        let a = cluster(&ws, &ClusterOptions::default());
        assert!(a.nodes.is_empty());
        assert_eq!(a.components, 0);
        assert_eq!(a.modularity, 0.0);
    }

    #[test]
    fn betweenness_peaks_on_the_node_every_path_crosses() {
        // A -> B -> C: B is on the only path between the other two.
        let mut ws = Workspace {
            name: "T".into(),
            ..Default::default()
        };
        ws.model = Model {
            software_systems: Some(vec![SoftwareSystem {
                id: "1".into(),
                name: "Sys".into(),
                containers: Some(vec![
                    container("a", "A", None, vec![("r1", "b")]),
                    container("b", "B", None, vec![("r2", "c")]),
                    container("c", "C", None, vec![]),
                ]),
                ..Default::default()
            }]),
            ..Default::default()
        };

        let a = cluster(&ws, &ClusterOptions::default());
        let node = |id: &str| a.nodes.iter().find(|n| n.id == id).unwrap();
        assert!(node("b").betweenness > 0.0);
        assert_eq!(node("a").betweenness, 0.0);
        assert_eq!(node("c").betweenness, 0.0);
    }
}
