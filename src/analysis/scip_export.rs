use std::collections::HashMap;
use std::path::Path;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::Database;

/// A SCIP-compatible index in JSON form.
#[derive(Serialize, Deserialize)]
pub struct ScipIndex {
    pub metadata: ScipMetadata,
    pub documents: Vec<ScipDocument>,
}

/// Top-level metadata for the SCIP index.
#[derive(Serialize, Deserialize)]
pub struct ScipMetadata {
    pub version: u32,
    pub tool_info: ToolInfo,
    pub project_root: String,
}

/// Information about the tool that produced the index.
#[derive(Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub version: String,
}

/// A single source document with its symbols and occurrences.
#[derive(Serialize, Deserialize)]
pub struct ScipDocument {
    pub relative_path: String,
    pub language: String,
    pub occurrences: Vec<ScipOccurrence>,
    pub symbols: Vec<ScipSymbolInfo>,
}

/// A symbol occurrence at a specific location.
#[derive(Serialize, Deserialize)]
pub struct ScipOccurrence {
    pub range: Vec<u32>,
    pub symbol: String,
    pub symbol_roles: u32,
}

/// Metadata about a symbol (kind, documentation, etc.).
#[derive(Serialize, Deserialize)]
pub struct ScipSymbolInfo {
    pub symbol: String,
    pub kind: String,
}

/// Export the Ariadne index as a SCIP-compatible JSON file.
///
/// Queries all files, symbols, and call references from the database and
/// writes a JSON structure that follows SCIP's logical model. This avoids
/// any protobuf dependency by using pure JSON via serde.
pub fn export_scip(
    db: &Database,
    output_path: &Path,
    project_root: &Path,
) -> anyhow::Result<()> {
    let conn = db.conn();

    // Query all indexed files
    let mut file_stmt = conn.prepare(
        "SELECT id, path, language FROM files ORDER BY path",
    )?;

    struct FileInfo {
        id: i64,
        path: String,
        language: String,
    }

    let files: Vec<FileInfo> = file_stmt
        .query_map([], |row| {
            Ok(FileInfo {
                id: row.get(0)?,
                path: row.get(1)?,
                language: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // Build a lookup of symbols per file
    let mut sym_stmt = conn.prepare(
        "SELECT id, file_id, name, qualified_name, kind, line_start, line_end
         FROM symbols WHERE file_id = ?1",
    )?;

    // Build a lookup of call references per file
    let mut call_stmt = conn.prepare(
        "SELECT callee_name, line FROM calls WHERE file_id = ?1",
    )?;

    let mut documents = Vec::with_capacity(files.len());

    for file in &files {
        let mut occurrences = Vec::new();
        let mut symbol_infos = Vec::new();
        let mut seen_symbols = HashMap::new();

        // Process symbol definitions
        let symbols: Vec<(String, String, String, u32, u32)> = sym_stmt
            .query_map(params![file.id], |row| {
                Ok((
                    row.get::<_, String>(3)?, // qualified_name
                    row.get::<_, String>(2)?, // name
                    row.get::<_, String>(4)?, // kind
                    row.get::<_, u32>(5)?,    // line_start
                    row.get::<_, u32>(6)?,    // line_end
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        for (qualified_name, _name, kind, line_start, line_end) in &symbols {
            // Definition occurrence: symbol_roles = 1 means definition
            occurrences.push(ScipOccurrence {
                range: vec![
                    line_start.saturating_sub(1), // Convert to 0-indexed
                    0,
                    line_end.saturating_sub(1),
                    0,
                ],
                symbol: qualified_name.clone(),
                symbol_roles: 1, // Definition
            });

            // Only add symbol info once per qualified name
            if !seen_symbols.contains_key(qualified_name) {
                symbol_infos.push(ScipSymbolInfo {
                    symbol: qualified_name.clone(),
                    kind: kind.clone(),
                });
                seen_symbols.insert(qualified_name.clone(), true);
            }
        }

        // Process call references
        let calls: Vec<(String, u32)> = call_stmt
            .query_map(params![file.id], |row| {
                Ok((
                    row.get::<_, String>(0)?, // callee_name
                    row.get::<_, u32>(1)?,    // line
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        for (callee_name, line) in &calls {
            // Reference occurrence: symbol_roles = 0 means reference
            occurrences.push(ScipOccurrence {
                range: vec![
                    line.saturating_sub(1), // Convert to 0-indexed
                    0,
                    line.saturating_sub(1),
                    0,
                ],
                symbol: callee_name.clone(),
                symbol_roles: 0, // Reference
            });
        }

        documents.push(ScipDocument {
            relative_path: file.path.clone(),
            language: file.language.clone(),
            occurrences,
            symbols: symbol_infos,
        });
    }

    let index = ScipIndex {
        metadata: ScipMetadata {
            version: 1,
            tool_info: ToolInfo {
                name: "ariadne".to_string(),
                version: "0.1.0".to_string(),
            },
            project_root: format!("file://{}", project_root.display()),
        },
        documents,
    };

    let json = serde_json::to_string_pretty(&index)?;
    std::fs::write(output_path, json)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn setup_test_db() -> Database {
        let db = Database::open_in_memory().expect("should open in-memory db");
        let conn = db.conn();

        // Insert a service
        conn.execute(
            "INSERT INTO services (id, name, type, repo_path) VALUES (1, 'test', 'monolith', '/tmp/test')",
            [],
        )
        .expect("insert service");

        // Insert a file
        conn.execute(
            "INSERT INTO files (id, service_id, path, absolute_path, language, last_modified, last_indexed)
             VALUES (1, 1, 'src/main.py', '/tmp/test/src/main.py', 'python', 0.0, 0.0)",
            [],
        )
        .expect("insert file");

        // Insert symbols
        conn.execute(
            "INSERT INTO symbols (id, file_id, name, qualified_name, kind, line_start, line_end)
             VALUES (1, 1, 'main', 'src.main.main', 'function', 1, 5)",
            [],
        )
        .expect("insert symbol 1");

        conn.execute(
            "INSERT INTO symbols (id, file_id, name, qualified_name, kind, line_start, line_end)
             VALUES (2, 1, 'helper', 'src.main.helper', 'function', 7, 10)",
            [],
        )
        .expect("insert symbol 2");

        // Insert a call reference
        conn.execute(
            "INSERT INTO calls (id, caller_symbol_id, callee_name, file_id, line)
             VALUES (1, 1, 'src.main.helper', 1, 3)",
            [],
        )
        .expect("insert call");

        db
    }

    #[test]
    fn export_scip_produces_valid_json() {
        let db = setup_test_db();
        let output = std::env::temp_dir().join("test_scip_export.json");
        let root = std::path::PathBuf::from("/tmp/test");

        export_scip(&db, &output, &root).expect("export should succeed");

        let content = std::fs::read_to_string(&output).expect("should read output");
        let index: ScipIndex = serde_json::from_str(&content).expect("should parse JSON");

        assert_eq!(index.metadata.version, 1);
        assert_eq!(index.metadata.tool_info.name, "ariadne");
        assert_eq!(index.metadata.project_root, "file:///tmp/test");
        assert_eq!(index.documents.len(), 1);

        let doc = &index.documents[0];
        assert_eq!(doc.relative_path, "src/main.py");
        assert_eq!(doc.language, "python");
        // 2 definitions + 1 reference
        assert_eq!(doc.occurrences.len(), 3);
        assert_eq!(doc.symbols.len(), 2);

        // Verify definitions have symbol_roles = 1
        let defs: Vec<_> = doc.occurrences.iter().filter(|o| o.symbol_roles == 1).collect();
        assert_eq!(defs.len(), 2);

        // Verify references have symbol_roles = 0
        let refs: Vec<_> = doc.occurrences.iter().filter(|o| o.symbol_roles == 0).collect();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].symbol, "src.main.helper");

        // Cleanup
        let _ = std::fs::remove_file(&output);
    }

    #[test]
    fn export_scip_empty_database() {
        let db = Database::open_in_memory().expect("should open in-memory db");
        let output = std::env::temp_dir().join("test_scip_empty.json");
        let root = std::path::PathBuf::from("/tmp/empty");

        export_scip(&db, &output, &root).expect("export should succeed");

        let content = std::fs::read_to_string(&output).expect("should read output");
        let index: ScipIndex = serde_json::from_str(&content).expect("should parse JSON");

        assert_eq!(index.documents.len(), 0);
        assert_eq!(index.metadata.version, 1);

        // Cleanup
        let _ = std::fs::remove_file(&output);
    }

    #[test]
    fn export_scip_ranges_are_zero_indexed() {
        let db = setup_test_db();
        let output = std::env::temp_dir().join("test_scip_ranges.json");
        let root = std::path::PathBuf::from("/tmp/test");

        export_scip(&db, &output, &root).expect("export should succeed");

        let content = std::fs::read_to_string(&output).expect("should read output");
        let index: ScipIndex = serde_json::from_str(&content).expect("should parse JSON");

        let doc = &index.documents[0];
        // Symbol 'main' is at lines 1-5 in DB, should be 0-4 in SCIP
        let main_def = doc.occurrences.iter().find(|o| o.symbol == "src.main.main" && o.symbol_roles == 1).unwrap();
        assert_eq!(main_def.range[0], 0); // line_start - 1
        assert_eq!(main_def.range[2], 4); // line_end - 1

        // Cleanup
        let _ = std::fs::remove_file(&output);
    }
}
