use crate::db::write;
use crate::db::Database;
use std::path::Path;

use super::discovery::DiscoveryResult;

/// Phase 1: Create database structure from discovered files.
///
/// Inserts a service record and a file record for each discovered source file.
pub fn create_structure(
    db: &Database,
    discovery: &DiscoveryResult,
    root: &Path,
) -> anyhow::Result<()> {
    let service_name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let primary_lang = discovery
        .languages
        .first()
        .map(|l| l.display_name().to_string())
        .unwrap_or_default();

    let service_id = write::insert_service(
        db,
        &service_name,
        &root.to_string_lossy(),
        "microservice",
        &primary_lang,
    )?;

    for file in &discovery.files {
        let abs = file.path.to_string_lossy().to_string();
        let rel = file
            .path
            .strip_prefix(root)
            .unwrap_or(&file.path)
            .to_string_lossy()
            .to_string();

        let last_modified = std::fs::metadata(&file.path)
            .and_then(|m| m.modified())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64()
            })
            .unwrap_or(0.0);

        write::insert_file(
            db,
            service_id,
            &rel,
            &abs,
            file.language.display_name(),
            last_modified,
        )?;
    }

    Ok(())
}
