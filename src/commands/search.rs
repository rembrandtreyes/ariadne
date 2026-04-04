use console::style;

use super::require_db;

pub fn cmd_search(query: &str, json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;

    let options = crate::search::SearchOptions {
        limit: Some(20),
        fuzzy: true,
        ..Default::default()
    };

    let results = crate::search::search(&db, query, &options)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }

    if results.is_empty() {
        println!("No symbols found matching \"{}\"", query);
        return Ok(());
    }

    println!(
        "{} results for \"{}\":\n",
        style(results.len()).cyan(),
        style(query).bold()
    );

    for r in &results {
        let kind = style(&r.kind).dim();
        let name = style(&r.name).bold();
        let file = style(&r.file).dim();
        let line = style(r.line).dim();
        println!("  {kind:<12} {name:<40} {file}:{line}");
    }

    Ok(())
}
