pub mod tools;

/// Start the MCP server on stdio using rmcp.
///
/// Loads the .ariadne.db from the current working directory and serves
/// all Ariadne tools over the MCP stdio transport.
pub async fn serve_stdio() -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db_path = root.join(".ariadne.db");

    if !db_path.exists() {
        anyhow::bail!(
            "No index found at {}. Run `ariadne index` first.",
            db_path.display()
        );
    }

    let db = crate::db::Database::open(&db_path)?;
    tracing::info!("Starting Ariadne MCP server on stdio");

    let service = tools::AriadneService::new(db);
    let server = rmcp::ServiceExt::serve(service, rmcp::transport::io::stdio()).await?;
    server.waiting().await?;
    Ok(())
}
