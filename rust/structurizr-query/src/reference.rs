//! References to model items by name, and the viewer links that carry them.
//!
//! A reference names a thing the way a reader sees it — by canonical path
//! (ancestor names then its own, joined with `/`, as in [`crate::diff`]) rather
//! than by id. The DSL parser assigns ids in parse order, so an id shifts as
//! soon as anything is declared before it; a path survives that, which is what
//! lets a link copied from the viewer still mean the same element after the
//! workspace has been edited.
//!
//! Grammar (paths match case-insensitively):
//!
//! - `Shop/API` — an element. Deployment elements are rooted at their
//!   environment: `Production/Server/API` for an instance of `API`.
//! - `Shop/API.http` — a port.
//! - `Shop/API->Shop/DB` — every relationship between the two ends;
//!   `Shop/API->Shop/DB "reads orders"` narrows by description. When nothing
//!   joins the ends directly but relationships between their descendants do,
//!   the reference resolves to those — the implied relationship a view draws.
//! - `view:<key>` — a view.
//! - `decision:<id>` — an imported architecture decision.
//!
//! Viewer links (`http://host/workspace/<slug>#<view>&sel=<ref>,<ref>`) carry
//! references percent-encoded in their hash; see [`parse_viewer_link`].

use std::collections::HashMap;

use structurizr_model::{DeploymentNode, Port, Relationship, Workspace};

/// A parsed reference; see the module docs for the grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reference {
    Element(String),
    Relationship {
        from: String,
        to: String,
        description: Option<String>,
    },
    View(String),
    Decision(String),
}

/// Parse a reference. Anything that is not a view, decision or relationship
/// reference is an element path, so this never fails; an unknown path is
/// reported when it is resolved.
pub fn parse_reference(reference: &str) -> Reference {
    let reference = reference.trim();
    if let Some(key) = strip_prefix_ignore_case(reference, "view:") {
        return Reference::View(key.trim().to_string());
    }
    if let Some(id) = strip_prefix_ignore_case(reference, "decision:") {
        return Reference::Decision(id.trim().to_string());
    }
    if let Some((from, rest)) = reference.split_once("->") {
        let rest = rest.trim();
        // `to "description"`: the last quoted string, if the reference ends in one.
        let described = rest
            .strip_suffix('"')
            .and_then(|r| r.rfind('"').map(|i| (r[..i].trim(), &r[i + 1..])))
            .filter(|(to, _)| !to.is_empty());
        let (to, description) = match described {
            Some((to, description)) => (to, Some(description.to_string())),
            None => (rest, None),
        };
        return Reference::Relationship {
            from: from.trim().to_string(),
            to: to.to_string(),
            description,
        };
    }
    Reference::Element(reference.to_string())
}

fn strip_prefix_ignore_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    s.get(..prefix.len())
        .filter(|head| head.eq_ignore_ascii_case(prefix))
        .map(|_| &s[prefix.len()..])
}

/// What a viewer link points at.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViewerLink {
    /// The workspace slug from `/workspace/<slug>`.
    pub workspace: Option<String>,
    /// The view the link opens: the workspace page's `#<key>`, or the diagram
    /// page's `/diagram/<key>`.
    pub view: Option<String>,
    /// Decoded references from `sel=`, in link order.
    pub selection: Vec<String>,
}

/// Parse a viewer link — an absolute URL, a `/workspace/...` path, or a bare
/// `#...` hash. Returns `None` for anything else, so a caller can treat its
/// input as a plain reference instead. (`#` alone does not make a link: element
/// names such as `C# Service` contain one.)
pub fn parse_viewer_link(link: &str) -> Option<ViewerLink> {
    let link = link.trim();
    let (before_hash, hash) = link.split_once('#').unwrap_or((link, ""));
    let path = match before_hash.find("://") {
        Some(i) => {
            let after_scheme = &before_hash[i + 3..];
            after_scheme.find('/').map_or("", |j| &after_scheme[j..])
        }
        None if before_hash.starts_with("/workspace/") => before_hash,
        None if before_hash.is_empty() && link.starts_with('#') => "",
        None => return None,
    };
    let path = path.split('?').next().unwrap_or("");

    let mut out = ViewerLink::default();
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if let Some(i) = segments.iter().position(|s| *s == "workspace") {
        out.workspace = segments.get(i + 1).map(|s| percent_decode(s));
        if segments.get(i + 2) == Some(&"diagram") {
            out.view = segments.get(i + 3).map(|s| percent_decode(s));
        }
    }
    for (i, part) in hash.split('&').enumerate() {
        if let Some(selection) = part.strip_prefix("sel=") {
            out.selection.extend(
                selection
                    .split(',')
                    .filter(|r| !r.is_empty())
                    .map(percent_decode),
            );
        } else if i == 0 && !part.is_empty() && !part.contains('=') {
            out.view = Some(percent_decode(part));
        }
    }
    Some(out)
}

/// Decode `%XX` escapes (as produced by `encodeURIComponent`); malformed
/// escapes are kept literally.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A relationship a reference resolved to, with its ends as paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipMatch {
    pub id: String,
    /// Source path, with `.port` when the relationship leaves a port.
    pub from: String,
    /// Destination path, with `.port` when it arrives at one.
    pub to: String,
    pub description: Option<String>,
    /// Set on relationships the parser replicated onto deployment instances:
    /// the declared relationship this one copies.
    pub linked_relationship_id: Option<String>,
}

/// One thing a reference resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Element {
        id: String,
        path: String,
        /// "person", "softwareSystem", "container", "component", "custom",
        /// "deploymentNode", "infrastructureNode", "containerInstance" or
        /// "softwareSystemInstance".
        kind: &'static str,
    },
    Port {
        element_id: String,
        port_id: String,
        path: String,
    },
    Relationship(RelationshipMatch),
    /// No relationship joins `from` and `to`, but these relationships between
    /// their descendants do.
    Implied {
        from: String,
        to: String,
        via: Vec<RelationshipMatch>,
    },
    View {
        key: String,
    },
    Decision {
        id: String,
        element_id: Option<String>,
        title: String,
    },
}

impl Target {
    /// Short kind label: the element kind, or "port", "relationship",
    /// "implied", "view", "decision".
    pub fn kind(&self) -> &'static str {
        match self {
            Target::Element { kind, .. } => kind,
            Target::Port { .. } => "port",
            Target::Relationship(_) => "relationship",
            Target::Implied { .. } => "implied",
            Target::View { .. } => "view",
            Target::Decision { .. } => "decision",
        }
    }
}

/// What kind of thing a reference failed to find.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissKind {
    /// No element or port with that path (also an unknown relationship end).
    Element,
    /// Both ends exist but nothing joins them (with that description).
    Relationship,
    View,
    Decision,
}

/// A reference that resolved to nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Miss {
    pub kind: MissKind,
    /// The part that was not found: a path, a `from->to` pair, a description,
    /// a view key or a decision id.
    pub name: String,
    /// What `name` could have been, for a did-you-mean suggestion: every path,
    /// the descriptions between the two ends, every view key or decision id.
    pub candidates: Vec<String>,
}

struct Entry {
    id: String,
    name: String,
    path: String,
    kind: &'static str,
    parent: Option<usize>,
    /// (port id, port name)
    ports: Vec<(String, String)>,
}

/// A relationship end: an element, optionally narrowed to one of its ports.
#[derive(Clone, Copy)]
struct End {
    entry: usize,
    port: Option<usize>,
}

/// Every addressable item in a workspace, by path. Covers the static model and
/// the deployment model (unlike [`crate::index`]), and every relationship
/// including those on deployment elements.
pub struct Catalog<'a> {
    entries: Vec<Entry>,
    by_id: HashMap<String, usize>,
    /// Lowercase path → entries (instances of one element in one node share a
    /// path).
    by_path: HashMap<String, Vec<usize>>,
    relationships: Vec<&'a Relationship>,
    view_keys: Vec<String>,
    decisions: Vec<(String, Option<String>, String)>,
}

impl<'a> Catalog<'a> {
    pub fn new(workspace: &'a Workspace) -> Self {
        let mut catalog = Catalog {
            entries: Vec::new(),
            by_id: HashMap::new(),
            by_path: HashMap::new(),
            relationships: Vec::new(),
            view_keys: Vec::new(),
            decisions: Vec::new(),
        };
        let model = &workspace.model;
        for p in model.people.iter().flatten() {
            catalog.add(&p.id, &p.name, "person", None, &p.ports, &p.relationships);
        }
        for s in model.software_systems.iter().flatten() {
            let system = catalog.add(&s.id, &s.name, "softwareSystem", None, &s.ports, &s.relationships);
            for c in s.containers.iter().flatten() {
                let container =
                    catalog.add(&c.id, &c.name, "container", Some(system), &c.ports, &c.relationships);
                for comp in c.components.iter().flatten() {
                    catalog.add(&comp.id, &comp.name, "component", Some(container), &comp.ports, &comp.relationships);
                }
            }
        }
        for e in model.custom_elements.iter().flatten() {
            catalog.add(&e.id, &e.name, "custom", None, &e.ports, &e.relationships);
        }
        // After the static model, so instances can take their element's name.
        for node in model.deployment_nodes.iter().flatten() {
            catalog.add_deployment_node(node, None);
        }

        let views = &workspace.views;
        let keys = views.system_landscape_views.iter().flatten().map(|v| &v.key)
            .chain(views.system_context_views.iter().flatten().map(|v| &v.key))
            .chain(views.container_views.iter().flatten().map(|v| &v.key))
            .chain(views.component_views.iter().flatten().map(|v| &v.key))
            .chain(views.dynamic_views.iter().flatten().map(|v| &v.key))
            .chain(views.deployment_views.iter().flatten().map(|v| &v.key))
            .chain(views.filtered_views.iter().flatten().map(|v| &v.key));
        catalog.view_keys = keys.flatten().cloned().collect();

        for d in workspace.documentation.iter().flat_map(|doc| doc.decisions.iter().flatten()) {
            catalog.decisions.push((d.id.clone(), d.element_id.clone(), d.title.clone()));
        }
        catalog
    }

    fn add(
        &mut self,
        id: &str,
        name: &str,
        kind: &'static str,
        parent: Option<usize>,
        ports: &Option<Vec<Port>>,
        relationships: &'a Option<Vec<Relationship>>,
    ) -> usize {
        let path = match parent {
            Some(p) => format!("{}/{}", self.entries[p].path, name),
            None => name.to_string(),
        };
        self.add_at(id, name, path, kind, parent, ports, relationships)
    }

    #[allow(clippy::too_many_arguments)]
    fn add_at(
        &mut self,
        id: &str,
        name: &str,
        path: String,
        kind: &'static str,
        parent: Option<usize>,
        ports: &Option<Vec<Port>>,
        relationships: &'a Option<Vec<Relationship>>,
    ) -> usize {
        let i = self.entries.len();
        self.by_id.insert(id.to_string(), i);
        self.by_path.entry(path.to_lowercase()).or_default().push(i);
        self.entries.push(Entry {
            id: id.to_string(),
            name: name.to_string(),
            path,
            kind,
            parent,
            ports: ports.iter().flatten().map(|p| (p.id.clone(), p.name.clone())).collect(),
        });
        self.relationships.extend(relationships.iter().flatten());
        i
    }

    fn add_deployment_node(&mut self, node: &'a DeploymentNode, parent: Option<usize>) {
        let i = match parent {
            Some(_) => self.add(&node.id, &node.name, "deploymentNode", parent, &None, &node.relationships),
            None => {
                let environment = node.environment.as_deref().unwrap_or("Default");
                let path = format!("{}/{}", environment, node.name);
                self.add_at(&node.id, &node.name, path, "deploymentNode", None, &None, &node.relationships)
            }
        };
        for inf in node.infrastructure_nodes.iter().flatten() {
            self.add(&inf.id, &inf.name, "infrastructureNode", Some(i), &None, &inf.relationships);
        }
        for ci in node.container_instances.iter().flatten() {
            let name = self.name_of(&ci.container_id);
            self.add(&ci.id, &name, "containerInstance", Some(i), &None, &ci.relationships);
        }
        for si in node.software_system_instances.iter().flatten() {
            let name = self.name_of(&si.software_system_id);
            self.add(&si.id, &name, "softwareSystemInstance", Some(i), &None, &si.relationships);
        }
        for child in node.children.iter().flatten() {
            self.add_deployment_node(child, Some(i));
        }
    }

    fn name_of(&self, id: &str) -> String {
        self.by_id.get(id).map_or_else(|| id.to_string(), |&i| self.entries[i].name.clone())
    }

    /// The canonical path of element `id`, or the id itself when the catalog
    /// has no such element.
    pub fn path_of(&self, id: &str) -> String {
        self.by_id.get(id).map_or_else(|| id.to_string(), |&i| self.entries[i].path.clone())
    }

    /// Every element and port path, in model order.
    pub fn paths(&self) -> Vec<String> {
        let mut out = Vec::new();
        for e in &self.entries {
            out.push(e.path.clone());
            out.extend(e.ports.iter().map(|(_, port)| format!("{}.{}", e.path, port)));
        }
        out
    }

    /// Resolve a reference to everything it names, in model order.
    pub fn resolve(&self, reference: &Reference) -> Result<Vec<Target>, Miss> {
        match reference {
            Reference::Element(path) => {
                let ends = self.ends(path)?;
                Ok(ends.into_iter().map(|end| self.end_target(end)).collect())
            }
            Reference::Relationship { from, to, description } => {
                self.resolve_relationship(from, to, description.as_deref())
            }
            Reference::View(key) => {
                let exact: Vec<&String> = self.view_keys.iter().filter(|k| *k == key).collect();
                let found = if exact.is_empty() {
                    self.view_keys.iter().filter(|k| k.eq_ignore_ascii_case(key)).collect()
                } else {
                    exact
                };
                if found.is_empty() {
                    return Err(Miss { kind: MissKind::View, name: key.clone(), candidates: self.view_keys.clone() });
                }
                Ok(found.into_iter().map(|k| Target::View { key: k.clone() }).collect())
            }
            Reference::Decision(id) => {
                let found: Vec<Target> = self
                    .decisions
                    .iter()
                    .filter(|(d, _, _)| d == id)
                    .map(|(id, element_id, title)| Target::Decision {
                        id: id.clone(),
                        element_id: element_id.clone(),
                        title: title.clone(),
                    })
                    .collect();
                if found.is_empty() {
                    let candidates = self.decisions.iter().map(|(d, _, _)| d.clone()).collect();
                    return Err(Miss { kind: MissKind::Decision, name: id.clone(), candidates });
                }
                Ok(found)
            }
        }
    }

    /// The elements (or ports) at `path`: an element path first, else
    /// `element.port`. Split at the last dot only when no element matches the
    /// whole path, since element names routinely contain dots.
    fn ends(&self, path: &str) -> Result<Vec<End>, Miss> {
        let key = path.trim().to_lowercase();
        let found: Vec<End> = match self.by_path.get(&key) {
            Some(entries) => entries.iter().map(|&entry| End { entry, port: None }).collect(),
            None => key
                .rsplit_once('.')
                .and_then(|(element, port)| {
                    self.by_path.get(element).map(|entries| (entries, port))
                })
                .map(|(entries, port)| {
                    entries
                        .iter()
                        .filter_map(|&entry| {
                            self.entries[entry]
                                .ports
                                .iter()
                                .position(|(_, name)| name.to_lowercase() == port)
                                .map(|p| End { entry, port: Some(p) })
                        })
                        .collect()
                })
                .unwrap_or_default(),
        };
        if found.is_empty() {
            return Err(Miss { kind: MissKind::Element, name: path.trim().to_string(), candidates: self.paths() });
        }
        Ok(found)
    }

    fn end_path(&self, end: End) -> String {
        let e = &self.entries[end.entry];
        match end.port {
            Some(p) => format!("{}.{}", e.path, e.ports[p].1),
            None => e.path.clone(),
        }
    }

    fn end_target(&self, end: End) -> Target {
        let e = &self.entries[end.entry];
        match end.port {
            Some(p) => Target::Port {
                element_id: e.id.clone(),
                port_id: e.ports[p].0.clone(),
                path: self.end_path(end),
            },
            None => Target::Element { id: e.id.clone(), path: e.path.clone(), kind: e.kind },
        }
    }

    fn resolve_relationship(&self, from: &str, to: &str, description: Option<&str>) -> Result<Vec<Target>, Miss> {
        let froms = self.ends(from)?;
        let tos = self.ends(to)?;
        let described = |r: &Relationship| {
            description.is_none_or(|d| {
                r.description.as_deref().is_some_and(|rd| rd.trim().eq_ignore_ascii_case(d.trim()))
            })
        };

        let between: Vec<&Relationship> = self
            .relationships
            .iter()
            .copied()
            .filter(|r| {
                self.at_end(&froms, &r.source_id, r.source_port_id.as_deref())
                    && self.at_end(&tos, &r.destination_id, r.destination_port_id.as_deref())
            })
            .collect();
        if !between.is_empty() {
            let matched: Vec<Target> = between
                .iter()
                .filter(|r| described(r))
                .map(|r| Target::Relationship(self.relationship_match(r)))
                .collect();
            if matched.is_empty() {
                return Err(Miss {
                    kind: MissKind::Relationship,
                    name: description.unwrap_or_default().to_string(),
                    candidates: between.iter().filter_map(|r| r.description.clone()).collect(),
                });
            }
            return Ok(matched);
        }

        // Implied relationships only exist between elements; a port names one
        // specific attachment point, which a descendant cannot stand in for.
        if froms.iter().chain(&tos).all(|end| end.port.is_none()) {
            let via: Vec<RelationshipMatch> = self
                .relationships
                .iter()
                .filter(|r| self.within(&froms, &r.source_id) && self.within(&tos, &r.destination_id))
                .filter(|r| described(r))
                .map(|r| self.relationship_match(r))
                .collect();
            if !via.is_empty() {
                return Ok(vec![Target::Implied {
                    from: self.end_path(froms[0]),
                    to: self.end_path(tos[0]),
                    via,
                }]);
            }
        }
        Err(Miss {
            kind: MissKind::Relationship,
            name: format!("{}->{}", self.end_path(froms[0]), self.end_path(tos[0])),
            candidates: Vec::new(),
        })
    }

    /// Whether element `id` (and port, when the end names one) is one of `ends`.
    fn at_end(&self, ends: &[End], id: &str, port_id: Option<&str>) -> bool {
        ends.iter().any(|end| {
            let e = &self.entries[end.entry];
            e.id == id && end.port.is_none_or(|p| Some(e.ports[p].0.as_str()) == port_id)
        })
    }

    /// Whether element `id` is one of `ends` or a descendant of one.
    fn within(&self, ends: &[End], id: &str) -> bool {
        let Some(&start) = self.by_id.get(id) else { return false };
        ends.iter().any(|end| {
            let mut current = Some(start);
            while let Some(i) = current {
                if i == end.entry {
                    return true;
                }
                current = self.entries[i].parent;
            }
            false
        })
    }

    fn relationship_match(&self, r: &Relationship) -> RelationshipMatch {
        let end = |id: &str, port_id: Option<&str>| {
            let path = self.path_of(id);
            let port = self.by_id.get(id).and_then(|&i| {
                self.entries[i].ports.iter().find(|(pid, _)| Some(pid.as_str()) == port_id)
            });
            match port {
                Some((_, name)) => format!("{path}.{name}"),
                None => path,
            }
        };
        RelationshipMatch {
            id: r.id.clone(),
            from: end(&r.source_id, r.source_port_id.as_deref()),
            to: end(&r.destination_id, r.destination_port_id.as_deref()),
            description: r.description.clone(),
            linked_relationship_id: r.linked_relationship_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DSL: &str = r#"workspace {
    model {
        user = person "User"
        shop = softwareSystem "Shop" {
            api = container "API" {
                port http "HTTP"
                orders = component "Orders"
            }
            db = container "shop.db"
            api -> db "reads"
            api -> db "writes"
        }
        pay = softwareSystem "Pay" {
            gateway = container "Gateway"
        }
        orders -> gateway "charges"
        user -> api.http "browses"
        prod = deploymentEnvironment "Production" {
            deploymentNode "Server" {
                apiInstance = instanceOf api
            }
        }
    }
    views {
        systemContext shop "context" {
            include *
        }
    }
}
"#;

    fn workspace() -> Workspace {
        structurizr_dsl::parse_str(DSL).expect("parses")
    }

    fn resolve(ws: &Workspace, reference: &str) -> Result<Vec<Target>, Miss> {
        Catalog::new(ws).resolve(&parse_reference(reference))
    }

    fn one(ws: &Workspace, reference: &str) -> Target {
        let mut targets = resolve(ws, reference).unwrap_or_else(|m| panic!("{reference}: {m:?}"));
        assert_eq!(targets.len(), 1, "{reference}: {targets:?}");
        targets.remove(0)
    }

    #[test]
    fn parses_reference_forms() {
        assert_eq!(parse_reference(" Shop/API "), Reference::Element("Shop/API".into()));
        assert_eq!(parse_reference("view:context"), Reference::View("context".into()));
        assert_eq!(parse_reference("Decision: 3"), Reference::Decision("3".into()));
        assert_eq!(
            parse_reference("Shop/API -> Shop/DB"),
            Reference::Relationship { from: "Shop/API".into(), to: "Shop/DB".into(), description: None }
        );
        assert_eq!(
            parse_reference(r#"Shop/API->Shop/DB "reads orders""#),
            Reference::Relationship {
                from: "Shop/API".into(),
                to: "Shop/DB".into(),
                description: Some("reads orders".into()),
            }
        );
    }

    #[test]
    fn parses_viewer_links() {
        let link = parse_viewer_link(
            "http://localhost:3000/workspace/big-bank#containers&sel=Shop%2FAPI,Shop%2FAPI-%3EShop%2FDB%20%22reads%22",
        )
        .unwrap();
        assert_eq!(link.workspace.as_deref(), Some("big-bank"));
        assert_eq!(link.view.as_deref(), Some("containers"));
        assert_eq!(link.selection, vec!["Shop/API", r#"Shop/API->Shop/DB "reads""#]);

        let graph = parse_viewer_link("http://h/workspace/shop/graph#sel=Shop").unwrap();
        assert_eq!((graph.view, graph.selection), (None, vec!["Shop".to_string()]));

        let diagram = parse_viewer_link("/workspace/shop/diagram/context?x=1").unwrap();
        assert_eq!(diagram.view.as_deref(), Some("context"));

        assert_eq!(parse_viewer_link("#sel=A").unwrap().selection, vec!["A"]);
        assert_eq!(parse_viewer_link("C# Service"), None);
        assert_eq!(parse_viewer_link("Shop/API"), None);
    }

    #[test]
    fn resolves_elements_case_insensitively_and_names_with_dots() {
        let ws = workspace();
        assert!(matches!(one(&ws, "shop/api"), Target::Element { kind: "container", ref path, .. } if path == "Shop/API"));
        assert!(matches!(one(&ws, "Shop/shop.db"), Target::Element { kind: "container", .. }));
        assert!(matches!(one(&ws, "Shop/API/Orders"), Target::Element { kind: "component", .. }));
    }

    #[test]
    fn resolves_ports() {
        let ws = workspace();
        assert!(matches!(one(&ws, "Shop/API.http"), Target::Port { ref path, .. } if path == "Shop/API.HTTP"));
    }

    #[test]
    fn resolves_deployment_elements_under_their_environment() {
        let ws = workspace();
        assert!(matches!(one(&ws, "Production/Server"), Target::Element { kind: "deploymentNode", .. }));
        assert!(matches!(one(&ws, "Production/Server/API"), Target::Element { kind: "containerInstance", .. }));
    }

    #[test]
    fn resolves_relationships_narrowed_by_description() {
        let ws = workspace();
        assert_eq!(resolve(&ws, "Shop/API->Shop/shop.db").unwrap().len(), 2);
        let Target::Relationship(r) = one(&ws, r#"Shop/API->Shop/shop.db "WRITES""#) else { panic!() };
        assert_eq!(r.description.as_deref(), Some("writes"));

        let miss = resolve(&ws, r#"Shop/API->Shop/shop.db "deletes""#).unwrap_err();
        assert_eq!(miss.kind, MissKind::Relationship);
        assert_eq!(miss.candidates, vec!["reads", "writes"]);
    }

    #[test]
    fn relationship_ends_include_ports() {
        let ws = workspace();
        let Target::Relationship(r) = one(&ws, "User->Shop/API") else { panic!() };
        assert_eq!(r.to, "Shop/API.HTTP");
        assert!(matches!(one(&ws, "User->Shop/API.http"), Target::Relationship(_)));
    }

    #[test]
    fn undeclared_pairs_resolve_to_the_relationships_implying_them() {
        let ws = workspace();
        let Target::Implied { from, to, via } = one(&ws, "Shop->Pay") else { panic!() };
        assert_eq!((from.as_str(), to.as_str()), ("Shop", "Pay"));
        assert_eq!(via.len(), 1);
        assert_eq!((via[0].from.as_str(), via[0].to.as_str()), ("Shop/API/Orders", "Pay/Gateway"));

        let miss = resolve(&ws, "Pay->Shop").unwrap_err();
        assert_eq!((miss.kind, miss.name.as_str()), (MissKind::Relationship, "Pay->Shop"));
    }

    #[test]
    fn misses_carry_candidates() {
        let ws = workspace();
        let miss = resolve(&ws, "Shop/APX").unwrap_err();
        assert_eq!(miss.kind, MissKind::Element);
        assert!(miss.candidates.contains(&"Shop/API".to_string()));
        assert!(miss.candidates.contains(&"Shop/API.HTTP".to_string()));

        let miss = resolve(&ws, "Nope->Shop").unwrap_err();
        assert_eq!((miss.kind, miss.name.as_str()), (MissKind::Element, "Nope"));

        assert!(matches!(one(&ws, "view:context"), Target::View { .. }));
        assert_eq!(resolve(&ws, "view:contxt").unwrap_err().candidates, vec!["context"]);
    }
}
