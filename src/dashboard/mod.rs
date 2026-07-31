pub mod api;
pub mod describe;

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
        .route("/api/modules", axum::routing::get(api::modules))
        .route("/api/coupling", axum::routing::get(api::coupling))
        .route("/api/describe", axum::routing::get(api::describe))
        .route("/api/entry_points", axum::routing::get(api::entry_points))
        .route(
            "/api/complexity_hotspots",
            axum::routing::get(api::complexity_hotspots),
        )
        .route("/api/god_objects", axum::routing::get(api::god_objects))
        .route(
            "/api/dependency_path",
            axum::routing::get(api::dependency_path),
        )
        .route(
            "/api/propose_edit_plan",
            axum::routing::get(api::propose_edit_plan),
        )
        .route("/api/overview", axum::routing::get(api::overview))
        .route("/api/cycles", axum::routing::get(api::cycles))
        .route("/api/churn", axum::routing::get(api::churn))
        .route("/api/symbol_search", axum::routing::get(api::symbol_search))
        .route("/next", axum::routing::get(next_ui_root))
        .route("/next/", axum::routing::get(next_ui_root))
        .route("/next/{*path}", axum::routing::get(next_ui_asset))
        .route(
            "/graph-renderer.js",
            axum::routing::get(graph_renderer_js_handler),
        )
        .route("/style.css", axum::routing::get(style_css_handler))
        .route("/signal.js", axum::routing::get(signal_js_handler))
        .route(
            "/void-renderer.js",
            axum::routing::get(void_renderer_js_handler),
        )
        .route(
            "/detail-panel.js",
            axum::routing::get(detail_panel_js_handler),
        )
        .route("/search.js", axum::routing::get(search_js_handler))
        .route(
            "/source-modal.js",
            axum::routing::get(source_modal_js_handler),
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
    let db_result = crate::db::Database::open(db_path.as_ref());
    let db_ok = db_result.is_ok();
    let last_indexed = db_result
        .ok()
        .and_then(|db| db.get_metadata("last_indexed").ok().flatten())
        .unwrap_or_default();

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
            "last_indexed": last_indexed,
        })),
    )
}

async fn index_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(INDEX_HTML)
}

/// The rebuilt dashboard (Svelte + Vite, bun-built at compile time by
/// build.rs into OUT_DIR) — embedded so the binary stays self-contained.
/// Mounted at /next until it reaches capability parity with the legacy UI.
#[derive(rust_embed::RustEmbed)]
#[folder = "$OUT_DIR/dashboard-dist"]
struct NextUiAssets;

fn mime_for(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript",
        Some("css") => "text/css",
        Some("svg") => "image/svg+xml",
        Some("json" | "map") => "application/json",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn serve_next_asset(rel: &str) -> axum::response::Response {
    use axum::response::IntoResponse;
    // The app is a hash router, so any non-asset path resolves to the shell.
    let (resolved, file) = match NextUiAssets::get(rel) {
        Some(f) => (rel, Some(f)),
        None => ("index.html", NextUiAssets::get("index.html")),
    };
    match file {
        Some(f) => {
            // Vite content-hashes everything under assets/, so those may cache
            // forever; the shell must revalidate or stale bundles keep serving
            // after a rebuild (Chrome heuristically caches validator-less 200s).
            let cache = if resolved.starts_with("assets/") {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };
            (
                axum::http::StatusCode::OK,
                [
                    (axum::http::header::CONTENT_TYPE, mime_for(resolved)),
                    (axum::http::header::CACHE_CONTROL, cache),
                ],
                f.data.into_owned(),
            )
                .into_response()
        }
        None => (
            axum::http::StatusCode::NOT_FOUND,
            "New dashboard assets were not embedded in this build.",
        )
            .into_response(),
    }
}

async fn next_ui_root() -> axum::response::Response {
    serve_next_asset("index.html")
}

async fn next_ui_asset(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> axum::response::Response {
    serve_next_asset(&path)
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

async fn style_css_handler() -> (
    axum::http::StatusCode,
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/css")],
        STYLE_CSS,
    )
}

async fn signal_js_handler() -> (
    axum::http::StatusCode,
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        SIGNAL_JS,
    )
}

async fn void_renderer_js_handler() -> (
    axum::http::StatusCode,
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        VOID_RENDERER_JS,
    )
}

async fn detail_panel_js_handler() -> (
    axum::http::StatusCode,
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        DETAIL_PANEL_JS,
    )
}

async fn search_js_handler() -> (
    axum::http::StatusCode,
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        SEARCH_JS,
    )
}

async fn source_modal_js_handler() -> (
    axum::http::StatusCode,
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        SOURCE_MODAL_JS,
    )
}

const INDEX_HTML: &str = include_str!("static/index.html");
const GRAPH_RENDERER_JS: &str = include_str!("static/graph-renderer.js");
const STYLE_CSS: &str = include_str!("static/style.css");
const SIGNAL_JS: &str = include_str!("static/signal.js");
const VOID_RENDERER_JS: &str = include_str!("static/void-renderer.js");
const DETAIL_PANEL_JS: &str = include_str!("static/detail-panel.js");
const SEARCH_JS: &str = include_str!("static/search.js");
const SOURCE_MODAL_JS: &str = include_str!("static/source-modal.js");
