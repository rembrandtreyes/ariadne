pub mod api;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use tower_http::cors::{AllowOrigin, CorsLayer};

/// Configuration for the web dashboard server.
pub struct DashboardConfig {
    pub port: u16,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self { port: 1337 }
    }
}

/// Start the web dashboard server.
///
/// Serves an embedded single-page application with interactive
/// graph visualization, search, and analysis tools.
pub async fn serve(config: DashboardConfig, db_path: &Path) -> anyhow::Result<()> {
    let state: api::DbState = Arc::new(db_path.to_path_buf());

    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));

    let cors = CorsLayer::new().allow_origin(AllowOrigin::exact(
        format!("http://127.0.0.1:{}", config.port)
            .parse()
            .expect("valid origin"),
    ));

    let app = axum::Router::new()
        .route("/api/health", axum::routing::get(health_handler))
        .route("/api/stats", axum::routing::get(api::stats))
        .route("/api/graph", axum::routing::get(api::graph_data))
        .route("/api/search", axum::routing::get(api::search_symbols))
        .fallback(axum::routing::get(index_handler))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("Dashboard running at http://{}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_handler() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn index_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(INDEX_HTML)
}

const INDEX_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Ariadne - Dependency Graph Explorer</title>
    <script src="https://d3js.org/d3.v7.min.js"></script>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0d1117; color: #c9d1d9; }
        .header { padding: 1rem 2rem; border-bottom: 1px solid #30363d; display: flex; align-items: center; gap: 1rem; }
        .header h1 { font-size: 1.5rem; color: #58a6ff; }
        .search { padding: 0.5rem 1rem; border: 1px solid #30363d; border-radius: 6px; background: #161b22; color: #c9d1d9; font-size: 0.9rem; width: 300px; }
        .main { display: flex; height: calc(100vh - 60px); }
        .sidebar { width: 280px; border-right: 1px solid #30363d; padding: 1rem; overflow-y: auto; flex-shrink: 0; }
        .stat-card { background: #161b22; border: 1px solid #30363d; border-radius: 6px; padding: 1rem; margin-bottom: 0.75rem; }
        .stat-card h3 { font-size: 0.75rem; color: #8b949e; text-transform: uppercase; letter-spacing: 0.05em; margin-bottom: 0.25rem; }
        .stat-value { font-size: 1.5rem; font-weight: bold; color: #58a6ff; }
        .stat-sub { font-size: 0.75rem; color: #8b949e; }
        .lang-list { list-style: none; padding: 0; }
        .lang-list li { padding: 0.25rem 0; font-size: 0.85rem; color: #c9d1d9; }
        .lang-list li::before { content: "●"; margin-right: 0.5rem; color: #58a6ff; }
        #graph-container { flex: 1; position: relative; overflow: hidden; }
        svg { width: 100%; height: 100%; }
        .node circle { stroke: #30363d; stroke-width: 1.5; cursor: pointer; }
        .node text { font-size: 10px; fill: #8b949e; pointer-events: none; }
        .link { stroke: #30363d; stroke-opacity: 0.4; }
        .tooltip { position: absolute; background: #161b22; border: 1px solid #30363d; border-radius: 6px; padding: 0.75rem; font-size: 0.8rem; pointer-events: none; display: none; z-index: 10; max-width: 300px; }
        .tooltip .name { font-weight: bold; color: #58a6ff; }
        .tooltip .detail { color: #8b949e; margin-top: 0.25rem; }
        .search-results { position: absolute; top: 52px; left: 300px; background: #161b22; border: 1px solid #30363d; border-radius: 6px; max-height: 300px; overflow-y: auto; display: none; width: 300px; z-index: 20; }
        .search-results .result { padding: 0.5rem 1rem; cursor: pointer; border-bottom: 1px solid #21262d; }
        .search-results .result:hover { background: #21262d; }
        .search-results .result .rname { color: #58a6ff; }
        .search-results .result .rkind { color: #8b949e; font-size: 0.75rem; }
        .legend { display: flex; gap: 1rem; margin-top: 0.5rem; flex-wrap: wrap; }
        .legend-item { display: flex; align-items: center; gap: 0.3rem; font-size: 0.75rem; color: #8b949e; }
        .legend-dot { width: 10px; height: 10px; border-radius: 50%; }
    </style>
</head>
<body>
    <div class="header">
        <h1>Ariadne</h1>
        <input class="search" type="text" id="search" placeholder="Search symbols..." autocomplete="off" />
    </div>
    <div class="search-results" id="searchResults"></div>
    <div class="main">
        <div class="sidebar">
            <div class="stat-card">
                <h3>Files</h3>
                <div class="stat-value" id="files">-</div>
            </div>
            <div class="stat-card">
                <h3>Symbols</h3>
                <div class="stat-value" id="symbols">-</div>
            </div>
            <div class="stat-card">
                <h3>Call Edges</h3>
                <div class="stat-value" id="calls">-</div>
                <div class="stat-sub"><span id="resolution">0</span>% resolved</div>
            </div>
            <div class="stat-card">
                <h3>Dead Functions</h3>
                <div class="stat-value" id="dead">-</div>
            </div>
            <div class="stat-card">
                <h3>Languages</h3>
                <ul class="lang-list" id="languages"></ul>
            </div>
            <div class="stat-card">
                <h3>Legend</h3>
                <div class="legend">
                    <div class="legend-item"><div class="legend-dot" style="background:#58a6ff"></div>function</div>
                    <div class="legend-item"><div class="legend-dot" style="background:#f78166"></div>class</div>
                    <div class="legend-item"><div class="legend-dot" style="background:#7ee787"></div>method</div>
                    <div class="legend-item"><div class="legend-dot" style="background:#d2a8ff"></div>interface</div>
                    <div class="legend-item"><div class="legend-dot" style="background:#8b949e"></div>other</div>
                </div>
            </div>
        </div>
        <div id="graph-container">
            <div class="tooltip" id="tooltip"></div>
        </div>
    </div>
    <script>
    function esc(s) { const d = document.createElement('div'); d.textContent = s; return d.innerHTML; }
    const colors = ['#58a6ff', '#f78166', '#7ee787', '#d2a8ff', '#8b949e'];

    // Load stats
    fetch('/api/stats').then(r => r.json()).then(data => {
        document.getElementById('files').textContent = data.files;
        document.getElementById('symbols').textContent = data.symbols;
        document.getElementById('calls').textContent = data.calls;
        document.getElementById('dead').textContent = data.dead_functions;
        document.getElementById('resolution').textContent = (data.resolution_rate * 100).toFixed(0);
        const langList = document.getElementById('languages');
        langList.innerHTML = '';
        (data.languages || []).forEach(l => {
            const li = document.createElement('li');
            li.textContent = l;
            langList.appendChild(li);
        });
    });

    // Search
    let searchTimeout;
    const searchInput = document.getElementById('search');
    const searchResults = document.getElementById('searchResults');
    searchInput.addEventListener('input', () => {
        clearTimeout(searchTimeout);
        const q = searchInput.value.trim();
        if (q.length < 2) { searchResults.style.display = 'none'; return; }
        searchTimeout = setTimeout(() => {
            fetch('/api/search?q=' + encodeURIComponent(q))
                .then(r => r.json())
                .then(results => {
                    if (results.length === 0) { searchResults.style.display = 'none'; return; }
                    searchResults.innerHTML = results.map(r =>
                        `<div class="result" data-id="${esc(r.id)}"><div class="rname">${esc(r.name)}</div><div class="rkind">${esc(r.kind)} · ${esc(r.file)}</div></div>`
                    ).join('');
                    searchResults.style.display = 'block';
                    searchResults.querySelectorAll('.result').forEach(el => {
                        el.addEventListener('click', () => {
                            highlightNode(el.dataset.id);
                            searchResults.style.display = 'none';
                        });
                    });
                });
        }, 200);
    });
    document.addEventListener('click', (e) => {
        if (!searchResults.contains(e.target) && e.target !== searchInput) searchResults.style.display = 'none';
    });

    // D3 Force Graph
    const container = document.getElementById('graph-container');
    const tooltip = document.getElementById('tooltip');
    const width = container.clientWidth;
    const height = container.clientHeight;

    const svg = d3.select('#graph-container').append('svg')
        .attr('viewBox', [0, 0, width, height]);

    const g = svg.append('g');
    svg.call(d3.zoom().scaleExtent([0.1, 8]).on('zoom', (e) => g.attr('transform', e.transform)));

    let simulation, nodeElements;

    fetch('/api/graph').then(r => r.json()).then(data => {
        if (!data.nodes || data.nodes.length === 0) {
            g.append('text').attr('x', width/2).attr('y', height/2).attr('text-anchor', 'middle').attr('fill', '#8b949e')
                .text('Run "ariadne index ." to populate the graph');
            return;
        }

        const nodeMap = new Map(data.nodes.map(n => [n.id, n]));

        simulation = d3.forceSimulation(data.nodes)
            .force('link', d3.forceLink(data.edges).id(d => d.id).distance(60))
            .force('charge', d3.forceManyBody().strength(-80))
            .force('center', d3.forceCenter(width / 2, height / 2))
            .force('collision', d3.forceCollide().radius(12));

        const link = g.append('g').selectAll('line')
            .data(data.edges).join('line').attr('class', 'link');

        const node = g.append('g').selectAll('g')
            .data(data.nodes).join('g').attr('class', 'node')
            .call(d3.drag().on('start', dragStart).on('drag', dragging).on('end', dragEnd));

        node.append('circle')
            .attr('r', d => d.kind === 'class' ? 8 : 5)
            .attr('fill', d => colors[d.group] || colors[4]);

        node.append('text')
            .attr('dx', 10).attr('dy', 3)
            .text(d => d.name);

        node.on('mouseover', (e, d) => {
            tooltip.innerHTML = `<div class="name">${esc(d.name)}</div><div class="detail">${esc(d.kind)} · ${esc(d.file)}</div>`;
            tooltip.style.display = 'block';
            tooltip.style.left = (e.pageX + 10) + 'px';
            tooltip.style.top = (e.pageY - 30) + 'px';
        }).on('mouseout', () => { tooltip.style.display = 'none'; });

        nodeElements = node;

        simulation.on('tick', () => {
            link.attr('x1', d => d.source.x).attr('y1', d => d.source.y)
                .attr('x2', d => d.target.x).attr('y2', d => d.target.y);
            node.attr('transform', d => `translate(${d.x},${d.y})`);
        });
    });

    function highlightNode(id) {
        if (!nodeElements) return;
        nodeElements.select('circle').attr('stroke', d => d.id === id ? '#f0883e' : '#30363d')
            .attr('stroke-width', d => d.id === id ? 3 : 1.5)
            .attr('r', d => d.id === id ? 12 : (d.kind === 'class' ? 8 : 5));
    }

    function dragStart(e, d) { if (!e.active) simulation.alphaTarget(0.3).restart(); d.fx = d.x; d.fy = d.y; }
    function dragging(e, d) { d.fx = e.x; d.fy = e.y; }
    function dragEnd(e, d) { if (!e.active) simulation.alphaTarget(0); d.fx = null; d.fy = null; }
    </script>
</body>
</html>"##;
