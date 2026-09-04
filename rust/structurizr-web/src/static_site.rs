//! Portable static HTML export for a workspace.

use std::path::Path;

use anyhow::{Context, Result};
use structurizr_model::Workspace;
use structurizr_renderer::{exporter::DiagramExporter, svg::SvgExporter};

/// Write a self-contained, browsable workspace artifact to `output`.
///
/// The artifact contains an overview, one SVG per supported diagram, and an
/// interactive relationship graph. It needs no web server or third-party CDN.
pub fn export(workspace: &Workspace, output: &Path) -> Result<()> {
    std::fs::create_dir_all(output)
        .with_context(|| format!("Cannot create output dir {}", output.display()))?;

    let diagrams = SvgExporter.export_workspace(workspace);
    let mut diagram_links = String::new();
    for diagram in &diagrams {
        let filename = format!("{}.svg", safe_filename(&diagram.key));
        std::fs::write(output.join(&filename), &diagram.content)
            .with_context(|| format!("Cannot write {}", output.join(&filename).display()))?;
        diagram_links.push_str(&format!(
            r#"<li><a href="{filename}">{}</a></li>"#,
            html_escape(&diagram.key)
        ));
    }

    let title = html_escape(&workspace.name);
    let description = workspace.description.as_deref().map(html_escape).unwrap_or_default();
    let index = format!(
        r#"<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title} – StructurizrX</title>
<style>{}</style>
<main><header><h1>{title}</h1><p>{description}</p></header>
<nav><a href="graph.html">Relationship graph</a></nav>
<h2>Diagrams</h2><ul>{diagram_links}</ul></main></html>"#,
        base_css()
    );
    std::fs::write(output.join("index.html"), index)
        .with_context(|| format!("Cannot write {}", output.join("index.html").display()))?;

    let workspace_json = serde_json::to_string(workspace)?.replace('<', "\\u003c");
    let graph = format!(
        r#"<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title} relationship graph – StructurizrX</title>
<style>{} #graph{{width:100%;height:calc(100vh - 130px);border:1px solid #ddd}} .node{{cursor:pointer}} text{{pointer-events:none;font:12px sans-serif}}</style>
<main><nav><a href="index.html">← Overview</a></nav><h1>{title} relationship graph</h1>
<p>Drag nodes to explore relationships. Scroll to zoom and drag the background to pan.</p>
<svg id="graph" aria-label="Interactive relationship graph"></svg></main>
<script id="workspace-data" type="application/json">{workspace_json}</script>
<script>{}</script></html>"#,
        base_css(),
        graph_script()
    );
    std::fs::write(output.join("graph.html"), graph)
        .with_context(|| format!("Cannot write {}", output.join("graph.html").display()))?;
    Ok(())
}

fn safe_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') { c } else { '_' })
        .collect()
}

fn html_escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn base_css() -> &'static str {
    "body{margin:0;font:16px system-ui,sans-serif;color:#212529;background:#f8f9fa}main{max-width:1100px;margin:auto;padding:2rem}a{color:#0d6efd;text-decoration:none}a:hover{text-decoration:underline}header,nav{margin-bottom:1.5rem}li{margin:.5rem 0}"
}

fn graph_script() -> &'static str {
    r#"(() => {
const ws=JSON.parse(document.getElementById('workspace-data').textContent), svg=document.getElementById('graph');
const ns='http://www.w3.org/2000/svg', elements=[], links=[], byId=new Map();
function walk(items){(items||[]).forEach(x=>{elements.push(x);byId.set(x.id,x);walk(x.containers);walk(x.components)})}
walk(ws.model.people);walk(ws.model.softwareSystems);walk(ws.model.customElements);
function relationships(items){(items||[]).forEach(x=>{(x.relationships||[]).forEach(r=>links.push(r));relationships(x.containers);(x.containers||[]).forEach(c=>relationships(c.components))})}
relationships(ws.model.people);relationships(ws.model.softwareSystems);relationships(ws.model.customElements);
const nodes=elements.map((x,i)=>({id:x.id,name:x.name||x.id,x:120+(i%6)*150,y:100+Math.floor(i/6)*110}));
const nodeById=new Map(nodes.map(n=>[n.id,n])), w=()=>svg.clientWidth,h=()=>svg.clientHeight;
let transform={x:0,y:0,k:1}, drag, pan;
const root=document.createElementNS(ns,'g');svg.append(root);
function render(){root.setAttribute('transform',`translate(${transform.x} ${transform.y}) scale(${transform.k})`);root.replaceChildren();
links.forEach(l=>{let a=nodeById.get(l.sourceId),b=nodeById.get(l.destinationId);if(!a||!b)return;let e=document.createElementNS(ns,'line');e.setAttribute('x1',a.x);e.setAttribute('y1',a.y);e.setAttribute('x2',b.x);e.setAttribute('y2',b.y);e.setAttribute('stroke','#9aa0a6');e.setAttribute('stroke-width','1.5');root.append(e)});
nodes.forEach(n=>{let g=document.createElementNS(ns,'g');g.classList.add('node');g.setAttribute('transform',`translate(${n.x} ${n.y})`);let c=document.createElementNS(ns,'circle');c.setAttribute('r','24');c.setAttribute('fill','#438dd5');let t=document.createElementNS(ns,'text');t.setAttribute('text-anchor','middle');t.setAttribute('y','42');t.textContent=n.name;g.append(c,t);g.onpointerdown=e=>{e.stopPropagation();drag={n,x:e.clientX,y:e.clientY};svg.setPointerCapture(e.pointerId)};root.append(g)})}
svg.onpointerdown=e=>pan={x:e.clientX,y:e.clientY,tx:transform.x,ty:transform.y};
svg.onpointermove=e=>{if(drag){drag.n.x+=(e.clientX-drag.x)/transform.k;drag.n.y+=(e.clientY-drag.y)/transform.k;drag.x=e.clientX;drag.y=e.clientY;render()}else if(pan){transform.x=pan.tx+e.clientX-pan.x;transform.y=pan.ty+e.clientY-pan.y;render()}};
svg.onpointerup=()=>{drag=pan=null};svg.onwheel=e=>{e.preventDefault();transform.k=Math.max(.3,Math.min(3,transform.k*(e.deltaY<0?1.1:.9)));render()};render();
})()"#
}
