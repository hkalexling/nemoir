use nemoir_ir::WorkflowIr;
use serde::Serialize;

#[derive(Serialize)]
struct WfMeta {
    id: String,
    entry: String,
    exits: Vec<String>,
    frontend: String,
    file: String,
    #[serde(rename = "irVersion")]
    ir_version: String,
    selection: String,
    #[serde(rename = "noMatch")]
    no_match: String,
    inputs: Vec<InputEntry>,
    capabilities: Vec<String>,
    #[serde(rename = "policyCount")]
    policy_count: usize,
}

#[derive(Serialize)]
struct InputEntry {
    id: String,
    #[serde(rename = "type")]
    ty: String,
}

pub fn generate_html(ir: &WorkflowIr, title: &str, graph_data: &serde_json::Value) -> String {
    let graph_json = serde_json::to_string(graph_data).unwrap_or_default();
    let graph_json_escaped = escape_script_json(&graph_json);

    let meta = WfMeta {
        id: ir.workflow.id.clone(),
        entry: ir.workflow.entry.clone(),
        exits: ir.workflow.exits.clone(),
        frontend: ir.source.frontend.clone(),
        file: ir.source.file.clone(),
        ir_version: ir.ir_version.clone(),
        selection: ir.workflow.transition_semantics.selection.clone(),
        no_match: ir.workflow.transition_semantics.no_match.clone(),
        inputs: ir
            .inputs
            .iter()
            .map(|inp| InputEntry {
                id: inp.id.clone(),
                ty: inp.ty.clone(),
            })
            .collect(),
        capabilities: ir.capabilities.clone(),
        policy_count: ir.policies.len(),
    };

    let meta_json = serde_json::to_string(&meta).unwrap_or_default();
    let meta_json_escaped = escape_script_json(&meta_json);

    let workflow_title = html_escape(title);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{workflow_title}</title>
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
html, body {{ width: 100%; height: 100%; overflow: hidden; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; }}
body {{ display: flex; flex-direction: column; }}
#header {{
  flex-shrink: 0;
  padding: 8px 16px;
  border-bottom: 1px solid #ddd;
  display: flex;
  align-items: center;
  gap: 12px;
  background: #fafafa;
  flex-wrap: wrap;
}}
#header h1 {{ font-size: 16px; font-weight: 600; }}
#header .meta {{ font-size: 12px; color: #666; }}
#header .controls {{ display: flex; gap: 6px; margin-left: auto; }}
#header button {{
  padding: 4px 10px;
  font-size: 12px;
  border: 1px solid #ccc;
  border-radius: 4px;
  background: #fff;
  cursor: pointer;
}}
#header button:hover {{ background: #eee; }}
#header input {{
  padding: 4px 8px;
  font-size: 12px;
  border: 1px solid #ccc;
  border-radius: 4px;
  width: 140px;
}}
#main {{ display: flex; flex: 1; min-height: 0; }}
#cy {{ flex: 1; min-width: 0; }}
#inspector {{
  flex-shrink: 0;
  width: 320px;
  border-left: 1px solid #ddd;
  overflow-y: auto;
  padding: 12px;
  font-size: 13px;
  background: #fafafa;
}}
#inspector h3 {{ font-size: 14px; margin-bottom: 8px; color: #333; }}
#inspector .section {{ margin-bottom: 12px; }}
#inspector .section-label {{ font-weight: 600; color: #555; font-size: 11px; text-transform: uppercase; margin-bottom: 4px; }}
#inspector .section-content {{ font-size: 12px; color: #333; white-space: pre-wrap; word-break: break-word; }}
#inspector .badge {{
  display: inline-block;
  padding: 1px 6px;
  border-radius: 3px;
  font-size: 10px;
  font-weight: 600;
  margin-right: 4px;
}}
.badge-entry {{ background: #d4edda; color: #155724; }}
.badge-exit {{ background: #f8d7da; color: #721c24; }}
#inspector .read-item, #inspector .write-item, #inspector .cap-item, #inspector .trans-item {{
  padding: 2px 0;
  border-bottom: 1px solid #eee;
  font-size: 11px;
}}
#inspector .optional {{ color: #888; }}
#cdnerror {{ display: none; padding: 20px; color: #721c24; background: #f8d7da; text-align: center; }}
</style>
</head>
<body>

<div id="header">
  <h1 id="wfTitle">{workflow_title}</h1>
  <span class="meta">IR v{meta_ir_version} | source: {meta_frontend}</span>
  <div class="controls">
    <input type="text" id="searchInput" placeholder="Search nodes...">
    <button id="btnLayout">Reset Layout</button>
    <button id="btnFit">Fit</button>
  </div>
</div>

<div id="main">
  <div id="cy"></div>
  <div id="inspector"></div>
</div>

<div id="cdnerror">Cytoscape.js failed to load from CDN. Please check your network connection.</div>

<script src="https://cdn.jsdelivr.net/npm/cytoscape@3.30.4/dist/cytoscape.min.js"></script>
<script type="text/javascript">

window.addEventListener('load', function() {{
  if (typeof cytoscape === 'undefined') {{
    document.getElementById('cdnerror').style.display = 'block';
    return;
  }}
  initVisualizer();
}});
</script>

<script id="graphdata" type="application/json">{graph_json_escaped}</script>
<script id="wfmetadata" type="application/json">{meta_json_escaped}</script>

<script>
function initVisualizer() {{
var graphData = JSON.parse(document.getElementById('graphdata').textContent);
var WF_META = JSON.parse(document.getElementById('wfmetadata').textContent);

var STORAGE_KEY = 'nemoir_positions_' + encodeURIComponent(WF_META.id + '_' + WF_META.file);

var edgeStyleByReason = {{
  'fallthrough': {{ color: '#888', style: 'solid' }},
  'output_branch_true': {{ color: '#28a745', style: 'solid' }},
  'output_branch_false': {{ color: '#dc3545', style: 'solid' }},
  'next_stage_required_input_available': {{ color: '#4a90d9', style: 'solid' }},
  'skip_next_stage_required_input_missing': {{ color: '#6c757d', style: 'dashed' }},
  'backward_ref_loop': {{ color: '#8e44ad', style: 'solid' }}
}};

function reasonStyle(reason) {{
  var s = edgeStyleByReason[reason];
  return s || {{ color: '#888', style: 'solid' }};
}}

var cy = cytoscape({{
  container: document.getElementById('cy'),
  elements: graphData,
  style: [
    {{
      selector: 'node',
      style: {{
        'label': 'data(label)',
        'text-valign': 'center',
        'text-halign': 'center',
        'font-size': '12px',
        'font-weight': '600',
        'width': 60,
        'height': 60,
        'shape': 'roundrectangle',
        'background-color': '#e8e8e8',
        'border-width': 2,
        'border-color': '#aaa',
        'color': '#333',
        'text-wrap': 'wrap',
        'text-max-width': '80px'
      }}
    }},
    {{
      selector: 'node[?isEntry]',
      style: {{
        'border-color': '#28a745',
        'border-width': 3,
        'background-color': '#d4edda'
      }}
    }},
    {{
      selector: 'node[?isExit]',
      style: {{
        'border-color': '#dc3545',
        'border-width': 3,
        'background-color': '#f8d7da'
      }}
    }},
    {{
      selector: 'node:selected',
      style: {{
        'border-color': '#0056d2',
        'border-width': 4
      }}
    }},
    {{
      selector: 'edge',
      style: {{
        'width': 2,
        'line-color': '#888',
        'target-arrow-color': '#888',
        'target-arrow-shape': 'triangle',
        'curve-style': 'bezier',
        'label': 'data(label)',
        'font-size': '9px',
        'color': '#555',
        'text-rotation': 'autorotate',
        'text-margin-y': -6
      }}
    }},
    {{
      selector: 'edge:selected',
      style: {{
        'width': 4,
        'line-color': '#0056d2',
        'target-arrow-color': '#0056d2'
      }}
    }}
  ],
  layout: {{ name: 'breadthfirst', directed: true, spacingFactor: 1.5 }}
}});

cy.ready(function() {{
  var saved = localStorage.getItem(STORAGE_KEY);
  if (saved) {{
    try {{
      var positions = JSON.parse(saved);
      cy.nodes().forEach(function(n) {{
        var pos = positions[n.id()];
        if (pos) {{ n.position(pos); }}
      }});
    }} catch(e) {{}}
  }}
}});

cy.on('layoutstop', savePositions);
cy.on('dragfree', 'node', savePositions);

function savePositions() {{
  var positions = {{}};
  cy.nodes().forEach(function(n) {{
    positions[n.id()] = n.position();
  }});
  localStorage.setItem(STORAGE_KEY, JSON.stringify(positions));
}}

cy.on('select unselect', function() {{
  updateInspector();
}});

cy.on('click', function(evt) {{
  if (evt.target === cy) {{
    updateInspector();
  }}
}});

var currentFilter = '';

function applyFilter(q) {{
  q = (q || '').toLowerCase().trim();
  currentFilter = q;
  if (!q) {{
    cy.elements().style('opacity', 1);
    cy.nodes().style('opacity', 1);
    return;
  }}
  cy.elements().style('opacity', 1);
  var matched = cy.nodes().filter(function(n) {{
    var label = (n.data('label') || '').toLowerCase();
    var prompt = (n.data('prompt') || '').toLowerCase();
    var id = (n.data('id') || '').toLowerCase();
    return label.indexOf(q) >= 0 || prompt.indexOf(q) >= 0 || id.indexOf(q) >= 0;
  }});
  var allNodes = cy.nodes();
  allNodes.style('opacity', 0.2);
  matched.style('opacity', 1);
  matched.connectedEdges().style('opacity', 1);
  matched.connectedEdges().connectedNodes().style('opacity', 1);
}}

document.getElementById('searchInput').addEventListener('input', function() {{
  applyFilter(this.value);
}});

document.getElementById('btnLayout').addEventListener('click', function() {{
  localStorage.removeItem(STORAGE_KEY);
  cy.layout({{ name: 'breadthfirst', directed: true, spacingFactor: 1.5 }}).run();
}});

document.getElementById('btnFit').addEventListener('click', function() {{
  cy.fit(undefined, 30);
}});

function escContentForInner(s) {{
  if (!s) return '';
  var div = document.createElement('div');
  div.textContent = s;
  return div.innerHTML;
}}

function updateInspector() {{
  var sel = cy.nodes(':selected');
  var sEdge = cy.edges(':selected');
  var container = document.getElementById('inspector');

  if (sel.length === 1) {{
    showNodeInspector(container, sel[0]);
  }} else if (sEdge.length === 1) {{
    showEdgeInspector(container, sEdge[0]);
  }} else {{
    showWorkflowInspector(container);
  }}
}}

function showWorkflowInspector(el) {{
  var inputsStr = WF_META.inputs.map(function(inp) {{ return inp.id + ': ' + inp.type; }}).join(', ');
  var capsStr = (WF_META.capabilities || []).join(', ');
  el.innerHTML =
    '<h3>Workflow: ' + escContentForInner(WF_META.id) + '</h3>' +
    '<div class="section"><div class="section-label">Source</div>' +
    '<div class="section-content">Frontend: ' + escContentForInner(WF_META.frontend) + '\\nFile: ' + escContentForInner(WF_META.file) + '</div></div>' +
    '<div class="section"><div class="section-label">IR Version</div>' +
    '<div class="section-content">' + escContentForInner(WF_META.irVersion) + '</div></div>' +
    '<div class="section"><div class="section-label">Entry</div>' +
    '<div class="section-content">' + escContentForInner(WF_META.entry) + '</div></div>' +
    '<div class="section"><div class="section-label">Exits</div>' +
    '<div class="section-content">' + escContentForInner((WF_META.exits || []).join(', ')) + '</div></div>' +
    '<div class="section"><div class="section-label">Transition Semantics</div>' +
    '<div class="section-content">Selection: ' + escContentForInner(WF_META.selection) + '\\nNo Match: ' + escContentForInner(WF_META.noMatch) + '</div></div>' +
    '<div class="section"><div class="section-label">Inputs</div>' +
    '<div class="section-content">' + escContentForInner(inputsStr) + '</div></div>' +
    '<div class="section"><div class="section-label">Capabilities</div>' +
    '<div class="section-content">' + escContentForInner(capsStr) + '</div></div>' +
    '<div class="section"><div class="section-label">Policies</div>' +
    '<div class="section-content">' + WF_META.policyCount + ' policies</div></div>';
}}

function showNodeInspector(el, node) {{
  var d = node.data();
  var badges = [];
  if (d.isEntry) badges.push('<span class="badge badge-entry">ENTRY</span>');
  if (d.isExit) badges.push('<span class="badge badge-exit">EXIT</span>');

  var annotationsStr = (d.annotations || []).join(', ');
  var promptStr = escContentForInner(d.prompt || '');

  var readsHtml = (d.reads || []).map(function(r) {{
    var refStr = '';
    if (r['ref'] && r['ref'].kind === 'input') refStr = 'input:' + escContentForInner(r['ref'].name);
    else if (r['ref'] && r['ref'].kind === 'node_output') refStr = escContentForInner(r['ref'].node) + '.' + escContentForInner(r['ref'].field);
    else refStr = JSON.stringify(r['ref']);
    var opt = r.optional ? ' <span class="optional">opt</span>' : '';
    return '<div class="read-item">' + refStr + opt + ' (' + escContentForInner(r.origin || '') + ')</div>';
  }}).join('');

  var writesHtml = (d.writes || []).map(function(w) {{
    var opt = w.optional ? ' <span class="optional">opt</span>' : '';
    return '<div class="write-item">' + escContentForInner(w.name) + ': ' + escContentForInner(w.type) + opt + '</div>';
  }}).join('');

  var capsHtml = (d.requires || []).map(function(c) {{
    return '<div class="cap-item">' + escContentForInner(c.capability) + '</div>';
  }}).join('');

  var transHtml = (d.transitions || []).map(function(t) {{
    return '<div class="trans-item">[' + t.priority + '] ' + escContentForInner(t.to) + ' — ' + escContentForInner(t.reason) + ' — ' + escContentForInner(t.guard_summary) + '</div>';
  }}).join('');

  el.innerHTML =
    '<h3>State: ' + escContentForInner(d.id) + '</h3>' +
    '<div class="section">' + badges.join(' ') + '</div>' +
    (annotationsStr ? '<div class="section"><div class="section-label">Annotations</div><div class="section-content">' + escContentForInner(annotationsStr) + '</div></div>' : '') +
    '<div class="section"><div class="section-label">Prompt</div><div class="section-content">' + promptStr + '</div></div>' +
    '<div class="section"><div class="section-label">Reads</div><div class="section-content">' + (readsHtml || 'none') + '</div></div>' +
    '<div class="section"><div class="section-label">Writes</div><div class="section-content">' + (writesHtml || 'none') + '</div></div>' +
    '<div class="section"><div class="section-label">Required Capabilities</div><div class="section-content">' + (capsHtml || 'none') + '</div></div>' +
    '<div class="section"><div class="section-label">Outgoing Transitions</div><div class="section-content">' + (transHtml || 'none') + '</div></div>';
}}

function showEdgeInspector(el, edge) {{
  var d = edge.data();
  var guardJson = JSON.stringify(d.guard, null, 2);
  el.innerHTML =
    '<h3>Transition</h3>' +
    '<div class="section"><div class="section-label">Source</div><div class="section-content">' + escContentForInner(d.source) + '</div></div>' +
    '<div class="section"><div class="section-label">Target</div><div class="section-content">' + escContentForInner(d.target) + '</div></div>' +
    '<div class="section"><div class="section-label">Priority</div><div class="section-content">' + d.priority + '</div></div>' +
    '<div class="section"><div class="section-label">Reason</div><div class="section-content">' + escContentForInner(d.reason) + '</div></div>' +
    '<div class="section"><div class="section-label">Guard Summary</div><div class="section-content">' + escContentForInner(d.guardSummary) + '</div></div>' +
    '<div class="section"><div class="section-label">Raw Guard</div><div class="section-content"><pre style="font-size:11px; max-height:200px; overflow:auto;">' + escContentForInner(guardJson) + '</pre></div></div>';
}}

// Apply reason-based edge styling
cy.edges().forEach(function(edge) {{
  var reason = edge.data('reason');
  var s = reasonStyle(reason);
  edge.style('line-color', s.color);
  edge.style('target-arrow-color', s.color);
  edge.style('color', s.color);
  if (s.style === 'dashed') {{
    edge.style('line-style', 'dashed');
    edge.style('line-color', s.color);
    edge.style('target-arrow-color', s.color);
  }}
  if (reason === 'backward_ref_loop') {{
    edge.style('curve-style', 'bezier');
    edge.style('control-point-step-size', 40);
  }}
}});

updateInspector();
}}
</script>
</body>
</html>"#,
        meta_ir_version = html_escape(&ir.ir_version),
        meta_frontend = html_escape(&ir.source.frontend),
    )
}

fn escape_script_json(s: &str) -> String {
    s.replace('<', "\\u003c")
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
