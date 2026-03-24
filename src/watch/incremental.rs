use std::path::Path;
use std::time::SystemTime;

use crate::db::Database;

pub fn reindex_files(db: &Database, root: &Path, changed_files: &[&Path]) -> anyhow::Result<()> {
    for file in changed_files {
        if let Some(ext) = file.extension().and_then(|e| e.to_str()) {
            if crate::parse::types::Language::from_extension(ext).is_some() {
                reindex_single_file(db, root, file)?;
            }
        }
    }
    Ok(())
}

fn reindex_single_file(db: &Database, root: &Path, path: &Path) -> anyhow::Result<()> {
    let source = std::fs::read_to_string(path)?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let Some(lang) = crate::parse::types::Language::from_extension(ext) else {
        return Ok(());
    };

    let abs_path = path.to_string_lossy().to_string();

    // Find and delete existing file data
    let file_id: Option<i64> = db
        .conn()
        .query_row(
            "SELECT id FROM files WHERE absolute_path = ?1",
            rusqlite::params![abs_path],
            |row| row.get(0),
        )
        .ok();

    if let Some(fid) = file_id {
        crate::db::write::delete_file_data(db, fid)?;
    }

    // Re-insert file record
    let rel_path = path.strip_prefix(root).unwrap_or(path);
    let service_id: i64 = db
        .conn()
        .query_row("SELECT id FROM services LIMIT 1", [], |row| row.get(0))
        .unwrap_or(1);

    let modified = path
        .metadata()?
        .modified()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    let new_file_id = crate::db::write::insert_file(
        db,
        service_id,
        &rel_path.to_string_lossy(),
        &abs_path,
        &lang.to_string(),
        modified,
    )?;

    // Parse the file
    let parser = crate::parse::get_parser(lang);
    let result = parser.parse_file(&source, &abs_path)?;

    // Insert symbols and imports
    crate::db::write::insert_symbols_batch(db, new_file_id, &result.symbols)?;
    crate::db::write::insert_imports_batch(db, new_file_id, &result.imports)?;

    // Insert calls with proper caller symbol IDs
    if !result.calls.is_empty() {
        let mut name_to_id = std::collections::HashMap::new();
        if let Ok(syms) = crate::db::query::get_file_symbols(db, new_file_id) {
            for s in &syms {
                name_to_id.insert(s.name.clone(), s.id);
                name_to_id.insert(s.qualified_name.clone(), s.id);
            }
        }

        for call in &result.calls {
            if let Some(&cid) = name_to_id.get(&call.caller_name) {
                let _ = db.conn().execute(
                    "INSERT INTO calls (caller_symbol_id, callee_name, file_id, line, confidence, resolution)
                     VALUES (?1, ?2, ?3, ?4, 0.5, 'unresolved')",
                    rusqlite::params![cid, call.callee_name, new_file_id, call.line],
                );
            }
        }
    }

    tracing::info!(path = %rel_path.display(), "Re-indexed file");

    Ok(())
}
