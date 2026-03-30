pub mod api;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use axum::extract::State;
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

    let origin_127 =
        format!("http://127.0.0.1:{}", config.port).parse::<axum::http::HeaderValue>()?;
    let origin_localhost =
        format!("http://localhost:{}", config.port).parse::<axum::http::HeaderValue>()?;

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
        .fallback(axum::routing::get(index_handler))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("Dashboard running at http://{}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_handler(
    State(db_path): State<api::DbState>,
) -> (axum::http::StatusCode, axum::Json<serde_json::Value>) {
    let db_ok = crate::db::Database::open(db_path.as_ref()).is_ok();
    let status = if db_ok {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        axum::Json(serde_json::json!({
            "status": if db_ok { "ok" } else { "error" },
            "version": env!("CARGO_PKG_VERSION"),
            "db": if db_ok { "connected" } else { "unavailable" },
        })),
    )
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

const INDEX_HTML: &str = include_str!("static/index.html");
const GRAPH_RENDERER_JS: &str = include_str!("static/graph-renderer.js");
