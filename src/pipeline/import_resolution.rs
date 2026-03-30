use crate::db::Database;
use rusqlite::params;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

/// TypeScript/JavaScript file extensions to try during module resolution.
const EXTENSIONS: &[&str] = &[".ts", ".tsx", ".js", ".jsx"];

/// Index file names to try when a path resolves to a directory.
const INDEX_FILES: &[&str] = &["index.ts", "index.tsx", "index.js", "index.jsx"];

/// Normalize a path by resolving `.` and `..` components using a stack-based approach.
/// This does NOT touch the filesystem -- it is purely lexical normalization.
fn normalize_path(path: &Path) -> PathBuf {
    let mut components: Vec<Component> = Vec::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                // Pop the last normal component if there is one
                if let Some(last) = components.last() {
                    match last {
                        Component::Normal(_) => {
                            components.pop();
                        }
                        _ => {
                            components.push(c);
                        }
                    }
                }
            }
            Component::CurDir => {
                // Skip `.` components entirely
            }
            _ => {
                components.push(c);
            }
        }
    }
    components.iter().collect()
}

/// Expand a module path through tsconfig path aliases.
///
/// Given `module_path = "@/components/Button"` and aliases containing
/// `"@/*" -> "src/*"`, returns `"src/components/Button"`.
///
/// Returns the original path unchanged if no alias matches.
fn expand_alias(module_path: &str, aliases: &[(String, String)]) -> String {
    for (pattern, replacement) in aliases {
        if let Some(wildcard_pos) = pattern.find('*') {
            let prefix = &pattern[..wildcard_pos];
            if let Some(rest) = module_path.strip_prefix(prefix) {
                if let Some(rep_wildcard_pos) = replacement.find('*') {
                    let rep_prefix = &replacement[..rep_wildcard_pos];
                    let rep_suffix = &replacement[rep_wildcard_pos + 1..];
                    return format!("{}{}{}", rep_prefix, rest, rep_suffix);
                } else {
                    return replacement.clone();
                }
            }
        } else if module_path == pattern {
            // Exact match (no wildcard)
            return replacement.clone();
        }
    }
    module_path.to_string()
}

/// Load path aliases from tsconfig.json for the first service in the database.
///
/// Reads `compilerOptions.baseUrl` and `compilerOptions.paths` from the
/// tsconfig.json file found at the service's `repo_path`. Returns a list
/// of (pattern, expanded_replacement) pairs where the baseUrl has already
/// been prepended to the replacement.
fn load_path_aliases(db: &Database) -> Vec<(String, String)> {
    let conn = db.conn();

    // Find the repo path for the first service
    let repo_path: Option<String> = conn
        .query_row("SELECT repo_path FROM services LIMIT 1", [], |row| {
            row.get(0)
        })
        .ok();

    let repo_path = match repo_path {
        Some(p) => p,
        None => return Vec::new(),
    };

    // Try to read tsconfig.json
    let tsconfig_path = Path::new(&repo_path).join("tsconfig.json");
    let content = match std::fs::read_to_string(&tsconfig_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let compiler_options = match parsed.get("compilerOptions") {
        Some(v) => v,
        None => return Vec::new(),
    };

    let base_url = compiler_options
        .get("baseUrl")
        .and_then(|v| v.as_str())
        .unwrap_or(".");

    let paths = match compiler_options.get("paths") {
        Some(serde_json::Value::Object(map)) => map,
        _ => return Vec::new(),
    };

    let mut aliases = Vec::new();
    for (pattern, targets) in paths {
        // paths values are arrays; take the first target
        if let Some(first_target) = targets.as_array().and_then(|arr| arr.first()) {
            if let Some(target_str) = first_target.as_str() {
                // Prepend baseUrl to the replacement path
                let full_replacement = if base_url == "." {
                    target_str.to_string()
                } else {
                    format!("{}/{}", base_url.trim_end_matches('/'), target_str)
                };
                aliases.push((pattern.clone(), full_replacement));
            }
        }
    }

    // Sort by pattern length descending so more specific aliases match first
    // e.g., "@components/*" should be tried before "@/*"
    aliases.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    aliases
}

/// Try to resolve a module path to a file ID in the file path map.
///
/// Implements Node/TypeScript module resolution:
/// 1. Expand path aliases from tsconfig.json
/// 2. Resolve relative paths against the importing file's directory
/// 3. Try exact match, then extensions (.ts, .tsx, .js, .jsx), then index files
fn resolve_module_path(
    module_path: &str,
    importing_file_path: &str,
    aliases: &[(String, String)],
    file_paths: &HashMap<String, i64>,
) -> Option<i64> {
    // Step 1: Expand aliases (only for non-relative paths)
    let expanded = if module_path.starts_with('.') {
        module_path.to_string()
    } else {
        expand_alias(module_path, aliases)
    };

    // Step 2: Resolve relative path against importing file's directory
    let resolved = if expanded.starts_with('.') {
        let base_dir = Path::new(importing_file_path).parent()?;
        base_dir.join(&expanded)
    } else {
        PathBuf::from(&expanded)
    };

    // Step 3: Normalize the path (resolve ../ and ./)
    let normalized = normalize_path(&resolved);
    let norm_str = normalized.to_string_lossy();

    // Step 4: Try exact match (path might already have an extension)
    if let Some(&id) = file_paths.get(norm_str.as_ref()) {
        return Some(id);
    }

    // Step 5: Try adding extensions: .ts, .tsx, .js, .jsx
    for ext in EXTENSIONS {
        let with_ext = format!("{}{}", norm_str, ext);
        if let Some(&id) = file_paths.get(&with_ext) {
            return Some(id);
        }
    }

    // Step 6: Try index files: /index.ts, /index.tsx, /index.js, /index.jsx
    for index_file in INDEX_FILES {
        let with_index = format!("{}/{}", norm_str, index_file);
        if let Some(&id) = file_paths.get(&with_index) {
            return Some(id);
        }
    }

    None
}

/// Resolve a Rust module path (e.g. `crate::db::query`) to a file ID.
///
/// Handles `crate::`, `super::`, and `self::` prefixes by mapping them
/// to the conventional Rust file layout: `src/foo/bar.rs` or `src/foo/bar/mod.rs`.
fn resolve_rust_module_path(
    module_path: &str,
    importing_file_path: &str,
    file_paths: &HashMap<String, i64>,
) -> Option<i64> {
    let segments: Vec<&str> = module_path.split("::").collect();
    if segments.is_empty() {
        return None;
    }

    let (start_dir, seg_start) = match segments[0] {
        "crate" => {
            // crate:: resolves from the project src/ directory
            (PathBuf::from("src"), 1)
        }
        "super" => {
            // super:: means the parent module. For src/db/query.rs, the parent
            // module is src/db/ (i.e., the directory containing this file).
            // For remaining segments, resolve relative to that directory.
            let parent = Path::new(importing_file_path).parent()?;
            (parent.to_path_buf(), 1)
        }
        "self" => {
            // self:: resolves from the importing file's directory
            let parent = Path::new(importing_file_path).parent()?;
            (parent.to_path_buf(), 1)
        }
        _ => {
            // No recognized prefix — likely an external crate
            return None;
        }
    };

    // Filter out glob `*` from segments — it means "import everything from the module file"
    let remaining: Vec<&str> = segments[seg_start..]
        .iter()
        .filter(|s| **s != "*")
        .copied()
        .collect();

    // Try progressively shorter paths: all segments, then all-but-last, etc.
    // For `crate::db::query::get_file_symbols`, tries `src/db/query/get_file_symbols.rs`,
    // then `src/db/query.rs`, then `src/db.rs` — first match wins.
    for take in (1..=remaining.len()).rev() {
        let path_segments = &remaining[..take];
        let mut candidate = start_dir.clone();
        for seg in path_segments {
            candidate.push(seg);
        }

        let rs_path = candidate.with_extension("rs");
        if let Some(&id) = file_paths.get(rs_path.to_string_lossy().as_ref()) {
            return Some(id);
        }

        let mod_path = candidate.join("mod.rs");
        if let Some(&id) = file_paths.get(mod_path.to_string_lossy().as_ref()) {
            return Some(id);
        }
    }

    // Fallback: no path segments matched a file. The imported name is likely
    // a symbol in the parent module itself (e.g., `use super::Database` where
    // Database is a struct in mod.rs, or `use super::*` for a glob re-export).
    let mod_path = start_dir.join("mod.rs");
    if let Some(&id) = file_paths.get(mod_path.to_string_lossy().as_ref()) {
        return Some(id);
    }
    let dir_rs = start_dir.with_extension("rs");
    if let Some(&id) = file_paths.get(dir_rs.to_string_lossy().as_ref()) {
        return Some(id);
    }

    None
}

/// Phase 4: Resolve import statements to their target files and symbols.
///
/// For each unresolved internal import, this function:
/// 1. Loads tsconfig.json path aliases for alias expansion
/// 2. Pre-loads all file paths into a HashMap for O(1) lookup
/// 3. Resolves each import using language-appropriate module resolution:
///    - Rust: `crate::`, `super::`, `self::` module path mapping
///    - JS/TS: relative paths, extensions, index files, path aliases
/// 4. After resolving the file, matches the imported symbol name
pub fn resolve_imports(db: &Database) -> anyhow::Result<()> {
    let conn = db.conn();

    // Load tsconfig.json path aliases
    let aliases = load_path_aliases(db);

    // Pre-load all file paths into a HashMap for O(1) lookup.
    // Key: relative path (e.g., "src/components/Button.tsx"), Value: file ID
    let file_paths: HashMap<String, i64> = {
        let mut stmt = conn.prepare("SELECT id, path FROM files")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(0)?))
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let (path, id) = row?;
            map.insert(path, id);
        }
        map
    };

    // Pre-load file languages for dispatch
    let file_languages: HashMap<i64, String> = {
        let mut stmt = conn.prepare("SELECT id, language FROM files")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let (id, lang) = row?;
            map.insert(id, lang);
        }
        map
    };

    // Get all unresolved internal imports, including the importing file's path
    let mut import_stmt = conn.prepare(
        "SELECT i.id, i.module_path, i.imported_name, i.file_id, f.path
         FROM imports i
         JOIN files f ON i.file_id = f.id
         WHERE i.resolved_file_id IS NULL AND i.is_external = 0",
    )?;

    let imports: Vec<(i64, String, String, i64, String)> = import_stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut resolved_count: usize = 0;
    let mut symbol_count: usize = 0;

    for (import_id, module_path, imported_name, file_id, importing_file_path) in &imports {
        // Dispatch to language-appropriate resolution
        let lang = file_languages.get(file_id).map(|s| s.as_str()).unwrap_or("");
        let resolved_file = if lang == "rust" {
            resolve_rust_module_path(module_path, importing_file_path, &file_paths)
        } else {
            resolve_module_path(module_path, importing_file_path, &aliases, &file_paths)
        };

        if let Some(fid) = resolved_file {
            conn.execute(
                "UPDATE imports SET resolved_file_id = ?1 WHERE id = ?2",
                params![fid, import_id],
            )?;
            resolved_count += 1;

            // Try to resolve the specific symbol within the resolved file.
            // Match by exported name; for default imports try "default" as well.
            let resolved_sym: Option<i64> = conn
                .query_row(
                    "SELECT id FROM symbols
                     WHERE file_id = ?1 AND name = ?2 AND is_exported = 1
                     LIMIT 1",
                    params![fid, imported_name],
                    |row| row.get(0),
                )
                .ok();

            // If no exported symbol found, try without the is_exported filter
            // (some parsers may not mark all exports)
            let resolved_sym = resolved_sym.or_else(|| {
                conn.query_row(
                    "SELECT id FROM symbols WHERE file_id = ?1 AND name = ?2 LIMIT 1",
                    params![fid, imported_name],
                    |row| row.get(0),
                )
                .ok()
            });

            if let Some(sid) = resolved_sym {
                conn.execute(
                    "UPDATE imports SET resolved_symbol_id = ?1 WHERE id = ?2",
                    params![sid, import_id],
                )?;
                symbol_count += 1;
            }
        }
    }

    tracing::info!(
        "Import resolution: resolved {}/{} files, {}/{} symbols",
        resolved_count,
        imports.len(),
        symbol_count,
        imports.len(),
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use rusqlite::params;

    /// Helper to set up a minimal database with a service, files, and imports.
    fn setup_test_db() -> Database {
        let db = Database::open_in_memory().expect("should open in-memory db");
        let conn = db.conn();

        // Create a service
        conn.execute(
            "INSERT INTO services (name, type, repo_path) VALUES ('test', 'monolith', '/tmp/test')",
            [],
        )
        .unwrap();

        db
    }

    fn insert_file(db: &Database, path: &str) -> i64 {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO files (service_id, path, absolute_path, language, last_modified, last_indexed)
             VALUES (1, ?1, ?1, 'typescript', 0.0, 0.0)",
            params![path],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn insert_import(db: &Database, file_id: i64, module_path: &str, imported_name: &str) -> i64 {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO imports (file_id, imported_name, module_path, line, is_external)
             VALUES (?1, ?2, ?3, 1, 0)",
            params![file_id, imported_name, module_path],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn insert_symbol(db: &Database, file_id: i64, name: &str, is_exported: bool) -> i64 {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO symbols (file_id, name, qualified_name, kind, line_start, line_end, is_exported)
             VALUES (?1, ?2, ?2, 'function', 1, 10, ?3)",
            params![file_id, name, is_exported],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn get_resolved_file(db: &Database, import_id: i64) -> Option<i64> {
        db.conn()
            .query_row(
                "SELECT resolved_file_id FROM imports WHERE id = ?1",
                params![import_id],
                |row| row.get(0),
            )
            .ok()
    }

    fn get_resolved_symbol(db: &Database, import_id: i64) -> Option<i64> {
        db.conn()
            .query_row(
                "SELECT resolved_symbol_id FROM imports WHERE id = ?1",
                params![import_id],
                |row| row.get(0),
            )
            .ok()
    }

    // --- normalize_path tests ---

    #[test]
    fn normalize_removes_current_dir() {
        let result = normalize_path(Path::new("src/./utils/./foo"));
        assert_eq!(result, PathBuf::from("src/utils/foo"));
    }

    #[test]
    fn normalize_resolves_parent_dir() {
        let result = normalize_path(Path::new("src/pages/../utils/foo"));
        assert_eq!(result, PathBuf::from("src/utils/foo"));
    }

    #[test]
    fn normalize_handles_multiple_parent_dirs() {
        let result = normalize_path(Path::new("src/a/b/../../c"));
        assert_eq!(result, PathBuf::from("src/c"));
    }

    // --- expand_alias tests ---

    #[test]
    fn expand_alias_matches_wildcard() {
        let aliases = vec![("@/*".to_string(), "src/*".to_string())];
        assert_eq!(
            expand_alias("@/components/Button", &aliases),
            "src/components/Button"
        );
    }

    #[test]
    fn expand_alias_matches_specific_prefix() {
        let aliases = vec![
            ("@components/*".to_string(), "src/components/*".to_string()),
            ("@/*".to_string(), "src/*".to_string()),
        ];
        assert_eq!(
            expand_alias("@components/Button", &aliases),
            "src/components/Button"
        );
    }

    #[test]
    fn expand_alias_no_match_returns_original() {
        let aliases = vec![("@/*".to_string(), "src/*".to_string())];
        assert_eq!(expand_alias("lodash", &aliases), "lodash");
    }

    #[test]
    fn expand_alias_exact_match() {
        let aliases = vec![("@config".to_string(), "src/config/index".to_string())];
        assert_eq!(expand_alias("@config", &aliases), "src/config/index");
    }

    // --- resolve_module_path tests ---

    #[test]
    fn resolve_relative_with_extension() {
        let mut file_paths = HashMap::new();
        file_paths.insert("src/utils/helpers.ts".to_string(), 1);

        // From src/pages/Home.tsx, ../utils/helpers goes up to src/ then into utils/
        let result =
            resolve_module_path("../utils/helpers", "src/pages/Home.tsx", &[], &file_paths);
        assert_eq!(result, Some(1));
    }

    #[test]
    fn resolve_relative_tsx_extension() {
        let mut file_paths = HashMap::new();
        file_paths.insert("src/components/Button.tsx".to_string(), 2);

        let result = resolve_module_path("./components/Button", "src/App.tsx", &[], &file_paths);
        assert_eq!(result, Some(2));
    }

    #[test]
    fn resolve_parent_relative_path() {
        let mut file_paths = HashMap::new();
        file_paths.insert("src/utils/format.ts".to_string(), 3);

        let result = resolve_module_path("../utils/format", "src/pages/Home.tsx", &[], &file_paths);
        assert_eq!(result, Some(3));
    }

    #[test]
    fn resolve_index_file() {
        let mut file_paths = HashMap::new();
        file_paths.insert("src/components/index.ts".to_string(), 4);

        let result = resolve_module_path("./components", "src/App.tsx", &[], &file_paths);
        assert_eq!(result, Some(4));
    }

    #[test]
    fn resolve_index_tsx_file() {
        let mut file_paths = HashMap::new();
        file_paths.insert("src/hooks/index.tsx".to_string(), 5);

        let result = resolve_module_path("./hooks", "src/App.tsx", &[], &file_paths);
        assert_eq!(result, Some(5));
    }

    #[test]
    fn resolve_with_alias() {
        let mut file_paths = HashMap::new();
        file_paths.insert("src/components/Button.tsx".to_string(), 6);

        let aliases = vec![("@/*".to_string(), "src/*".to_string())];

        let result = resolve_module_path(
            "@/components/Button",
            "src/pages/Home.tsx",
            &aliases,
            &file_paths,
        );
        assert_eq!(result, Some(6));
    }

    #[test]
    fn resolve_alias_with_index() {
        let mut file_paths = HashMap::new();
        file_paths.insert("src/store/index.ts".to_string(), 7);

        let aliases = vec![("@/*".to_string(), "src/*".to_string())];

        let result = resolve_module_path("@/store", "src/pages/Home.tsx", &aliases, &file_paths);
        assert_eq!(result, Some(7));
    }

    #[test]
    fn resolve_exact_path_with_extension() {
        let mut file_paths = HashMap::new();
        file_paths.insert("src/utils/helpers.ts".to_string(), 8);

        let result = resolve_module_path("./utils/helpers.ts", "src/App.tsx", &[], &file_paths);
        assert_eq!(result, Some(8));
    }

    #[test]
    fn resolve_prefers_ts_over_tsx() {
        let mut file_paths = HashMap::new();
        file_paths.insert("src/utils/helpers.ts".to_string(), 10);
        file_paths.insert("src/utils/helpers.tsx".to_string(), 11);

        let result = resolve_module_path("./utils/helpers", "src/App.tsx", &[], &file_paths);
        // .ts is tried before .tsx, so we get the .ts file
        assert_eq!(result, Some(10));
    }

    // --- Full integration tests ---

    #[test]
    fn integration_resolves_relative_import() {
        let db = setup_test_db();
        let utils_id = insert_file(&db, "src/utils/helpers.ts");
        let page_id = insert_file(&db, "src/pages/Home.tsx");
        let _sym_id = insert_symbol(&db, utils_id, "formatDate", true);
        let import_id = insert_import(&db, page_id, "./utils/helpers", "formatDate");

        // Home.tsx is in src/pages/, but module_path is relative...
        // The importing file path is "src/pages/Home.tsx" so ../utils would go up.
        // Actually "./utils/helpers" from "src/pages/Home.tsx" resolves to "src/pages/utils/helpers"
        // That won't match "src/utils/helpers.ts".
        // The correct import for this would be "../utils/helpers".
        // Let's fix the test to use the correct relative path.
        let conn = db.conn();
        conn.execute(
            "UPDATE imports SET module_path = '../utils/helpers' WHERE id = ?1",
            params![import_id],
        )
        .unwrap();

        resolve_imports(&db).unwrap();

        assert_eq!(get_resolved_file(&db, import_id), Some(utils_id));
        assert!(get_resolved_symbol(&db, import_id).is_some());
    }

    #[test]
    fn integration_resolves_sibling_import() {
        let db = setup_test_db();
        let button_id = insert_file(&db, "src/components/Button.tsx");
        let header_id = insert_file(&db, "src/components/Header.tsx");
        let _sym_id = insert_symbol(&db, button_id, "Button", true);
        let import_id = insert_import(&db, header_id, "./Button", "Button");

        resolve_imports(&db).unwrap();

        assert_eq!(get_resolved_file(&db, import_id), Some(button_id));
    }

    #[test]
    fn integration_resolves_index_barrel() {
        let db = setup_test_db();
        let index_id = insert_file(&db, "src/components/index.ts");
        let app_id = insert_file(&db, "src/App.tsx");
        let _sym_id = insert_symbol(&db, index_id, "Button", true);
        let import_id = insert_import(&db, app_id, "./components", "Button");

        resolve_imports(&db).unwrap();

        assert_eq!(get_resolved_file(&db, import_id), Some(index_id));
    }

    #[test]
    fn integration_skips_external_imports() {
        let db = setup_test_db();
        let app_id = insert_file(&db, "src/App.tsx");
        let conn = db.conn();
        conn.execute(
            "INSERT INTO imports (file_id, imported_name, module_path, line, is_external)
             VALUES (?1, 'useState', 'react', 1, 1)",
            params![app_id],
        )
        .unwrap();
        let external_import_id = conn.last_insert_rowid();

        resolve_imports(&db).unwrap();

        // External imports should remain unresolved
        assert_eq!(get_resolved_file(&db, external_import_id), None);
    }

    #[test]
    fn integration_resolves_symbol_in_file() {
        let db = setup_test_db();
        let utils_id = insert_file(&db, "src/utils/math.ts");
        let app_id = insert_file(&db, "src/App.tsx");
        let sym_id = insert_symbol(&db, utils_id, "add", true);
        let _other_sym = insert_symbol(&db, utils_id, "subtract", true);
        let import_id = insert_import(&db, app_id, "./utils/math", "add");

        resolve_imports(&db).unwrap();

        assert_eq!(get_resolved_file(&db, import_id), Some(utils_id));
        assert_eq!(get_resolved_symbol(&db, import_id), Some(sym_id));
    }
}
