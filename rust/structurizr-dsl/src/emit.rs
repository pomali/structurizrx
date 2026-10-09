//! Model-to-DSL emitter: the inverse of [`crate::parser::parse_str`].
//!
//! `emit` / `emit_with_identifiers` walk a [`Workspace`] and produce DSL text
//! that this crate's own parser accepts and that yields an equivalent model
//! (see `structurizr-dsl/tests/emit.rs` for the round-trip contract). Comments
//! in any original source are not preserved.

use std::collections::{HashMap, HashSet};

use structurizr_model::*;

use crate::identifier_register::{ElementType, IdentifierRegister};
use crate::parser::keyword_sets;

const INDENT: &str = "    ";

/// Emit `ws` as DSL text, synthesizing every identifier.
pub fn emit(ws: &Workspace) -> String {
    let identifiers = IdentifierRegister::default();
    emit_with_identifiers(ws, &identifiers)
}

/// Emit `ws` as DSL text, preferring identifiers from `identifiers` (as
/// returned by `parse_file_with_identifiers` / `parse_str_with_identifiers`)
/// where available, and synthesizing the rest.
pub fn emit_with_identifiers(ws: &Workspace, identifiers: &IdentifierRegister) -> String {
    let ctx = Ctx::build(ws, identifiers);
    ctx.render(ws)
}

// ─── Identifier allocation ─────────────────────────────────────────────────

/// Words the emitted DSL must not use as a bare identifier, because the
/// parser would read them as a keyword or grammar token instead.
fn reserved_words() -> HashSet<String> {
    let mut set = HashSet::new();
    for (_, words) in keyword_sets() {
        for w in *words {
            set.insert(w.to_lowercase());
        }
    }
    let extra = [
        "workspace", "model", "views", "person", "softwaresystem", "container", "component",
        "group", "element", "deploymentenvironment", "deploymentnode", "containerinstance",
        "softwaresysteminstance", "infrastructurenode", "this", "auto", "include", "exclude",
        "styles", "theme", "themes", "kind", "status", "port", "enterprise", "specification",
        "milestones", "perspectives", "properties", "documentation", "docs", "configuration",
        "filtered", "dynamic", "deployment", "systemlandscape", "systemcontext", "image",
        "custom", "branding", "instanceof", "instances", "deploymentgroup", "protocol",
        "direction", "technology", "description", "url", "tags", "perspective", "introduced",
        "retired", "autolayout", "default", "in", "out", "inout", "sync", "async", "publish",
        "subscribe", "dataflow", "dependency", "deploy", "idea", "draft", "specified",
        "implemented", "deprecated", "focus", "layer", "slice", "paths", "rollup", "asof",
        "delta", "lint", "depth", "splitby", "true", "false",
    ];
    for w in extra {
        set.insert(w.to_string());
    }
    set
}

/// Allocates globally-unique, keyword-safe DSL identifiers.
struct IdentAllocator {
    used: HashSet<String>,
    reserved: HashSet<String>,
}

impl IdentAllocator {
    fn new(reserved: HashSet<String>) -> Self {
        Self { used: HashSet::new(), reserved }
    }

    fn is_free(&self, candidate: &str) -> bool {
        let lower = candidate.to_lowercase();
        !self.used.contains(&lower) && !self.reserved.contains(&lower)
    }

    /// Allocate a fresh identifier from `base`, suffixing `2`, `3`, … on
    /// collision (case-insensitive) with an already-used identifier or a DSL
    /// keyword.
    fn alloc(&mut self, base: &str) -> String {
        let base = if base.is_empty() { "element".to_string() } else { base.to_string() };
        let base = if base.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            format!("_{}", base)
        } else {
            base
        };
        if self.is_free(&base) {
            self.used.insert(base.to_lowercase());
            return base;
        }
        let mut n = 2u32;
        loop {
            let candidate = format!("{}{}", base, n);
            if self.is_free(&candidate) {
                self.used.insert(candidate.to_lowercase());
                return candidate;
            }
            n += 1;
        }
    }
}

/// lowerCamelCase of an element name's alphanumeric words: `Web App` ->
/// `webApp`, `E-mail System` -> `eMailSystem`. An all-caps word is an
/// acronym and is cased as a word, so `API` -> `api` and
/// `Customer REST API` -> `customerRestApi`, not `aPI` / `customerRESTAPI`.
fn camel_from_name(name: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();
    for c in name.chars() {
        if c.is_alphanumeric() {
            current.push(c);
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    let mut out = String::new();
    for (i, word) in words.iter().enumerate() {
        let is_acronym = word.chars().count() > 1
            && word.chars().any(char::is_alphabetic)
            && !word.chars().any(char::is_lowercase);
        let word = if is_acronym { word.to_lowercase() } else { word.clone() };
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            if i == 0 {
                out.extend(first.to_lowercase());
            } else {
                out.extend(first.to_uppercase());
            }
            out.push_str(chars.as_str());
        }
    }
    out
}

// ─── Rendering context ─────────────────────────────────────────────────────

struct Ctx {
    id_to_ident: HashMap<String, String>,
    rel_name: HashMap<String, String>,
    /// element id -> (port id -> port identifier)
    port_ident: HashMap<String, HashMap<String, String>>,
    /// element id -> Relationship, gathered from the whole model, used to
    /// resolve dynamic-view steps and other id-keyed lookups.
    rel_by_id: HashMap<String, Relationship>,
    /// Element (non-deployment) relationships re-bucketed by the scope their
    /// *source* actually lives in ("model", or a software system/container
    /// id), independent of which model list currently holds them. Grouping
    /// by "where the relationship struct happens to be stored" would not be
    /// stable across round-trips: the parser attaches an explicit `a -> b`
    /// written directly inside a softwareSystem/container body to that
    /// system's/container's own relationship list regardless of what `a`
    /// resolves to, while the same statement at model level resolves `a`'s
    /// real owner. Re-deriving the scope from `source_id` every time breaks
    /// that oscillation.
    scope_rels: HashMap<String, Vec<Relationship>>,
    /// Deployment relationships bucketed by environment name, emitted at the
    /// `deploymentEnvironment` level (see `scope_rels` doc: the parser only
    /// redistributes explicit `a -> b` statements written at environment
    /// level to their true source node/instance/infrastructure node; the
    /// same statement inside a specific node's body always stays on that
    /// node regardless of what `a` resolves to).
    env_rels: HashMap<String, Vec<Relationship>>,
    group_sep: String,
}

impl Ctx {
    fn build(ws: &Workspace, identifiers: &IdentifierRegister) -> Self {
        let mut alloc = IdentAllocator::new(reserved_words());
        let mut id_to_ident: HashMap<String, String> = HashMap::new();
        let mut rel_name: HashMap<String, String> = HashMap::new();

        // Prefer identifiers the workspace was parsed with. Multiple keys can
        // map to the same id in hierarchical mode (`parent.child` as well as
        // `child`); prefer the shortest key without a dot.
        let mut candidates: HashMap<String, Vec<String>> = HashMap::new();
        for (lower_key, (id, kind)) in &identifiers.identifiers {
            if *kind == ElementType::Relationship {
                continue;
            }
            let spelling = identifiers.spellings.get(lower_key).cloned().unwrap_or_else(|| lower_key.clone());
            candidates.entry(id.clone()).or_default().push(spelling);
        }
        for (id, mut opts) in candidates {
            opts.sort_by(|a, b| {
                a.contains('.')
                    .cmp(&b.contains('.'))
                    .then(a.len().cmp(&b.len()))
                    .then(a.cmp(b))
            });
            let chosen = alloc.alloc(&opts[0]);
            id_to_ident.insert(id, chosen);
        }
        for (lower_key, (id, kind)) in &identifiers.identifiers {
            if *kind != ElementType::Relationship {
                continue;
            }
            let spelling = identifiers.spellings.get(lower_key).cloned().unwrap_or_else(|| lower_key.clone());
            let chosen = alloc.alloc(&spelling);
            rel_name.insert(id.clone(), chosen);
        }

        let group_sep = ws
            .model
            .properties
            .as_ref()
            .and_then(|p| p.get("structurizr.groupSeparator"))
            .cloned()
            .unwrap_or_else(|| "/".to_string());

        let mut ctx = Ctx {
            id_to_ident,
            rel_name,
            port_ident: HashMap::new(),
            rel_by_id: HashMap::new(),
            scope_rels: HashMap::new(),
            env_rels: HashMap::new(),
            group_sep,
        };

        ctx.assign_missing_idents(ws, &mut alloc);
        ctx.assign_ports(ws);
        ctx.collect_relationships(ws);
        ctx.bucket_element_relationships(ws);
        ctx.bucket_deployment_relationships(ws);
        ctx
    }

    /// Group every element-level relationship (source is a person, software
    /// system, container, component or custom element) by the scope its
    /// source id actually lives in, so it is emitted at the one place that
    /// is stable across round-trips (see the `scope_rels` field doc).
    fn bucket_element_relationships(&mut self, ws: &Workspace) {
        let mut scope: HashMap<String, String> = HashMap::new();
        for p in ws.model.people.iter().flatten() {
            scope.insert(p.id.clone(), "model".to_string());
        }
        for ss in ws.model.software_systems.iter().flatten() {
            scope.insert(ss.id.clone(), "model".to_string());
            for c in ss.containers.iter().flatten() {
                scope.insert(c.id.clone(), ss.id.clone());
                for comp in c.components.iter().flatten() {
                    scope.insert(comp.id.clone(), c.id.clone());
                }
            }
        }
        for ce in ws.model.custom_elements.iter().flatten() {
            scope.insert(ce.id.clone(), "model".to_string());
        }

        let mut all: Vec<Relationship> = Vec::new();
        for p in ws.model.people.iter().flatten() {
            all.extend(p.relationships.iter().flatten().cloned());
        }
        for ss in ws.model.software_systems.iter().flatten() {
            all.extend(ss.relationships.iter().flatten().cloned());
            for c in ss.containers.iter().flatten() {
                all.extend(c.relationships.iter().flatten().cloned());
                for comp in c.components.iter().flatten() {
                    all.extend(comp.relationships.iter().flatten().cloned());
                }
            }
        }
        for ce in ws.model.custom_elements.iter().flatten() {
            all.extend(ce.relationships.iter().flatten().cloned());
        }

        for r in all {
            // A named relationship (`name = a -> b`) only has its name
            // registered by the parser when written at model level — inside
            // a softwareSystem/container body an explicit `a -> b` is parsed
            // the same either way, but any `ident =` prefix in front of it is
            // silently discarded. Force named relationships to model scope so
            // the name survives; unnamed ones use the source's natural scope.
            let key = if self.rel_name.contains_key(&r.id) {
                "model".to_string()
            } else {
                scope.get(&r.source_id).cloned().unwrap_or_else(|| "model".to_string())
            };
            self.scope_rels.entry(key).or_default().push(r);
        }
    }

    fn bucket_deployment_relationships(&mut self, ws: &Workspace) {
        for node in ws.model.deployment_nodes.iter().flatten() {
            let env = node.environment.clone().unwrap_or_default();
            self.gather_deployment_rels_into_env(node, &env);
        }
    }

    fn gather_deployment_rels_into_env(&mut self, node: &DeploymentNode, env: &str) {
        let bucket = self.env_rels.entry(env.to_string()).or_default();
        bucket.extend(node.relationships.iter().flatten().cloned());
        for ci in node.container_instances.iter().flatten() {
            bucket.extend(ci.relationships.iter().flatten().cloned());
        }
        for ssi in node.software_system_instances.iter().flatten() {
            bucket.extend(ssi.relationships.iter().flatten().cloned());
        }
        for inf in node.infrastructure_nodes.iter().flatten() {
            bucket.extend(inf.relationships.iter().flatten().cloned());
        }
        if let Some(children) = &node.children {
            for child in children {
                self.gather_deployment_rels_into_env(child, env);
            }
        }
    }

    fn ensure_ident(&mut self, alloc: &mut IdentAllocator, id: &str, name: &str) {
        if !self.id_to_ident.contains_key(id) {
            let ident = alloc.alloc(&camel_from_name(name));
            self.id_to_ident.insert(id.to_string(), ident);
        }
    }

    fn assign_missing_idents(&mut self, ws: &Workspace, alloc: &mut IdentAllocator) {
        for p in ws.model.people.iter().flatten() {
            self.ensure_ident(alloc, &p.id, &p.name);
        }
        for ss in ws.model.software_systems.iter().flatten() {
            self.ensure_ident(alloc, &ss.id, &ss.name);
            for c in ss.containers.iter().flatten() {
                self.ensure_ident(alloc, &c.id, &c.name);
                for comp in c.components.iter().flatten() {
                    self.ensure_ident(alloc, &comp.id, &comp.name);
                }
            }
        }
        for ce in ws.model.custom_elements.iter().flatten() {
            self.ensure_ident(alloc, &ce.id, &ce.name);
        }
        self.assign_deployment_idents(ws.model.deployment_nodes.iter().flatten(), alloc);
    }

    fn assign_deployment_idents<'a>(
        &mut self,
        nodes: impl Iterator<Item = &'a DeploymentNode>,
        alloc: &mut IdentAllocator,
    ) {
        for node in nodes {
            self.ensure_ident(alloc, &node.id, &node.name);
            for ci in node.container_instances.iter().flatten() {
                self.ensure_ident(alloc, &ci.id, "instance");
            }
            for ssi in node.software_system_instances.iter().flatten() {
                self.ensure_ident(alloc, &ssi.id, "instance");
            }
            for inf in node.infrastructure_nodes.iter().flatten() {
                self.ensure_ident(alloc, &inf.id, &inf.name);
            }
            if let Some(children) = &node.children {
                self.assign_deployment_idents(children.iter(), alloc);
            }
        }
    }

    fn assign_ports(&mut self, ws: &Workspace) {
        for p in ws.model.people.iter().flatten() {
            self.assign_element_ports(&p.id, p.ports.as_deref());
        }
        for ss in ws.model.software_systems.iter().flatten() {
            self.assign_element_ports(&ss.id, ss.ports.as_deref());
            for c in ss.containers.iter().flatten() {
                self.assign_element_ports(&c.id, c.ports.as_deref());
                for comp in c.components.iter().flatten() {
                    self.assign_element_ports(&comp.id, comp.ports.as_deref());
                }
            }
        }
        for ce in ws.model.custom_elements.iter().flatten() {
            self.assign_element_ports(&ce.id, ce.ports.as_deref());
        }
    }

    fn assign_element_ports(&mut self, element_id: &str, ports: Option<&[Port]>) {
        let Some(ports) = ports else { return };
        if ports.is_empty() {
            return;
        }
        let mut alloc = IdentAllocator::new(reserved_words());
        let mut map = HashMap::new();
        for port in ports {
            let is_valid = !port.id.is_empty()
                && port.id.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
                && port.id.chars().all(|c| c.is_alphanumeric() || c == '_');
            let base = if is_valid { port.id.clone() } else { camel_from_name(&port.name) };
            let ident = alloc.alloc(&base);
            map.insert(port.id.clone(), ident);
        }
        self.port_ident.insert(element_id.to_string(), map);
    }

    fn collect_relationships(&mut self, ws: &Workspace) {
        for p in ws.model.people.iter().flatten() {
            for r in p.relationships.iter().flatten() {
                self.rel_by_id.insert(r.id.clone(), r.clone());
            }
        }
        for ss in ws.model.software_systems.iter().flatten() {
            for r in ss.relationships.iter().flatten() {
                self.rel_by_id.insert(r.id.clone(), r.clone());
            }
            for c in ss.containers.iter().flatten() {
                for r in c.relationships.iter().flatten() {
                    self.rel_by_id.insert(r.id.clone(), r.clone());
                }
                for comp in c.components.iter().flatten() {
                    for r in comp.relationships.iter().flatten() {
                        self.rel_by_id.insert(r.id.clone(), r.clone());
                    }
                }
            }
        }
        for ce in ws.model.custom_elements.iter().flatten() {
            for r in ce.relationships.iter().flatten() {
                self.rel_by_id.insert(r.id.clone(), r.clone());
            }
        }
        self.collect_deployment_relationships(ws.model.deployment_nodes.iter().flatten());
    }

    fn collect_deployment_relationships<'a>(&mut self, nodes: impl Iterator<Item = &'a DeploymentNode>) {
        for node in nodes {
            for r in node.relationships.iter().flatten() {
                self.rel_by_id.insert(r.id.clone(), r.clone());
            }
            for ci in node.container_instances.iter().flatten() {
                for r in ci.relationships.iter().flatten() {
                    self.rel_by_id.insert(r.id.clone(), r.clone());
                }
            }
            for ssi in node.software_system_instances.iter().flatten() {
                for r in ssi.relationships.iter().flatten() {
                    self.rel_by_id.insert(r.id.clone(), r.clone());
                }
            }
            for inf in node.infrastructure_nodes.iter().flatten() {
                for r in inf.relationships.iter().flatten() {
                    self.rel_by_id.insert(r.id.clone(), r.clone());
                }
            }
            if let Some(children) = &node.children {
                self.collect_deployment_relationships(children.iter());
            }
        }
    }

    fn ident(&self, id: &str) -> String {
        self.id_to_ident.get(id).cloned().unwrap_or_else(|| id.to_string())
    }

    /// An endpoint identifier: `elemIdent` or `elemIdent.portIdent`.
    fn endpoint(&self, elem_id: &str, port_id: &Option<String>) -> String {
        let base = self.ident(elem_id);
        match port_id {
            Some(pid) => {
                let port = self
                    .port_ident
                    .get(elem_id)
                    .and_then(|m| m.get(pid))
                    .cloned()
                    .unwrap_or_else(|| pid.clone());
                format!("{}.{}", base, port)
            }
            None => base,
        }
    }

    /// Reference a ref (element ref or milestone/name) that may or may not be
    /// a known element id.
    fn ref_or_raw(&self, s: &str) -> String {
        if let Some(ident) = self.id_to_ident.get(s) {
            ident.clone()
        } else {
            s.to_string()
        }
    }
}

// ─── Grouping tree ──────────────────────────────────────────────────────────

enum Node<T> {
    Leaf(T),
    Group(String, Vec<Node<T>>),
}

fn insert_path<T>(nodes: &mut Vec<Node<T>>, path: &[String], item: T) {
    if path.is_empty() {
        nodes.push(Node::Leaf(item));
        return;
    }
    let head = &path[0];
    if let Some(Node::Group(_, children)) =
        nodes.iter_mut().find(|n| matches!(n, Node::Group(name, _) if name == head))
    {
        insert_path(children, &path[1..], item);
        return;
    }
    let mut children = Vec::new();
    insert_path(&mut children, &path[1..], item);
    nodes.push(Node::Group(head.clone(), children));
}

fn build_group_tree<T>(items: Vec<(Option<String>, T)>, sep: &str) -> Vec<Node<T>> {
    let mut nodes: Vec<Node<T>> = Vec::new();
    for (group, item) in items {
        match group {
            None => nodes.push(Node::Leaf(item)),
            Some(g) if g.is_empty() => nodes.push(Node::Leaf(item)),
            Some(g) => {
                let path: Vec<String> = g.split(sep).map(|s| s.to_string()).collect();
                insert_path(&mut nodes, &path, item);
            }
        }
    }
    nodes
}

// ─── String formatting helpers ─────────────────────────────────────────────

fn quote(s: &str) -> String {
    if s.contains('\n') && !s.contains("\"\"\"") {
        format!("\"\"\"{}\"\"\"", s)
    } else {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{}\"", escaped)
    }
}

/// Trailing-optional positional args: emits up to and including the last
/// `Some`, filling any earlier gaps with `""`.
fn positional(parts: &[Option<String>]) -> String {
    let last = parts.iter().rposition(|p| p.is_some());
    match last {
        None => String::new(),
        Some(last_idx) => {
            let mut out = String::new();
            for part in parts.iter().take(last_idx + 1) {
                out.push(' ');
                out.push_str(&quote(part.as_deref().unwrap_or("")));
            }
            out
        }
    }
}

/// Split `tags` into (remaining tags after stripping `defaults` and
/// `Uncertain`, whether `Uncertain` was present).
fn strip_tags(tags: &Option<String>, defaults: &[&str]) -> (Option<String>, bool) {
    let Some(t) = tags else { return (None, false) };
    let mut uncertain = false;
    let mut remaining = Vec::new();
    for part in t.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        if p == "Uncertain" {
            uncertain = true;
            continue;
        }
        if defaults.contains(&p) {
            continue;
        }
        remaining.push(p.to_string());
    }
    (if remaining.is_empty() { None } else { Some(remaining.join(",")) }, uncertain)
}

fn status_word(s: Status) -> &'static str {
    match s {
        Status::Idea => "idea",
        Status::Draft => "draft",
        Status::Specified => "specified",
        Status::Implemented => "implemented",
        Status::Deprecated => "deprecated",
    }
}

fn kind_word(k: RelationshipKind) -> &'static str {
    match k {
        RelationshipKind::Sync => "sync",
        RelationshipKind::Async => "async",
        RelationshipKind::Publish => "publish",
        RelationshipKind::Subscribe => "subscribe",
        RelationshipKind::Dataflow => "dataflow",
        RelationshipKind::Dependency => "dependency",
        RelationshipKind::Deploy => "deploy",
    }
}

fn direction_word(d: PortDirection) -> &'static str {
    match d {
        PortDirection::In => "in",
        PortDirection::Out => "out",
        PortDirection::InOut => "inout",
    }
}

fn rank_dir_word(rd: &str) -> String {
    match rd {
        "TopBottom" => "tb".to_string(),
        "BottomTop" => "bt".to_string(),
        "LeftRight" => "lr".to_string(),
        "RightLeft" => "rl".to_string(),
        other => other.to_lowercase(),
    }
}

const DEFAULT_THEME_URL: &str = "https://static.structurizr.com/themes/default/theme.json";

// ─── Rendering ──────────────────────────────────────────────────────────────

struct Out {
    buf: String,
}

impl Out {
    fn new() -> Self {
        Self { buf: String::new() }
    }
    fn line(&mut self, indent: usize, text: &str) {
        if text.is_empty() {
            self.buf.push('\n');
        } else {
            for _ in 0..indent {
                self.buf.push_str(INDENT);
            }
            self.buf.push_str(text);
            self.buf.push('\n');
        }
    }
}

/// Common element attributes shared across kinds, used to render the body.
struct Attrs<'a> {
    status: Option<Status>,
    introduced: Option<&'a str>,
    retired: Option<&'a str>,
    url: Option<&'a str>,
    perspectives: &'a [Perspective],
    properties: Option<&'a HashMap<String, String>>,
    ports: Option<&'a [Port]>,
}

impl Ctx {
    fn render(&self, ws: &Workspace) -> String {
        let mut out = Out::new();
        let name_desc = positional(&[Some(ws.name.clone()), ws.description.clone()]);
        out.line(0, &format!("workspace{} {{", name_desc));
        out.line(0, "");

        if ws.documentation.as_ref().is_some_and(|d| d.decisions.as_ref().is_some_and(|v| !v.is_empty())) {
            out.line(1, "// decisions were imported from an !adrs directory; re-add the directive by hand");
        }

        if let Some(milestones) = &ws.milestones {
            if !milestones.is_empty() {
                out.line(1, "milestones {");
                for m in milestones {
                    let line = format!(
                        "{}{}",
                        bare_or_quote(&m.name),
                        positional(&[m.date.clone(), m.description.clone()])
                    );
                    out.line(2, &line);
                }
                out.line(1, "}");
            }
        }

        if let Some(perspectives) = &ws.perspectives {
            if !perspectives.is_empty() {
                out.line(1, "perspectives {");
                for p in perspectives {
                    // The workspace-level registry has no `perspective`
                    // keyword per entry (`perspectives { security "…" }`),
                    // unlike the element/relationship body form.
                    let line = format!(
                        "{}{}",
                        bare_or_quote(&p.name),
                        positional(&[p.description.clone(), p.value.clone()])
                    );
                    out.line(2, &line);
                }
                out.line(1, "}");
            }
        }

        if let Some(props) = &ws.properties {
            if !props.is_empty() {
                self.render_properties(&mut out, 1, props);
            }
        }

        out.line(1, "model {");
        self.render_model(&mut out, ws);
        out.line(1, "}");
        out.line(0, "");

        out.line(1, "views {");
        self.render_views(&mut out, ws);
        out.line(1, "}");

        out.line(0, "}");
        out.buf
    }

    fn render_perspective(&self, out: &mut Out, indent: usize, p: &Perspective) {
        let line = format!(
            "perspective {}{}",
            quote(&p.name),
            positional(&[p.description.clone(), p.value.clone()])
        );
        out.line(indent, &line);
    }

    fn render_properties(&self, out: &mut Out, indent: usize, props: &HashMap<String, String>) {
        out.line(indent, "properties {");
        let mut keys: Vec<&String> = props.keys().collect();
        keys.sort();
        for k in keys {
            out.line(indent + 1, &format!("{} {}", bare_or_quote(k), quote(&props[k])));
        }
        out.line(indent, "}");
    }

    fn render_attrs(&self, out: &mut Out, indent: usize, attrs: &Attrs) {
        if let Some(url) = attrs.url {
            out.line(indent, &format!("url {}", quote(url)));
        }
        if let Some(status) = attrs.status {
            out.line(indent, &format!("status {}", status_word(status)));
        }
        if let Some(m) = attrs.introduced {
            out.line(indent, &format!("introduced {}", bare_or_quote(m)));
        }
        if let Some(m) = attrs.retired {
            out.line(indent, &format!("retired {}", bare_or_quote(m)));
        }
        for p in attrs.perspectives {
            self.render_perspective(out, indent, p);
        }
        if let Some(props) = attrs.properties {
            if !props.is_empty() {
                self.render_properties(out, indent, props);
            }
        }
    }

    fn render_port_for(&self, out: &mut Out, indent: usize, element_id: &str, port: &Port) {
        let ident = self
            .port_ident
            .get(element_id)
            .and_then(|m| m.get(&port.id))
            .cloned()
            .unwrap_or_else(|| port.id.clone());
        let has_body = port.protocol.is_some()
            || port.direction.is_some()
            || port.description.is_some()
            || port.url.is_some()
            || port.tags.is_some()
            || port.properties.as_ref().is_some_and(|p| !p.is_empty())
            || port.perspectives.as_ref().is_some_and(|p| !p.is_empty());
        let header = format!("port {} {}", ident, quote(&port.name));
        if !has_body {
            out.line(indent, &header);
            return;
        }
        out.line(indent, &format!("{} {{", header));
        if let Some(protocol) = &port.protocol {
            out.line(indent + 1, &format!("protocol {}", quote(protocol)));
        }
        if let Some(dir) = port.direction {
            out.line(indent + 1, &format!("direction {}", direction_word(dir)));
        }
        if let Some(desc) = &port.description {
            out.line(indent + 1, &format!("description {}", quote(desc)));
        }
        if let Some(url) = &port.url {
            out.line(indent + 1, &format!("url {}", quote(url)));
        }
        if let Some(tags) = &port.tags {
            if !tags.is_empty() {
                out.line(indent + 1, &format!("tags {}", quote(tags)));
            }
        }
        if let Some(props) = &port.properties {
            if !props.is_empty() {
                self.render_properties(out, indent + 1, props);
            }
        }
        for p in port.perspectives.iter().flatten() {
            self.render_perspective(out, indent + 1, p);
        }
        out.line(indent, "}");
    }

    // ─── Model ──────────────────────────────────────────────────────────

    fn render_model(&self, out: &mut Out, ws: &Workspace) {
        if let Some(props) = &ws.model.properties {
            if !props.is_empty() {
                self.render_properties(out, 2, props);
            }
        }

        // People, software systems and custom elements share the model-level
        // grouping and relationship-emission scope.
        let mut items: Vec<(Option<String>, ModelLeaf)> = Vec::new();
        for p in ws.model.people.iter().flatten() {
            items.push((p.group.clone(), ModelLeaf::Person(p)));
        }
        for ss in ws.model.software_systems.iter().flatten() {
            items.push((ss.group.clone(), ModelLeaf::SoftwareSystem(ss)));
        }
        for ce in ws.model.custom_elements.iter().flatten() {
            items.push((ce.group.clone(), ModelLeaf::CustomElement(ce)));
        }
        let tree = build_group_tree(items, &self.group_sep);
        self.render_model_nodes(out, 2, &tree);

        // Model-level relationships: whichever element/system/custom element
        // source ids resolved to model scope (see `scope_rels`).
        if let Some(rels) = self.scope_rels.get("model") {
            self.render_relationships(out, 2, rels);
        }

        if let Some(nodes) = &ws.model.deployment_nodes {
            self.render_deployment_environments(out, 2, nodes);
        }
    }

    fn render_model_nodes(&self, out: &mut Out, indent: usize, nodes: &[Node<ModelLeaf>]) {
        for node in nodes {
            match node {
                Node::Leaf(leaf) => self.render_model_leaf(out, indent, leaf),
                Node::Group(name, children) => {
                    out.line(indent, &format!("group {} {{", quote(name)));
                    self.render_model_nodes(out, indent + 1, children);
                    out.line(indent, "}");
                }
            }
        }
    }

    fn render_model_leaf(&self, out: &mut Out, indent: usize, leaf: &ModelLeaf) {
        match leaf {
            ModelLeaf::Person(p) => self.render_person(out, indent, p),
            ModelLeaf::SoftwareSystem(ss) => self.render_software_system(out, indent, ss),
            ModelLeaf::CustomElement(ce) => self.render_custom_element(out, indent, ce),
        }
    }

    fn render_person(&self, out: &mut Out, indent: usize, p: &Person) {
        let (tags, uncertain) = strip_tags(&p.tags, &["Element", "Person"]);
        let head = format!(
            "{} = person{}",
            self.ident(&p.id),
            positional(&[Some(p.name.clone()), p.description.clone(), tags])
        );
        self.render_element_common(
            out,
            indent,
            &head,
            uncertain,
            Attrs {
                status: p.status,
                introduced: p.introduced.as_deref(),
                retired: p.retired.as_deref(),
                url: p.url.as_deref(),
                perspectives: p.perspectives.as_deref().unwrap_or(&[]),
                properties: p.properties.as_ref(),
                ports: p.ports.as_deref(),
            },
            &p.id,
        );
    }

    fn render_software_system(&self, out: &mut Out, indent: usize, ss: &SoftwareSystem) {
        let (tags, uncertain) = strip_tags(&ss.tags, &["Element", "Software System"]);
        let head = format!(
            "{} = softwareSystem{}",
            self.ident(&ss.id),
            positional(&[Some(ss.name.clone()), ss.description.clone(), tags])
        );

        let mut body = Out::new();
        if let Some(props) = &ss.properties {
            if !props.is_empty() {
                self.render_properties(&mut body, indent + 1, props);
            }
        }
        let mut items: Vec<(Option<String>, &Container)> = Vec::new();
        for c in ss.containers.iter().flatten() {
            items.push((c.group.clone(), c));
        }
        let tree = build_group_tree(items, &self.group_sep);
        self.render_container_nodes(&mut body, indent + 1, &tree);

        if let Some(rels) = self.scope_rels.get(&ss.id) {
            self.render_relationships(&mut body, indent + 1, rels);
        }

        self.render_attrs(
            &mut body,
            indent + 1,
            &Attrs {
                status: ss.status,
                introduced: ss.introduced.as_deref(),
                retired: ss.retired.as_deref(),
                url: ss.url.as_deref(),
                perspectives: ss.perspectives.as_deref().unwrap_or(&[]),
                properties: None,
                ports: ss.ports.as_deref(),
            },
        );
        for port in ss.ports.iter().flatten() {
            self.render_port_for(&mut body, indent + 1, &ss.id, port);
        }

        emit_block(out, indent, &head, uncertain, &body.buf);
    }

    fn render_container_nodes(&self, out: &mut Out, indent: usize, nodes: &[Node<&Container>]) {
        for node in nodes {
            match node {
                Node::Leaf(c) => self.render_container(out, indent, c),
                Node::Group(name, children) => {
                    out.line(indent, &format!("group {} {{", quote(name)));
                    self.render_container_nodes(out, indent + 1, children);
                    out.line(indent, "}");
                }
            }
        }
    }

    fn render_container(&self, out: &mut Out, indent: usize, c: &Container) {
        let (tags, uncertain) = strip_tags(&c.tags, &["Element", "Container"]);
        let head = format!(
            "{} = container{}",
            self.ident(&c.id),
            positional(&[Some(c.name.clone()), c.description.clone(), c.technology.clone(), tags])
        );

        let mut body = Out::new();
        let mut items: Vec<(Option<String>, &Component)> = Vec::new();
        for comp in c.components.iter().flatten() {
            items.push((comp.group.clone(), comp));
        }
        let tree = build_group_tree(items, &self.group_sep);
        self.render_component_nodes(&mut body, indent + 1, &tree);

        if let Some(rels) = self.scope_rels.get(&c.id) {
            self.render_relationships(&mut body, indent + 1, rels);
        }

        self.render_attrs(
            &mut body,
            indent + 1,
            &Attrs {
                status: c.status,
                introduced: c.introduced.as_deref(),
                retired: c.retired.as_deref(),
                url: c.url.as_deref(),
                perspectives: c.perspectives.as_deref().unwrap_or(&[]),
                properties: c.properties.as_ref(),
                ports: c.ports.as_deref(),
            },
        );
        for port in c.ports.iter().flatten() {
            self.render_port_for(&mut body, indent + 1, &c.id, port);
        }

        emit_block(out, indent, &head, uncertain, &body.buf);
    }

    fn render_component_nodes(&self, out: &mut Out, indent: usize, nodes: &[Node<&Component>]) {
        for node in nodes {
            match node {
                Node::Leaf(comp) => self.render_component(out, indent, comp),
                Node::Group(name, children) => {
                    out.line(indent, &format!("group {} {{", quote(name)));
                    self.render_component_nodes(out, indent + 1, children);
                    out.line(indent, "}");
                }
            }
        }
    }

    fn render_component(&self, out: &mut Out, indent: usize, comp: &Component) {
        let (tags, uncertain) = strip_tags(&comp.tags, &["Element", "Component"]);
        let head = format!(
            "{} = component{}",
            self.ident(&comp.id),
            positional(&[Some(comp.name.clone()), comp.description.clone(), comp.technology.clone(), tags])
        );
        self.render_element_common(
            out,
            indent,
            &head,
            uncertain,
            Attrs {
                status: comp.status,
                introduced: comp.introduced.as_deref(),
                retired: comp.retired.as_deref(),
                url: comp.url.as_deref(),
                perspectives: comp.perspectives.as_deref().unwrap_or(&[]),
                properties: comp.properties.as_ref(),
                ports: comp.ports.as_deref(),
            },
            &comp.id,
        );
    }

    fn render_custom_element(&self, out: &mut Out, indent: usize, ce: &CustomElement) {
        let (tags, uncertain) = strip_tags(&ce.tags, &["Element"]);
        let head = format!(
            "{} = element{}",
            self.ident(&ce.id),
            positional(&[Some(ce.name.clone()), ce.metadata.clone(), ce.description.clone(), tags])
        );
        self.render_element_common(
            out,
            indent,
            &head,
            uncertain,
            Attrs {
                status: ce.status,
                introduced: ce.introduced.as_deref(),
                retired: ce.retired.as_deref(),
                url: ce.url.as_deref(),
                perspectives: ce.perspectives.as_deref().unwrap_or(&[]),
                properties: ce.properties.as_ref(),
                ports: ce.ports.as_deref(),
            },
            &ce.id,
        );
    }

    /// Render a leaf element (person/component/custom element: no nested
    /// elements of their own, only attrs+ports+relationships).
    fn render_element_common(&self, out: &mut Out, indent: usize, head: &str, uncertain: bool, attrs: Attrs, element_id: &str) {
        let mut body = Out::new();
        self.render_attrs(&mut body, indent + 1, &attrs);
        for port in attrs.ports.into_iter().flatten() {
            self.render_port_for(&mut body, indent + 1, element_id, port);
        }
        emit_block(out, indent, head, uncertain, &body.buf);
    }

    fn render_relationships(&self, out: &mut Out, indent: usize, rels: &[Relationship]) {
        self.render_relationships_named(out, indent, rels, true);
    }

    /// `allow_name` is false for deployment-environment-level relationships:
    /// that grammar position doesn't accept `name = a -> b` (only bare
    /// `a -> b`), so a named relationship there is emitted unnamed.
    fn render_relationships_named(&self, out: &mut Out, indent: usize, rels: &[Relationship], allow_name: bool) {
        for r in rels {
            if r.linked_relationship_id.is_some() {
                continue;
            }
            self.render_relationship(out, indent, r, allow_name);
        }
    }

    fn render_relationship(&self, out: &mut Out, indent: usize, r: &Relationship, allow_name: bool) {
        let (tags, uncertain) = strip_tags(&r.tags, &["Relationship"]);
        let src = self.endpoint(&r.source_id, &r.source_port_id);
        let dst = self.endpoint(&r.destination_id, &r.destination_port_id);
        let name = if allow_name { self.rel_name.get(&r.id).cloned() } else { None };
        let prefix = name.as_deref().map(|n| format!("{} = ", n)).unwrap_or_default();
        let head = format!(
            "{}{} -> {}{}",
            prefix,
            src,
            dst,
            positional(&[r.description.clone(), r.technology.clone(), tags])
        );

        let needs_body = r.kind.is_some()
            || r.status.is_some()
            || r.introduced.is_some()
            || r.retired.is_some()
            || r.perspectives.as_ref().is_some_and(|p| !p.is_empty())
            || r.url.is_some()
            || r.properties.as_ref().is_some_and(|p| !p.is_empty());

        if !needs_body {
            out.line(indent, &format!("{}{}", head, if uncertain { " ?" } else { "" }));
            return;
        }

        let mut body = Out::new();
        if let Some(kind) = r.kind {
            body.line(indent + 1, &format!("kind {}", kind_word(kind)));
        }
        if let Some(status) = r.status {
            body.line(indent + 1, &format!("status {}", status_word(status)));
        }
        if let Some(m) = &r.introduced {
            body.line(indent + 1, &format!("introduced {}", bare_or_quote(m)));
        }
        if let Some(m) = &r.retired {
            body.line(indent + 1, &format!("retired {}", bare_or_quote(m)));
        }
        for p in r.perspectives.iter().flatten() {
            self.render_perspective(&mut body, indent + 1, p);
        }
        if let Some(url) = &r.url {
            body.line(indent + 1, &format!("url {}", quote(url)));
        }
        if let Some(props) = &r.properties {
            if !props.is_empty() {
                self.render_properties(&mut body, indent + 1, props);
            }
        }

        out.line(indent, &format!("{}{} {{", head, if uncertain { " ?" } else { "" }));
        out.buf.push_str(&body.buf);
        out.line(indent, "}");
    }

    // ─── Deployment ─────────────────────────────────────────────────────

    fn render_deployment_environments(&self, out: &mut Out, indent: usize, nodes: &[DeploymentNode]) {
        let mut order: Vec<String> = Vec::new();
        let mut by_env: HashMap<String, Vec<&DeploymentNode>> = HashMap::new();
        for n in nodes {
            let env = n.environment.clone().unwrap_or_default();
            if !by_env.contains_key(&env) {
                order.push(env.clone());
            }
            by_env.entry(env).or_default().push(n);
        }
        for env in order {
            out.line(indent, &format!("deploymentEnvironment {} {{", quote(&env)));
            for node in &by_env[&env] {
                self.render_deployment_node(out, indent + 1, node);
            }
            // Deployment relationships are always emitted at environment
            // level (see `env_rels` doc) so re-parsing redistributes them to
            // their true source node/instance/infrastructure node the same
            // way every time.
            if let Some(rels) = self.env_rels.get(&env) {
                self.render_relationships_named(out, indent + 1, rels, false);
            }
            out.line(indent, "}");
        }
    }

    fn render_deployment_node(&self, out: &mut Out, indent: usize, node: &DeploymentNode) {
        let (tags, uncertain) = strip_tags(&node.tags, &["Element", "Deployment Node"]);
        let count = match &node.instances {
            Some(serde_json::Value::Number(n)) => Some(n.to_string()),
            _ => None,
        };
        let mut head = format!(
            "{} = deploymentNode{}",
            self.ident(&node.id),
            positional(&[Some(node.name.clone()), node.description.clone(), node.technology.clone(), tags])
        );
        if let Some(c) = count {
            head.push(' ');
            head.push_str(&c);
        }

        let mut body = Out::new();
        if let Some(children) = &node.children {
            for child in children {
                self.render_deployment_node(&mut body, indent + 1, child);
            }
        }
        for ci in node.container_instances.iter().flatten() {
            self.render_container_instance(&mut body, indent + 1, ci);
        }
        for ssi in node.software_system_instances.iter().flatten() {
            self.render_software_system_instance(&mut body, indent + 1, ssi);
        }
        for inf in node.infrastructure_nodes.iter().flatten() {
            self.render_infrastructure_node(&mut body, indent + 1, inf);
        }
        if let Some(props) = &node.properties {
            if !props.is_empty() {
                self.render_properties(&mut body, indent + 1, props);
            }
        }
        if let Some(url) = &node.url {
            body.line(indent + 1, &format!("url {}", quote(url)));
        }

        emit_block(out, indent, &head, uncertain, &body.buf);
    }

    fn render_container_instance(&self, out: &mut Out, indent: usize, ci: &ContainerInstance) {
        let (tags, _uncertain) = strip_tags(&ci.tags, &["Container Instance"]);
        let ref_ident = self.ref_or_raw(&ci.container_id);
        let head = format!(
            "{} = containerInstance {}{}",
            self.ident(&ci.id),
            ref_ident,
            positional(&[tags])
        );
        let mut body = Out::new();
        if let Some(props) = &ci.properties {
            if !props.is_empty() {
                self.render_properties(&mut body, indent + 1, props);
            }
        }
        if let Some(url) = &ci.url {
            body.line(indent + 1, &format!("url {}", quote(url)));
        }
        emit_block(out, indent, &head, false, &body.buf);
    }

    fn render_software_system_instance(&self, out: &mut Out, indent: usize, ssi: &SoftwareSystemInstance) {
        let (tags, _uncertain) = strip_tags(&ssi.tags, &["Software System Instance"]);
        let ref_ident = self.ref_or_raw(&ssi.software_system_id);
        let head = format!(
            "{} = softwareSystemInstance {}{}",
            self.ident(&ssi.id),
            ref_ident,
            positional(&[tags])
        );
        let mut body = Out::new();
        if let Some(props) = &ssi.properties {
            if !props.is_empty() {
                self.render_properties(&mut body, indent + 1, props);
            }
        }
        if let Some(url) = &ssi.url {
            body.line(indent + 1, &format!("url {}", quote(url)));
        }
        emit_block(out, indent, &head, false, &body.buf);
    }

    fn render_infrastructure_node(&self, out: &mut Out, indent: usize, inf: &InfrastructureNode) {
        let (tags, uncertain) = strip_tags(&inf.tags, &["Infrastructure Node"]);
        let head = format!(
            "{} = infrastructureNode{}",
            self.ident(&inf.id),
            positional(&[Some(inf.name.clone()), inf.description.clone(), inf.technology.clone(), tags])
        );
        let mut body = Out::new();
        if let Some(props) = &inf.properties {
            if !props.is_empty() {
                self.render_properties(&mut body, indent + 1, props);
            }
        }
        if let Some(url) = &inf.url {
            body.line(indent + 1, &format!("url {}", quote(url)));
        }
        emit_block(out, indent, &head, uncertain, &body.buf);
    }

    // ─── Views ──────────────────────────────────────────────────────────

    fn render_views(&self, out: &mut Out, ws: &Workspace) {
        let has_auto_specs = ws.views.auto_views.as_ref().is_some_and(|v| !v.is_empty());
        for spec in ws.views.auto_views.iter().flatten() {
            out.line(2, &self.auto_view_line(spec));
        }

        let skip_auto_materialized = |key: &Option<String>| -> bool {
            has_auto_specs && key.as_deref().is_some_and(|k| k.starts_with("auto-"))
        };

        for v in ws.views.system_landscape_views.iter().flatten() {
            if skip_auto_materialized(&v.key) {
                continue;
            }
            self.render_view_base(
                out,
                "systemLandscape",
                None,
                &v.key,
                &v.title,
                &v.description,
                v.element_views.as_deref(),
                v.relationship_views.as_deref(),
                v.automatic_layout.as_ref(),
            );
        }
        for v in ws.views.system_context_views.iter().flatten() {
            if skip_auto_materialized(&v.key) {
                continue;
            }
            self.render_view_base(
                out,
                "systemContext",
                Some(self.ident(&v.software_system_id)),
                &v.key,
                &v.title,
                &v.description,
                v.element_views.as_deref(),
                v.relationship_views.as_deref(),
                v.automatic_layout.as_ref(),
            );
        }
        for v in ws.views.container_views.iter().flatten() {
            if skip_auto_materialized(&v.key) {
                continue;
            }
            self.render_view_base(
                out,
                "container",
                Some(self.ident(&v.software_system_id)),
                &v.key,
                &v.title,
                &v.description,
                v.element_views.as_deref(),
                v.relationship_views.as_deref(),
                v.automatic_layout.as_ref(),
            );
        }
        for v in ws.views.component_views.iter().flatten() {
            if skip_auto_materialized(&v.key) {
                continue;
            }
            self.render_view_base(
                out,
                "component",
                Some(self.ident(&v.container_id)),
                &v.key,
                &v.title,
                &v.description,
                v.element_views.as_deref(),
                v.relationship_views.as_deref(),
                v.automatic_layout.as_ref(),
            );
        }
        for v in ws.views.dynamic_views.iter().flatten() {
            if skip_auto_materialized(&v.key) {
                continue;
            }
            self.render_dynamic_view(out, v);
        }
        for v in ws.views.deployment_views.iter().flatten() {
            if skip_auto_materialized(&v.key) {
                continue;
            }
            let scope = v.software_system_id.as_deref().map(|id| self.ident(id)).unwrap_or_else(|| "*".to_string());
            let head = format!(
                "deployment {} {}{}",
                scope,
                quote(&v.environment),
                positional(&[v.key.clone(), v.title.clone()])
            );
            let mut body = Out::new();
            self.render_include_and_layout(
                &mut body,
                3,
                v.element_views.as_deref(),
                &v.description,
                v.automatic_layout.as_ref(),
            );
            emit_block(out, 2, &head, false, &body.buf);
        }
        for v in ws.views.filtered_views.iter().flatten() {
            if skip_auto_materialized(&v.key) {
                continue;
            }
            let mode = v.mode.to_lowercase();
            let tags = v.tags.as_ref().map(|t| t.join(","));
            let line = format!(
                "filtered {} {}{}",
                quote(&v.base_view_key),
                mode,
                positional(&[tags, v.key.clone(), v.title.clone()])
            );
            out.line(2, &line);
        }

        if let Some(cfg) = &ws.views.configuration {
            if let Some(styles) = &cfg.styles {
                self.render_styles(out, styles);
            }
            if let Some(themes) = &cfg.themes {
                if !themes.is_empty() {
                    let words: Vec<String> = themes
                        .iter()
                        .map(|t| if t == DEFAULT_THEME_URL { "default".to_string() } else { quote(t) })
                        .collect();
                    out.line(2, &format!("theme {}", words.join(" ")));
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_view_base(
        &self,
        out: &mut Out,
        keyword: &str,
        scope_ident: Option<String>,
        key: &Option<String>,
        title: &Option<String>,
        description: &Option<String>,
        elements: Option<&[ElementView]>,
        _relationships: Option<&[RelationshipView]>,
        layout: Option<&AutomaticLayout>,
    ) {
        let scope = scope_ident.map(|s| format!(" {}", s)).unwrap_or_default();
        let head = format!("{}{}{}", keyword, scope, positional(&[key.clone(), title.clone()]));
        let mut body = Out::new();
        self.render_include_and_layout(&mut body, 3, elements, description, layout);
        emit_block(out, 2, &head, false, &body.buf);
    }

    fn render_include_and_layout(
        &self,
        out: &mut Out,
        indent: usize,
        elements: Option<&[ElementView]>,
        description: &Option<String>,
        layout: Option<&AutomaticLayout>,
    ) {
        if let Some(elements) = elements {
            if !elements.is_empty() {
                // The parser populates `include *`/neighborhood includes via a
                // HashSet, so `ElementView` order is not itself stable across
                // reparses. Sort before emitting so the *set* of included
                // elements determines the text deterministically.
                let mut idents: Vec<String> = elements.iter().map(|e| self.ident(&e.id)).collect();
                idents.sort();
                self.render_wrapped(out, indent, "include", &idents);
            }
        }
        if let Some(desc) = description {
            out.line(indent, &format!("description {}", quote(desc)));
        }
        if let Some(layout) = layout {
            let dir = layout.rank_direction.as_deref().map(rank_dir_word).unwrap_or_else(|| "tb".to_string());
            let mut line = format!("autoLayout {}", dir);
            if let (Some(rs), Some(ns)) = (layout.rank_separation, layout.node_separation) {
                line.push_str(&format!(" {} {}", rs, ns));
            }
            out.line(indent, &line);
        }
    }

    fn render_wrapped(&self, out: &mut Out, indent: usize, keyword: &str, words: &[String]) {
        let prefix = format!("{}{} ", INDENT.repeat(indent), keyword);
        let mut line = prefix.clone();
        let mut first = true;
        for w in words {
            let piece = if first { w.clone() } else { format!(" {}", w) };
            if !first && line.len() + piece.len() > 100 {
                out.buf.push_str(line.trim_end());
                out.buf.push('\n');
                line = format!("{}{} ", INDENT.repeat(indent), keyword);
                line.push_str(w);
            } else {
                line.push_str(&piece);
            }
            first = false;
        }
        out.buf.push_str(line.trim_end());
        out.buf.push('\n');
    }

    fn render_dynamic_view(&self, out: &mut Out, v: &DynamicView) {
        let scope = v.element_id.as_deref().map(|id| self.ident(id)).unwrap_or_else(|| "*".to_string());
        let head = format!("dynamic {}{}", scope, positional(&[v.key.clone(), v.title.clone()]));
        let mut steps: Vec<&RelationshipView> = v.relationship_views.iter().flatten().collect();
        steps.sort_by_key(|rv| rv.order.as_deref().and_then(|o| o.parse::<i64>().ok()).unwrap_or(0));
        let mut body = Out::new();
        for rv in steps {
            let Some(r) = self.rel_by_id.get(&rv.id) else { continue };
            let (src, dst) = if rv.response == Some(true) {
                (&r.destination_id, &r.source_id)
            } else {
                (&r.source_id, &r.destination_id)
            };
            let desc = rv.description.clone().or_else(|| r.description.clone());
            let line = format!("{} -> {}{}", self.ident(src), self.ident(dst), positional(&[desc]));
            body.line(3, &line);
        }
        if let Some(desc) = &v.description {
            body.line(3, &format!("description {}", quote(desc)));
        }
        if let Some(layout) = &v.automatic_layout {
            let dir = layout.rank_direction.as_deref().map(rank_dir_word).unwrap_or_else(|| "tb".to_string());
            body.line(3, &format!("autoLayout {}", dir));
        }
        emit_block(out, 2, &head, false, &body.buf);
    }

    fn auto_view_line(&self, spec: &AutoViewSpec) -> String {
        match spec.generator.as_str() {
            "focus" => {
                let target = spec.target.as_deref().map(|t| self.ref_or_raw(t)).unwrap_or_default();
                let mut opts = Vec::new();
                if let Some(d) = spec.depth {
                    opts.push(format!("depth {}", if d == u32::MAX { "*".to_string() } else { d.to_string() }));
                }
                if let Some(dir) = &spec.direction {
                    opts.push(format!("direction {}", dir));
                }
                if let Some(sb) = &spec.split_by {
                    opts.push(format!("splitBy {}", sb));
                }
                if let Some(a) = &spec.asof {
                    opts.push(format!("asof {}", bare_or_quote(a)));
                }
                if opts.is_empty() {
                    format!("auto focus {}", target)
                } else {
                    format!("auto focus {} {{ {} }}", target, opts.join(" "))
                }
            }
            "perspective" | "layer" | "asof" | "rollup" => {
                format!("auto {} {}", spec.generator, spec.target.as_deref().map(bare_or_quote).unwrap_or_default())
            }
            "paths" => {
                let t1 = spec.target.as_deref().map(|t| self.ref_or_raw(t)).unwrap_or_default();
                let t2 = spec.target2.as_deref().map(|t| self.ref_or_raw(t)).unwrap_or_default();
                format!("auto paths {} {}", t1, t2)
            }
            "delta" => format!(
                "auto delta {} {}",
                spec.target.as_deref().unwrap_or_default(),
                spec.target2.as_deref().unwrap_or_default()
            ),
            "slice" => format!("auto slice {}", spec.expression.as_deref().unwrap_or_default()),
            "lint" => "auto lint".to_string(),
            _ => "auto".to_string(),
        }
    }

    fn render_styles(&self, out: &mut Out, styles: &Styles) {
        let has_any = styles.elements.as_ref().is_some_and(|v| !v.is_empty())
            || styles.relationships.as_ref().is_some_and(|v| !v.is_empty());
        if !has_any {
            return;
        }
        out.line(2, "styles {");
        for e in styles.elements.iter().flatten() {
            let mut body = Out::new();
            if let Some(shape) = &e.shape {
                body.line(4, &format!("shape {}", shape));
            }
            if let Some(bg) = &e.background {
                body.line(4, &format!("background {}", quote(bg)));
            }
            if let Some(c) = &e.color {
                body.line(4, &format!("color {}", quote(c)));
            }
            if let Some(s) = &e.stroke {
                body.line(4, &format!("stroke {}", quote(s)));
            }
            if let Some(fs) = e.font_size {
                body.line(4, &format!("fontSize {}", fs));
            }
            if let Some(b) = &e.border {
                body.line(4, &format!("border {}", b));
            }
            if let Some(o) = e.opacity {
                body.line(4, &format!("opacity {}", o));
            }
            if let Some(w) = e.width {
                body.line(4, &format!("width {}", w));
            }
            if let Some(h) = e.height {
                body.line(4, &format!("height {}", h));
            }
            let head = format!("element {}", quote(&e.tag));
            emit_block(out, 3, &head, false, &body.buf);
        }
        for r in styles.relationships.iter().flatten() {
            let mut body = Out::new();
            if let Some(t) = r.thickness {
                body.line(4, &format!("thickness {}", t));
            }
            if let Some(c) = &r.color {
                body.line(4, &format!("color {}", quote(c)));
            }
            if let Some(fs) = r.font_size {
                body.line(4, &format!("fontSize {}", fs));
            }
            if let Some(ls) = &r.line_style {
                body.line(4, &format!("lineStyle {}", ls));
            }
            if let Some(routing) = &r.routing {
                body.line(4, &format!("routing {}", routing));
            }
            if let Some(o) = r.opacity {
                body.line(4, &format!("opacity {}", o));
            }
            if let Some(d) = r.dashed {
                body.line(4, &format!("dashed {}", d));
            }
            if let Some(p) = r.position {
                body.line(4, &format!("position {}", p));
            }
            let head = format!("relationship {}", quote(&r.tag));
            emit_block(out, 3, &head, false, &body.buf);
        }
        out.line(2, "}");
    }
}

/// One model-level declaration (person/softwareSystem/custom element).
enum ModelLeaf<'a> {
    Person(&'a Person),
    SoftwareSystem(&'a SoftwareSystem),
    CustomElement(&'a CustomElement),
}

/// Emit `head` followed by `{ body }` if `body` is non-empty, else just
/// `head` with the uncertainty marker. `body` is already-rendered, indented
/// lines (with trailing newline), or empty.
fn emit_block(out: &mut Out, indent: usize, head: &str, uncertain: bool, body: &str) {
    if body.is_empty() {
        out.line(indent, &format!("{}{}", head, if uncertain { " ?" } else { "" }));
    } else {
        if uncertain {
            out.line(indent, &format!("{} ? {{", head));
        } else {
            out.line(indent, &format!("{} {{", head));
        }
        out.buf.push_str(body);
        out.line(indent, "}");
    }
}

fn bare_or_quote(s: &str) -> String {
    let simple = !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.');
    if simple {
        s.to_string()
    } else {
        quote(s)
    }
}

#[cfg(test)]
mod camel_tests {
    use super::camel_from_name;

    #[test]
    fn acronyms_are_cased_as_words() {
        assert_eq!(camel_from_name("API"), "api");
        assert_eq!(camel_from_name("Customer REST API"), "customerRestApi");
        assert_eq!(camel_from_name("Web App"), "webApp");
        assert_eq!(camel_from_name("E-mail System"), "eMailSystem");
        assert_eq!(camel_from_name("iOS App"), "iOSApp");
        assert_eq!(camel_from_name("S3"), "s3");
    }
}
