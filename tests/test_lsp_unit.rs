#![cfg(feature = "lsp")]

use ariadne::lsp;

#[test]
fn test_lsp_module_compiles() {
    // Verify the LSP module is accessible when the feature flag is enabled.
    // This test proves the feature gate and module structure are correct.
    let _ = std::any::type_name::<lsp::AriadneLsp>();
}

#[test]
fn test_lsp_capabilities() {
    use tower_lsp::lsp_types::*;

    let caps = lsp::capabilities::server_capabilities();

    // text_document_sync should be set
    assert!(
        caps.text_document_sync.is_some(),
        "should have text_document_sync"
    );

    // definition_provider
    assert!(
        caps.definition_provider.is_some(),
        "should support go-to-definition"
    );

    // references_provider
    assert!(
        caps.references_provider.is_some(),
        "should support find-references"
    );

    // hover_provider
    assert!(caps.hover_provider.is_some(), "should support hover");

    // document_symbol_provider
    assert!(
        caps.document_symbol_provider.is_some(),
        "should support document symbols"
    );

    // workspace_symbol_provider
    assert!(
        caps.workspace_symbol_provider.is_some(),
        "should support workspace symbols"
    );

    // code_lens_provider
    assert!(
        caps.code_lens_provider.is_some(),
        "should support code lens"
    );

    // Verify text document sync includes save notifications
    if let Some(TextDocumentSyncCapability::Options(opts)) = &caps.text_document_sync {
        assert_eq!(opts.open_close, Some(true), "should track open/close");
        assert!(opts.save.is_some(), "should track saves");
    } else {
        panic!("expected TextDocumentSyncOptions");
    }
}

#[test]
fn test_lsp_convert_symbol_kinds() {
    use tower_lsp::lsp_types::SymbolKind;

    let cases = vec![
        ("function", SymbolKind::FUNCTION),
        ("method", SymbolKind::METHOD),
        ("class", SymbolKind::CLASS),
        ("interface", SymbolKind::INTERFACE),
        ("module", SymbolKind::MODULE),
        ("variable", SymbolKind::VARIABLE),
        ("constant", SymbolKind::CONSTANT),
        ("type_alias", SymbolKind::TYPE_PARAMETER),
        ("enum", SymbolKind::ENUM),
        ("trait", SymbolKind::INTERFACE),
    ];

    for (input, expected) in &cases {
        let result = lsp::convert::to_lsp_symbol_kind(input);
        assert_eq!(
            result, *expected,
            "to_lsp_symbol_kind({:?}) should return {:?}, got {:?}",
            input, expected, result
        );
    }
}

#[test]
fn test_lsp_convert_unknown_kind() {
    use tower_lsp::lsp_types::SymbolKind;

    let result = lsp::convert::to_lsp_symbol_kind("unknown_kind");
    assert_eq!(result, SymbolKind::NULL, "unknown kinds should map to NULL");
}

#[test]
fn test_lsp_convert_location() {
    let loc = lsp::convert::to_lsp_location("/tmp/test.py", 10, 0, 5);
    // Line 10 in 1-based should become line 9 in 0-based LSP coordinates
    assert_eq!(loc.range.start.line, 9);
    assert_eq!(loc.range.start.character, 0);
    assert_eq!(loc.range.end.line, 9);
    assert_eq!(loc.range.end.character, 5);
}

#[test]
fn test_lsp_convert_location_line_zero() {
    // Line 0 should saturating_sub(1) to 0, not underflow
    let loc = lsp::convert::to_lsp_location("/tmp/test.py", 0, 0, 0);
    assert_eq!(loc.range.start.line, 0);
}

// ---------------------------------------------------------------------------
// Behavioral diagnostics tests — first coverage of the diagnostics path
// ---------------------------------------------------------------------------

fn diag_db(parse_errors: i64) -> ariadne::db::Database {
    use ariadne::db::{write, Database};
    let db = Database::open_in_memory().expect("in-memory db");
    let svc = write::insert_service(&db, "t", "/tmp/t", "monolith", "typescript").unwrap();
    let file = write::insert_file(
        &db,
        svc,
        "src/app.tsx",
        "/tmp/t/src/app.tsx",
        "typescript",
        0.0,
    )
    .unwrap();
    if parse_errors > 0 {
        write::set_file_parse_error_count(&db, file, parse_errors as usize).unwrap();
    }
    db
}

#[test]
fn test_build_diagnostics_parse_health_warning_when_dirty() {
    let db = diag_db(3);
    let diags = lsp::build_diagnostics(&db, "src/app.tsx");
    assert!(
        diags.iter().any(|d| d.message.contains("3 syntax error")),
        "expected a parse-health warning naming the count, got: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

#[test]
fn test_build_diagnostics_no_parse_health_warning_when_clean() {
    let db = diag_db(0);
    let diags = lsp::build_diagnostics(&db, "src/app.tsx");
    assert!(
        !diags.iter().any(|d| d.message.contains("syntax error")),
        "clean files must not carry a parse-health warning, got: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

#[test]
fn test_build_diagnostics_dead_code_warning_survives_refactor() {
    use ariadne::db::write;
    let db = diag_db(0);
    // Insert a symbol and mark it dead — the pre-existing diagnostic source
    // must keep firing after the testability extraction.
    let file_id: i64 = db
        .conn()
        .query_row("SELECT id FROM files LIMIT 1", [], |r| r.get(0))
        .unwrap();
    write::insert_symbol(
        &db,
        file_id,
        "unused_fn",
        "app::unused_fn",
        "function",
        3,
        8,
        false,
        false,
        "",
        "",
        None,
    )
    .unwrap();
    db.conn()
        .execute_batch("UPDATE symbols SET is_dead = 1 WHERE name = 'unused_fn'")
        .unwrap();

    let diags = lsp::build_diagnostics(&db, "src/app.tsx");
    assert!(
        diags.iter().any(|d| d.message.contains("Dead code")),
        "dead-code diagnostic must survive the extraction, got: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}
