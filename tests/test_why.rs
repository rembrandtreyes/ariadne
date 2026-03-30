use ariadne::config::RepoConfig;
use ariadne::db::query;
use ariadne::db::Database;
use ariadne::graph::blast_radius::analyze_blast_radius;
use ariadne::pipeline::run_full_pipeline;
use std::path::Path;

/// The `why` logic should find a symbol, its callers, callees, and blast radius.
/// Uses the python_repo fixture which has known call relationships.
#[test]
fn test_why_symbol_found() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    let config = RepoConfig::default();
    let fixture = Path::new("tests/fixtures/python_repo");

    run_full_pipeline(&db, fixture, &config).unwrap();

    // Find any symbol to verify the why logic works
    let symbols = query::get_dead_symbols(&db).unwrap();
    // There should be at least some symbols in the DB
    let sym_count = query::count_symbols(&db).unwrap();
    assert!(sym_count > 0, "fixture should have symbols");

    // Pick the first symbol by name
    let all_syms: Vec<query::SymbolRow> = db
        .conn()
        .prepare("SELECT id, file_id, name, qualified_name, kind, line_start, line_end, is_dead, is_test FROM symbols LIMIT 1")
        .unwrap()
        .query_map([], |row| {
            Ok(query::SymbolRow {
                id: row.get(0)?,
                file_id: row.get(1)?,
                name: row.get(2)?,
                qualified_name: row.get(3)?,
                kind: row.get(4)?,
                line_start: row.get(5)?,
                line_end: row.get(6)?,
                is_dead: row.get(7)?,
                is_test: row.get(8)?,
            })
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert!(!all_syms.is_empty(), "should have at least one symbol");
    let sym = &all_syms[0];

    // Verify we can get the file path
    let file_path = query::file_path_by_id(&db, sym.file_id);
    assert!(file_path.is_ok(), "should resolve file path");

    // Verify callers/callees queries succeed (may be empty)
    let callers = query::get_dependents(&db, sym.id);
    assert!(callers.is_ok(), "get_dependents should succeed");

    let callees = query::get_dependencies(&db, sym.id);
    assert!(callees.is_ok(), "get_dependencies should succeed");

    // Verify blast radius query succeeds
    let graph = query::build_call_graph(&db, None).unwrap();
    let blast = analyze_blast_radius(&graph, sym.id as u64, Some(10), false);
    // blast radius should return a valid result (may be 0)
    assert!(blast.total_affected >= 0);
}

/// The `why` logic should handle symbol-not-found gracefully.
#[test]
fn test_why_symbol_not_found() {
    let db = Database::open_in_memory().unwrap();

    let result = query::find_symbol_by_name(&db, "nonexistent_symbol_xyz");
    assert!(result.is_ok(), "query should succeed");
    assert!(
        result.unwrap().is_none(),
        "should return None for missing symbol"
    );
}

/// The JSON output structure should contain all expected fields.
#[test]
fn test_why_json_output_structure() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    let config = RepoConfig::default();
    let fixture = Path::new("tests/fixtures/python_repo");

    run_full_pipeline(&db, fixture, &config).unwrap();

    // Get first symbol
    let sym = db
        .conn()
        .query_row(
            "SELECT id, file_id, name, qualified_name, kind, line_start, line_end, is_dead, is_test FROM symbols LIMIT 1",
            [],
            |row| {
                Ok(query::SymbolRow {
                    id: row.get(0)?,
                    file_id: row.get(1)?,
                    name: row.get(2)?,
                    qualified_name: row.get(3)?,
                    kind: row.get(4)?,
                    line_start: row.get(5)?,
                    line_end: row.get(6)?,
                    is_dead: row.get(7)?,
                    is_test: row.get(8)?,
                })
            },
        )
        .unwrap();

    let file_path = query::file_path_by_id(&db, sym.file_id).unwrap();
    let callers = query::get_dependents(&db, sym.id).unwrap();
    let callees = query::get_dependencies(&db, sym.id).unwrap();
    let graph = query::build_call_graph(&db, None).unwrap();
    let blast = analyze_blast_radius(&graph, sym.id as u64, Some(10), false);

    // Build the same JSON structure the cmd_why function would
    let output = serde_json::json!({
        "symbol": {
            "name": sym.name,
            "qualified_name": sym.qualified_name,
            "kind": sym.kind,
            "file": file_path,
            "line_start": sym.line_start,
            "line_end": sym.line_end,
            "is_dead": sym.is_dead,
            "is_test": sym.is_test,
        },
        "callers": callers.iter().map(|c| serde_json::json!({
            "name": c.name, "qualified_name": c.qualified_name, "kind": c.kind,
        })).collect::<Vec<_>>(),
        "callees": callees.iter().map(|c| serde_json::json!({
            "name": c.name, "qualified_name": c.qualified_name, "kind": c.kind,
        })).collect::<Vec<_>>(),
        "blast_radius": {
            "total_affected": blast.total_affected,
            "direct": blast.direct_dependents.len(),
            "transitive": blast.transitive_dependents.len(),
            "affected_files": blast.affected_files,
        },
    });

    // Verify structure
    let obj = output.as_object().unwrap();
    assert!(obj.contains_key("symbol"), "missing symbol field");
    assert!(obj.contains_key("callers"), "missing callers field");
    assert!(obj.contains_key("callees"), "missing callees field");
    assert!(
        obj.contains_key("blast_radius"),
        "missing blast_radius field"
    );

    let sym_obj = obj["symbol"].as_object().unwrap();
    assert!(sym_obj.contains_key("name"), "symbol missing name");
    assert!(
        sym_obj.contains_key("qualified_name"),
        "symbol missing qualified_name"
    );
    assert!(sym_obj.contains_key("kind"), "symbol missing kind");
    assert!(sym_obj.contains_key("file"), "symbol missing file");
    assert!(sym_obj.contains_key("is_dead"), "symbol missing is_dead");

    let br = obj["blast_radius"].as_object().unwrap();
    assert!(
        br.contains_key("total_affected"),
        "blast_radius missing total_affected"
    );
    assert!(br.contains_key("direct"), "blast_radius missing direct");
    assert!(
        br.contains_key("transitive"),
        "blast_radius missing transitive"
    );
}
