use tower_lsp::lsp_types::*;

/// Convert an Ariadne symbol location to an LSP Location.
pub fn to_lsp_location(file_path: &str, line: u32, col_start: u32, col_end: u32) -> Location {
    Location {
        uri: Url::from_file_path(file_path).unwrap_or_else(|_| {
            Url::parse(&format!("file://{}", file_path))
                .unwrap_or_else(|_| Url::parse("file:///unknown").unwrap())
        }),
        range: Range {
            start: Position {
                line: line.saturating_sub(1),
                character: col_start,
            },
            end: Position {
                line: line.saturating_sub(1),
                character: col_end,
            },
        },
    }
}

/// Convert an Ariadne symbol kind string to an LSP SymbolKind.
pub fn to_lsp_symbol_kind(kind: &str) -> SymbolKind {
    match kind {
        "function" => SymbolKind::FUNCTION,
        "method" => SymbolKind::METHOD,
        "class" => SymbolKind::CLASS,
        "interface" => SymbolKind::INTERFACE,
        "module" => SymbolKind::MODULE,
        "variable" => SymbolKind::VARIABLE,
        "constant" => SymbolKind::CONSTANT,
        "type_alias" => SymbolKind::TYPE_PARAMETER,
        "enum" => SymbolKind::ENUM,
        "trait" => SymbolKind::INTERFACE,
        _ => SymbolKind::NULL,
    }
}
