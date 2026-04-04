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
    crate::db::query::find_symbol_by_name(db, name)?
        .ok_or_else(|| anyhow::anyhow!("Symbol not found: {}", name))
}
