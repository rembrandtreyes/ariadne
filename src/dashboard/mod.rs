pub mod api;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use tower_http::cors::CorsLayer;

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

    let origin_127 = format!("http://127.0.0.1:{}", config.port)
        .parse::<axum::http::HeaderValue>()
        .expect("valid origin");
    let origin_localhost = format!("http://localhost:{}", config.port)
        .parse::<axum::http::HeaderValue>()
        .expect("valid origin");

    let cors = CorsLayer::new()
        .allow_origin([origin_127, origin_localhost])
        .allow_methods([axum::http::Method::GET])
        .allow_headers([axum::http::header::CONTENT_TYPE]);

    let app = axum::Router::new()
        .route("/api/health", axum::routing::get(health_handler))
        .route("/api/stats", axum::routing::get(api::stats))
        .route("/api/graph", axum::routing::get(api::graph_data))
        .route("/api/search", axum::routing::get(api::search_symbols))
        .route(
            "/api/graph/neighborhood",
            axum::routing::get(api::neighborhood),
        )
        .route("/api/graph/insights", axum::routing::get(api::insights))
        .route("/api/source", axum::routing::get(api::source))
        .route(
            "/graph-renderer.js",
            axum::routing::get(graph_renderer_js_handler),
        )
        .route("/v1", axum::routing::get(variant1_handler))
        .route("/v2", axum::routing::get(variant2_handler))
        .route("/v3", axum::routing::get(variant3_handler))
        .fallback(axum::routing::get(index_handler))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("Dashboard running at http://{}", addr);
    println!("  Variant 1 (Command Center): http://{}/v1", addr);
    println!("  Variant 2 (Navigator):      http://{}/v2", addr);
    println!("  Variant 3 (Audit Report):   http://{}/v3", addr);
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

async fn graph_renderer_js_handler() -> (
    axum::http::StatusCode,
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        GRAPH_RENDERER_JS,
    )
}

async fn variant1_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(VARIANT_1_HTML)
}

async fn variant2_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(VARIANT_2_HTML)
}

async fn variant3_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(VARIANT_3_HTML)
}

const INDEX_HTML: &str = include_str!("static/index.html");
const GRAPH_RENDERER_JS: &str = include_str!("static/graph-renderer.js");
const VARIANT_1_HTML: &str = include_str!("static/variant-1-command-center.html");
const VARIANT_2_HTML: &str = include_str!("static/variant-2-navigator.html");
const VARIANT_3_HTML: &str = include_str!("static/variant-3-audit-report.html");
