//! Portable static HTML export for a workspace.

use std::path::Path;

use anyhow::{Context, Result};
use structurizr_model::Workspace;
use structurizr_renderer::{exporter::DiagramExporter, svg::SvgExporter};

/// Write a single-page SVG technical report to `output`.
pub fn export(workspace: &Workspace, output: &Path) -> Result<()> {
    std::fs::create_dir_all(output)
        .with_context(|| format!("Cannot create output dir {}", output.display()))?;

    let diagrams = SvgExporter.export_workspace(workspace);
    let mut index = String::new();
    let mut content = String::new();
    for diagram in &diagrams {
        let id = safe_filename(&diagram.key);
        index.push_str(&format!(
            r##"<li><a href="#{id}">{}</a></li>"##,
            html_escape(&diagram.key)
        ));
        content.push_str(&format!(
            r#"<section id="{id}"><h2>{}</h2>{}</section>"#,
            html_escape(&diagram.key),
            diagram.content
        ));
    }

    let title = html_escape(&workspace.name);
    let description = workspace.description.as_deref().map(html_escape).unwrap_or_default();
    let report = format!(
        r#"<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title} – StructurizrX</title>
<style>{} section{{margin:3rem 0}}section svg{{display:block;max-width:100%;height:auto;border:1px solid #ddd;background:#fff}}</style>
<main><header><h1>{title}</h1><p>{description}</p></header>
<nav><h2>Index</h2><ol>{index}</ol></nav>{content}</main></html>"#,
        base_css()
    );
    std::fs::write(output.join("index.html"), report)
        .with_context(|| format!("Cannot write {}", output.join("index.html").display()))?;
    Ok(())
}

/// Write a portable viewer application with workspace JSON and bundled assets.
pub fn export_viewer(workspace: &Workspace, output: &Path) -> Result<()> {
    std::fs::create_dir_all(output)
        .with_context(|| format!("Cannot create output dir {}", output.display()))?;
    copy_assets(output)?;
    std::fs::write(
        output.join("workspace.json"),
        serde_json::to_string_pretty(workspace).context("Failed to serialize workspace")?,
    )
    .with_context(|| format!("Cannot write {}", output.join("workspace.json").display()))?;
    std::fs::write(output.join("index.html"), viewer_page(workspace))
        .with_context(|| format!("Cannot write {}", output.join("index.html").display()))?;
    Ok(())
}

fn viewer_page(workspace: &Workspace) -> String {
    include_str!("templates/workspace.html")
        .replace("{{WORKSPACE_NAME}}", &html_escape(&workspace.name))
        .replace("{{WORKSPACE_PATH}}", ".")
        .replace("{{GRAPH_LINK}}", "")
        .replace("{{WORKSPACE_SLUG}}", "")
        .replace("/static/", "static/")
        .replace(
            "fetch('/api/workspace/' + encodeURIComponent(WORKSPACE_SLUG))",
            "fetch('workspace.json')",
        )
}

/// Render the standalone interactive relationship graph page.
pub fn graph_page(workspace: &Workspace, overview_href: &str, asset_prefix: &str) -> Result<String> {
    let title = html_escape(&workspace.name);
    let workspace_json = serde_json::to_string(workspace)?.replace('<', "\\u003c");
    Ok(format!(
        r#"<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title} relationship graph – StructurizrX</title>
<style>{} .graph-layout{{display:flex;gap:1rem;height:calc(100vh - 170px)}}#graph{{flex:1;min-width:0;border:1px solid #ddd}}#node-details{{width:260px;padding:1rem;border:1px solid #ddd;background:#fff;overflow:auto}}#node-details pre{{white-space:pre-wrap;word-break:break-word;font-size:.8rem}}@media(max-width:700px){{.graph-layout{{height:auto;flex-direction:column}}#graph{{height:60vh}}#node-details{{width:auto}}}}</style>
<main><nav><a href="{}">← Overview</a></nav><h1>{title} relationship graph</h1>
<p>Drag nodes to explore relationships. Select a node to copy a reference for an LLM agent.</p>
<div class="graph-layout"><div id="graph" aria-label="Interactive relationship graph"></div><aside id="node-details" aria-live="polite"><strong>Select a node</strong><p>Its ID, name, type, and description can be copied here.</p></aside></div></main>
<script id="workspace-data" type="application/json">{workspace_json}</script>
<script src="{asset_prefix}/jquery-3.7.1.min.js"></script>
<script src="{asset_prefix}/jointjs-Core-4.1.3.js"></script>
<script src="{asset_prefix}/dagre-1.1.8.js"></script>
<script src="{asset_prefix}/graphlib-2.2.4.min.js"></script>
<script src="{asset_prefix}/jointjs-DirectedGraph-4.1.3.min.js"></script>
<script>{}</script></html>"#,
        base_css(),
        html_escape(overview_href),
        graph_script()
    ))
}

fn copy_assets(output: &Path) -> Result<()> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let target = output.join("static");
    for entry in std::fs::read_dir(&source)
        .with_context(|| format!("Cannot read asset dir {}", source.display()))?
    {
        let entry = entry?;
        let target_path = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target_path)?;
        } else {
            std::fs::create_dir_all(&target)?;
            std::fs::copy(entry.path(), target_path)?;
        }
    }
    Ok(())
}

fn copy_dir(source: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let target_path = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target_path)?;
        } else {
            std::fs::copy(entry.path(), target_path)?;
        }
    }
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
const ws=JSON.parse(document.getElementById('workspace-data').textContent), elements=[], links=[];
function walk(items){(items||[]).forEach(x=>{elements.push(x);walk(x.containers);walk(x.components)})}
walk(ws.model.people);walk(ws.model.softwareSystems);walk(ws.model.customElements);
function relationships(items){(items||[]).forEach(x=>{(x.relationships||[]).forEach(r=>links.push(r));relationships(x.containers);(x.containers||[]).forEach(c=>relationships(c.components))})}
relationships(ws.model.people);relationships(ws.model.softwareSystems);relationships(ws.model.customElements);
const graph=new joint.dia.Graph(), paper=new joint.dia.Paper({el:document.getElementById('graph'),model:graph,width:'100%',height:'100%',gridSize:10,drawGrid:true,interactive:true,async:true});
const nodeById=new Map();
function reference(element){return ['Element','ID: '+element.id,'Name: '+(element.name||''),'Type: '+(element.type||''),element.technology?'Technology: '+element.technology:'',element.description?'Description: '+element.description:''].filter(Boolean).join('\n')}
function copy(text){if(navigator.clipboard&&window.isSecureContext)return navigator.clipboard.writeText(text);const area=document.createElement('textarea');area.value=text;area.style.position='fixed';area.style.opacity='0';document.body.append(area);area.select();document.execCommand('copy');area.remove();return Promise.resolve()}
function showDetails(element){const panel=document.getElementById('node-details'), text=reference(element);panel.replaceChildren();const heading=document.createElement('strong');heading.textContent=element.name||element.id;const details=document.createElement('pre');details.textContent=text;const button=document.createElement('button');button.type='button';button.textContent='Copy reference';button.onclick=()=>copy(text).then(()=>{button.textContent='Copied'}).catch(()=>{button.textContent='Copy failed'});panel.append(heading,details,button)}
elements.forEach((element,index)=>{const node=new joint.shapes.standard.Rectangle({position:{x:80+(index%5)*200,y:80+Math.floor(index/5)*120},size:{width:150,height:58},attrs:{body:{fill:'#438dd5',stroke:'#1168bd',rx:6,ry:6},label:{text:element.name||element.id,fill:'#fff',fontSize:13,textWrap:{width:-16,height:-16}}}});nodeById.set(element.id,node);node.set('element',element);graph.addCell(node)});
links.forEach(relationship=>{const source=nodeById.get(relationship.sourceId),target=nodeById.get(relationship.destinationId);if(!source||!target)return;graph.addCell(new joint.shapes.standard.Link({source:{id:source.id},target:{id:target.id},attrs:{line:{stroke:'#6c757d',targetMarker:{type:'path',d:'M 10 -5 0 0 10 5 z'}}}}))});
joint.layout.DirectedGraph.layout(graph,{dagre,graphlib,setVertices:true,nodeSep:50,rankSep:80,marginX:40,marginY:40});
paper.on('cell:pointerclick',cellView=>showDetails(cellView.model.get('element')));
paper.on('blank:pointerdown',(evt,x,y)=>paper.setInteractivity(false));
paper.on('cell:pointerup',()=>paper.setInteractivity(true));
})()"#
}
