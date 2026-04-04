use console::style;

use super::{require_db, DB_FILENAME};

pub fn cmd_topology(json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;
    let conn = db.conn();

    #[derive(serde::Serialize)]
    struct ServiceEdge {
        from: String,
        to: String,
        protocol: String,
        call_count: i32,
        confidence: f64,
    }

    let mut stmt = conn.prepare(
        "SELECT sf.name, st.name, se.protocol, se.call_count, se.confidence
         FROM service_edges se
         JOIN services sf ON se.from_service_id = sf.id
         JOIN services st ON se.to_service_id = st.id
         ORDER BY se.call_count DESC",
    )?;

    let edges: Vec<ServiceEdge> = stmt
        .query_map([], |row| {
            Ok(ServiceEdge {
                from: row.get(0)?,
                to: row.get(1)?,
                protocol: row.get(2)?,
                call_count: row.get(3)?,
                confidence: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // Also get all services for the diagram
    let mut svc_stmt = conn.prepare("SELECT name FROM services ORDER BY name")?;
    let services: Vec<String> = svc_stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    if services.is_empty() {
        println!("No services found. Run `ariadne index` first.");
        return Ok(());
    }

    if json {
        #[derive(serde::Serialize)]
        struct TopologyResult {
            services: Vec<String>,
            edges: Vec<ServiceEdge>,
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&TopologyResult {
                services: services.clone(),
                edges,
            })?
        );
    } else {
        println!("graph LR");
        for svc in &services {
            // Sanitize name for Mermaid
            let safe = svc.replace(['-', ' '], "_");
            println!("    {}[{}]", safe, svc);
        }
        for edge in &edges {
            let from = edge.from.replace(['-', ' '], "_");
            let to = edge.to.replace(['-', ' '], "_");
            println!(
                "    {} -->|{} ({})| {}",
                from, edge.protocol, edge.call_count, to
            );
        }
    }

    Ok(())
}

pub async fn cmd_dash(port: u16) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db_path = root.join(DB_FILENAME);
    if !db_path.exists() {
        anyhow::bail!("No index found. Run `ariadne index` first.");
    }

    let config = crate::dashboard::DashboardConfig { port };
    crate::dashboard::serve(config, &db_path).await
}

pub fn cmd_export_scip(output: &std::path::Path) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;

    crate::analysis::scip_export::export_scip(&db, output, &root)?;

    println!(
        "{} Exported SCIP index to {}",
        style("✓").green().bold(),
        output.display()
    );

    Ok(())
}
