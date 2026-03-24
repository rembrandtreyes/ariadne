pub mod capabilities;
pub mod convert;

use std::path::PathBuf;
use std::sync::Arc;

use rusqlite::params;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::db::Database;

pub struct AriadneLsp {
    client: Client,
    db_path: Arc<PathBuf>,
}

impl AriadneLsp {
    pub fn new(client: Client, db_path: PathBuf) -> Self {
        Self {
            client,
            db_path: Arc::new(db_path),
        }
    }

    /// Open a fresh database connection for the current request.
    /// Returns None if the database file doesn't exist or can't be opened.
    fn open_db(&self) -> Option<Database> {
        if !self.db_path.exists() {
            return None;
        }
        Database::open(&self.db_path).ok()
    }

    /// Extract a usable file path suffix from a URI for LIKE matching against the DB.
    /// The DB stores relative paths, so we strip any workspace prefix and match with LIKE.
    fn uri_to_path_suffix(uri: &Url) -> Option<String> {
        let path = uri.to_file_path().ok()?;
        // Extract just the filename for LIKE matching
        let filename = path.file_name()?.to_string_lossy().to_string();
        Some(filename)
    }

    /// Build diagnostics for a given file URI.
    /// Returns dead code warnings and architectural rule violation errors.
    fn build_diagnostics_for_file(&self, uri: &Url) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        let db = match self.open_db() {
            Some(db) => db,
            None => return diagnostics,
        };

        let suffix = match Self::uri_to_path_suffix(uri) {
            Some(s) => s,
            None => return diagnostics,
        };

        let conn = db.conn();
        let pattern = format!("%{}", crate::db::escape_like(&suffix));

        // 1) Dead code symbols (is_dead = true) -> Warning severity
        if let Ok(mut stmt) = conn.prepare(
            "SELECT s.name, s.qualified_name, s.line_start, s.line_end
             FROM symbols s
             JOIN files f ON s.file_id = f.id
             WHERE f.path LIKE ?1 ESCAPE '\\' AND s.is_dead = 1",
        ) {
            if let Ok(rows) = stmt.query_map(params![pattern], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u32>(2)?,
                    row.get::<_, u32>(3)?,
                ))
            }) {
                for row in rows.flatten() {
                    let (name, _qualified, line_start, line_end) = row;
                    diagnostics.push(Diagnostic {
                        range: Range {
                            start: Position {
                                line: line_start.saturating_sub(1),
                                character: 0,
                            },
                            end: Position {
                                line: line_end.saturating_sub(1),
                                character: 0,
                            },
                        },
                        severity: Some(DiagnosticSeverity::WARNING),
                        source: Some("ariadne".to_string()),
                        message: format!(
                            "Dead code: `{}` has no callers and is not exported",
                            name
                        ),
                        ..Default::default()
                    });
                }
            }
        }

        // 2) Architectural rule violations for this file -> Error or Warning severity
        if let Ok(mut stmt) = conn.prepare(
            "SELECT rv.rule_name, rv.from_symbol, rv.to_symbol, rv.line, rv.severity
             FROM rule_violations rv
             JOIN files f ON rv.from_file_id = f.id
             WHERE f.path LIKE ?1 ESCAPE '\\'",
        ) {
            if let Ok(rows) = stmt.query_map(params![pattern], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<u32>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            }) {
                for row in rows.flatten() {
                    let (rule_name, from_sym, to_sym, line, severity) = row;
                    let diag_line = line.unwrap_or(1);
                    let diag_severity = if severity == "error" {
                        DiagnosticSeverity::ERROR
                    } else {
                        DiagnosticSeverity::WARNING
                    };
                    let message = match (&from_sym, &to_sym) {
                        (Some(from), Some(to)) => {
                            format!("Rule `{}` violated: `{}` -> `{}`", rule_name, from, to)
                        }
                        (_, Some(to)) => {
                            format!(
                                "Rule `{}` violated: forbidden dependency on `{}`",
                                rule_name, to
                            )
                        }
                        _ => {
                            format!("Rule `{}` violated", rule_name)
                        }
                    };
                    diagnostics.push(Diagnostic {
                        range: Range {
                            start: Position {
                                line: diag_line.saturating_sub(1),
                                character: 0,
                            },
                            end: Position {
                                line: diag_line.saturating_sub(1),
                                character: 0,
                            },
                        },
                        severity: Some(diag_severity),
                        source: Some("ariadne".to_string()),
                        message,
                        ..Default::default()
                    });
                }
            }
        }

        diagnostics
    }

    /// Find the symbol at a given cursor position in a file.
    fn find_symbol_at_position(
        db: &Database,
        file_path_suffix: &str,
        line: u32,
    ) -> Option<SymbolAtCursor> {
        let conn = db.conn();
        let pattern = format!("%{}", crate::db::escape_like(file_path_suffix));
        // line is 0-based from LSP, DB stores 1-based line numbers
        let db_line = line + 1;

        let mut stmt = conn
            .prepare(
                "SELECT s.id, s.name, s.qualified_name, s.kind, f.path, s.line_start, s.line_end, s.is_exported, s.is_dead, f.id
                 FROM symbols s
                 JOIN files f ON s.file_id = f.id
                 WHERE f.path LIKE ?1 ESCAPE '\\' AND s.line_start <= ?2 AND s.line_end >= ?2
                 ORDER BY (s.line_end - s.line_start) ASC
                 LIMIT 1",
            )
            .ok()?;

        stmt.query_row(params![pattern, db_line], |row| {
            Ok(SymbolAtCursor {
                id: row.get(0)?,
                name: row.get(1)?,
                qualified_name: row.get(2)?,
                kind: row.get(3)?,
                file_path: row.get(4)?,
                line_start: row.get(5)?,
                line_end: row.get(6)?,
                is_exported: row.get(7)?,
                is_dead: row.get(8)?,
                file_id: row.get(9)?,
            })
        })
        .ok()
    }
}

/// Internal representation of a symbol found at the cursor position.
#[allow(dead_code)]
struct SymbolAtCursor {
    id: i64,
    name: String,
    qualified_name: String,
    kind: String,
    file_path: String,
    line_start: u32,
    line_end: u32,
    is_exported: bool,
    is_dead: bool,
    file_id: i64,
}

#[tower_lsp::async_trait]
impl LanguageServer for AriadneLsp {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "ariadne-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            capabilities: capabilities::server_capabilities(),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Ariadne LSP initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let diagnostics = self.build_diagnostics_for_file(&uri);
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        let diagnostics = self.build_diagnostics_for_file(&uri);
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let uri = &params.text_document.uri;

        let db = match self.open_db() {
            Some(db) => db,
            None => return Ok(Some(Vec::new())),
        };

        let suffix = match Self::uri_to_path_suffix(uri) {
            Some(s) => s,
            None => return Ok(Some(Vec::new())),
        };

        let conn = db.conn();
        let pattern = format!("%{}", crate::db::escape_like(&suffix));

        // Query all symbols in this file
        let mut stmt = match conn.prepare(
            "SELECT s.id, s.name, s.line_start, s.line_end, s.is_exported, s.is_dead
             FROM symbols s
             JOIN files f ON s.file_id = f.id
             WHERE f.path LIKE ?1
             ORDER BY s.line_start",
        ) {
            Ok(s) => s,
            Err(_) => return Ok(Some(Vec::new())),
        };

        let symbols: Vec<(i64, String, u32, u32, bool, bool)> =
            match stmt.query_map(params![pattern], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u32>(2)?,
                    row.get::<_, u32>(3)?,
                    row.get::<_, bool>(4)?,
                    row.get::<_, bool>(5)?,
                ))
            }) {
                Ok(mapped) => mapped.filter_map(|r| r.ok()).collect(),
                Err(_) => return Ok(Some(Vec::new())),
            };

        let mut lenses = Vec::new();

        for (sym_id, name, line_start, _line_end, is_exported, is_dead) in symbols {
            let lsp_line = line_start.saturating_sub(1);
            let range = Range {
                start: Position {
                    line: lsp_line,
                    character: 0,
                },
                end: Position {
                    line: lsp_line,
                    character: 0,
                },
            };

            // Count callers
            let caller_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM calls WHERE callee_symbol_id = ?1",
                    params![sym_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            if caller_count > 0 {
                let title = if caller_count == 1 {
                    "1 reference".to_string()
                } else {
                    format!("{} references", caller_count)
                };
                lenses.push(CodeLens {
                    range,
                    command: Some(Command {
                        title,
                        command: "ariadne.showReferences".to_string(),
                        arguments: None,
                    }),
                    data: None,
                });
            } else if is_dead || !is_exported {
                // 0 callers and either DB-marked dead or not exported
                lenses.push(CodeLens {
                    range,
                    command: Some(Command {
                        title: format!("DEAD CODE - {}", name),
                        command: "ariadne.deadCode".to_string(),
                        arguments: None,
                    }),
                    data: None,
                });
            }
        }

        Ok(Some(lenses))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let db = match self.open_db() {
            Some(db) => db,
            None => return Ok(None),
        };

        let suffix = match Self::uri_to_path_suffix(uri) {
            Some(s) => s,
            None => return Ok(None),
        };

        let sym = match Self::find_symbol_at_position(&db, &suffix, position.line) {
            Some(s) => s,
            None => return Ok(None),
        };

        // Check if the symbol at cursor is actually a call site.
        // Look for a call on this line where the cursor symbol is the caller.
        let conn = db.conn();
        let db_line = position.line + 1;
        let pattern = format!("%{}", crate::db::escape_like(&suffix));

        // Try to find a callee defined at this call site
        let callee_location: Option<(String, u32)> = conn
            .prepare(
                "SELECT f2.path, s2.line_start
                 FROM calls c
                 JOIN symbols s2 ON c.callee_symbol_id = s2.id
                 JOIN files f2 ON s2.file_id = f2.id
                 JOIN files f1 ON c.file_id = f1.id
                 WHERE f1.path LIKE ?1 ESCAPE '\\' AND c.line = ?2
                 LIMIT 1",
            )
            .ok()
            .and_then(|mut stmt| {
                stmt.query_row(params![pattern, db_line], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
                })
                .ok()
            });

        if let Some((callee_path, callee_line)) = callee_location {
            // Resolve to absolute path if needed
            let abs_path = resolve_absolute_path(&db, &callee_path);
            let location = convert::to_lsp_location(&abs_path, callee_line, 0, 0);
            return Ok(Some(GotoDefinitionResponse::Scalar(location)));
        }

        // Otherwise, return the symbol's own definition location
        let abs_path = resolve_absolute_path(&db, &sym.file_path);
        let location = convert::to_lsp_location(&abs_path, sym.line_start, 0, 0);
        Ok(Some(GotoDefinitionResponse::Scalar(location)))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let db = match self.open_db() {
            Some(db) => db,
            None => return Ok(Some(Vec::new())),
        };

        let suffix = match Self::uri_to_path_suffix(uri) {
            Some(s) => s,
            None => return Ok(Some(Vec::new())),
        };

        let sym = match Self::find_symbol_at_position(&db, &suffix, position.line) {
            Some(s) => s,
            None => return Ok(Some(Vec::new())),
        };

        // Build the call graph and find all callers
        let graph = match crate::db::query::build_call_graph(&db, None) {
            Ok(g) => g,
            Err(_) => return Ok(Some(Vec::new())),
        };

        let node_idx = match graph.find_node(sym.id as u64) {
            Some(idx) => idx,
            None => return Ok(Some(Vec::new())),
        };

        let caller_indexes = graph.callers_of(node_idx);
        let mut locations = Vec::new();

        for caller_idx in caller_indexes {
            if let Some(caller_sym) = graph.get_symbol(caller_idx) {
                let abs_path = resolve_absolute_path(&db, &caller_sym.file_path);
                let location = convert::to_lsp_location(
                    &abs_path,
                    // Find the actual call line from the DB
                    find_call_line(&db, caller_sym.id, sym.id)
                        .unwrap_or(caller_sym.file_path.len() as u32),
                    0,
                    0,
                );
                locations.push(location);
            }
        }

        // Also include direct call sites from the calls table for more precise locations
        if locations.is_empty() {
            let conn = db.conn();
            if let Ok(mut stmt) = conn.prepare(
                "SELECT f.path, c.line
                 FROM calls c
                 JOIN files f ON c.file_id = f.id
                 WHERE c.callee_symbol_id = ?1",
            ) {
                if let Ok(rows) = stmt.query_map(params![sym.id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
                }) {
                    for row in rows.flatten() {
                        let abs_path = resolve_absolute_path(&db, &row.0);
                        locations.push(convert::to_lsp_location(&abs_path, row.1, 0, 0));
                    }
                }
            }
        }

        Ok(Some(locations))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let db = match self.open_db() {
            Some(db) => db,
            None => return Ok(None),
        };

        let suffix = match Self::uri_to_path_suffix(uri) {
            Some(s) => s,
            None => return Ok(None),
        };

        let sym = match Self::find_symbol_at_position(&db, &suffix, position.line) {
            Some(s) => s,
            None => return Ok(None),
        };

        // Count callers (incoming edges)
        let conn = db.conn();
        let caller_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM calls WHERE callee_symbol_id = ?1",
                params![sym.id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Count callees (outgoing edges)
        let callee_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM calls WHERE caller_symbol_id = ?1",
                params![sym.id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Build hover markdown
        let mut markdown = format!(
            "**{}** ({})\n\nFile: `{}:{}`\nCallers: {} | Callees: {}",
            sym.qualified_name, sym.kind, sym.file_path, sym.line_start, caller_count, callee_count
        );

        // Dead code warning: no callers and not exported
        if caller_count == 0 && !sym.is_exported {
            markdown.push_str(
                "\n\n**WARNING: Potential dead code** -- no incoming calls and not exported",
            );
        }

        // Also flag if the DB already marks it dead
        if sym.is_dead {
            if caller_count > 0 || sym.is_exported {
                // Already warned above, but if DB says dead, note it
            } else {
                // The warning above already covers this case
            }
        }

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: markdown,
            }),
            range: Some(Range {
                start: Position {
                    line: sym.line_start.saturating_sub(1),
                    character: 0,
                },
                end: Position {
                    line: sym.line_end.saturating_sub(1),
                    character: 0,
                },
            }),
        }))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;

        let db = match self.open_db() {
            Some(db) => db,
            None => return Ok(Some(DocumentSymbolResponse::Flat(Vec::new()))),
        };

        let suffix = match Self::uri_to_path_suffix(uri) {
            Some(s) => s,
            None => return Ok(Some(DocumentSymbolResponse::Flat(Vec::new()))),
        };

        let conn = db.conn();
        let pattern = format!("%{}", crate::db::escape_like(&suffix));

        let mut stmt = match conn.prepare(
            "SELECT s.name, s.kind, s.line_start, s.line_end, s.qualified_name
             FROM symbols s
             JOIN files f ON s.file_id = f.id
             WHERE f.path LIKE ?1
             ORDER BY s.line_start",
        ) {
            Ok(s) => s,
            Err(_) => return Ok(Some(DocumentSymbolResponse::Flat(Vec::new()))),
        };

        let rows: Vec<(String, String, u32, u32)> = match stmt.query_map(params![pattern], |row| {
            let name: String = row.get(0)?;
            let kind: String = row.get(1)?;
            let line_start: u32 = row.get(2)?;
            let line_end: u32 = row.get(3)?;
            let _qualified_name: String = row.get(4)?;
            Ok((name, kind, line_start, line_end))
        }) {
            Ok(mapped) => mapped.filter_map(|r| r.ok()).collect(),
            Err(_) => Vec::new(),
        };

        #[allow(deprecated)]
        let symbols: Vec<SymbolInformation> = rows
            .into_iter()
            .map(|(name, kind, line_start, line_end)| {
                let symbol_kind = convert::to_lsp_symbol_kind(&kind);
                #[allow(deprecated)]
                SymbolInformation {
                    name,
                    kind: symbol_kind,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: uri.clone(),
                        range: Range {
                            start: Position {
                                line: line_start.saturating_sub(1),
                                character: 0,
                            },
                            end: Position {
                                line: line_end.saturating_sub(1),
                                character: 0,
                            },
                        },
                    },
                    container_name: None,
                }
            })
            .collect();

        Ok(Some(DocumentSymbolResponse::Flat(symbols)))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let query = &params.query;

        if query.is_empty() {
            return Ok(Some(Vec::new()));
        }

        let db = match self.open_db() {
            Some(db) => db,
            None => return Ok(Some(Vec::new())),
        };

        let options = crate::search::SearchOptions {
            limit: Some(50),
            fuzzy: true,
            ..Default::default()
        };

        let results = match crate::search::search(&db, query, &options) {
            Ok(r) => r,
            Err(_) => return Ok(Some(Vec::new())),
        };

        #[allow(deprecated)]
        let symbols: Vec<SymbolInformation> = results
            .into_iter()
            .filter_map(|r| {
                let abs_path = resolve_absolute_path(&db, &r.file);
                let uri = Url::from_file_path(&abs_path).ok()?;
                let symbol_kind = convert::to_lsp_symbol_kind(&r.kind);

                #[allow(deprecated)]
                Some(SymbolInformation {
                    name: r.name,
                    kind: symbol_kind,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri,
                        range: Range {
                            start: Position {
                                line: r.line.saturating_sub(1),
                                character: 0,
                            },
                            end: Position {
                                line: r.line.saturating_sub(1),
                                character: 0,
                            },
                        },
                    },
                    container_name: r.qualified_name,
                })
            })
            .collect();

        Ok(Some(symbols))
    }
}

/// Find the line number of a specific call from caller to callee.
fn find_call_line(db: &Database, caller_id: i64, callee_id: i64) -> Option<u32> {
    db.conn()
        .query_row(
            "SELECT line FROM calls WHERE caller_symbol_id = ?1 AND callee_symbol_id = ?2 LIMIT 1",
            params![caller_id, callee_id],
            |row| row.get(0),
        )
        .ok()
}

/// Resolve a relative DB path to an absolute path using the file's absolute_path column.
fn resolve_absolute_path(db: &Database, relative_path: &str) -> String {
    db.conn()
        .query_row(
            "SELECT absolute_path FROM files WHERE path = ?1 LIMIT 1",
            params![relative_path],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| relative_path.to_string())
}

pub async fn serve_lsp_stdio(db_path: PathBuf) -> anyhow::Result<()> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| AriadneLsp::new(client, db_path.clone()));
    Server::new(stdin, stdout, socket).serve(service).await;
    Ok(())
}
