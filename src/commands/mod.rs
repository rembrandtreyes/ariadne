pub mod analysis;
pub mod graph;
pub mod index;
pub mod plugin;
pub mod search;
pub mod serve;

#[cfg(feature = "watch")]
pub mod watch;

use std::path::Path;

pub const DB_FILENAME: &str = ".ariadne.db";

pub fn require_db(root: &Path) -> anyhow::Result<crate::db::Database> {
    let db_path = root.join(DB_FILENAME);
    if !db_path.exists() {
        anyhow::bail!(
            "No index found at {}. Run `ariadne index` first.",
            db_path.display()
        );
    }
    crate::db::Database::open(&db_path)
}

pub fn resolve_symbol(
    db: &crate::db::Database,
    name: &str,
) -> anyhow::Result<crate::db::query::SymbolRow> {
    use crate::db::query::SymbolResolution;
    match crate::db::query::resolve_symbol_by_name(db, name, None)? {
        SymbolResolution::Unique(sym) => Ok(sym),
        SymbolResolution::NotFound => anyhow::bail!("Symbol not found: {}", name),
        SymbolResolution::Ambiguous(candidates) => {
            eprintln!(
                "Ambiguous symbol \"{}\" — {} matches:",
                name,
                candidates.len()
            );
            for c in &candidates {
                eprintln!(
                    "  {}  {}:{}  ({})",
                    c.symbol.qualified_name, c.file_path, c.symbol.line_start, c.symbol.kind
                );
            }
            anyhow::bail!(
                "Ambiguous symbol: \"{}\". Re-run with the qualified name.",
                name
            )
        }
    }
}
