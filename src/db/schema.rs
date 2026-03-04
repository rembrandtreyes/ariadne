use rusqlite::Connection;

const SCHEMA_SQL: &str = "
-- Services table
CREATE TABLE IF NOT EXISTS services (
    id INTEGER PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    type TEXT NOT NULL DEFAULT 'microservice',
    repo_path TEXT NOT NULL,
    base_url TEXT DEFAULT '',
    primary_language TEXT DEFAULT '',
    last_indexed REAL
);

-- Files table
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY,
    service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    absolute_path TEXT UNIQUE NOT NULL,
    language TEXT NOT NULL,
    last_modified REAL NOT NULL,
    last_indexed REAL NOT NULL,
    community_id INTEGER
);

-- Symbols table
CREATE TABLE IF NOT EXISTS symbols (
    id INTEGER PRIMARY KEY,
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    qualified_name TEXT NOT NULL,
    kind TEXT NOT NULL,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    parent_symbol_id INTEGER REFERENCES symbols(id) ON DELETE CASCADE,
    is_exported BOOLEAN DEFAULT FALSE,
    is_entry_point BOOLEAN DEFAULT FALSE,
    is_dead BOOLEAN DEFAULT FALSE,
    is_test BOOLEAN DEFAULT FALSE,
    decorators TEXT DEFAULT '',
    signature TEXT DEFAULT '',
    community_id INTEGER
);

-- Imports table
CREATE TABLE IF NOT EXISTS imports (
    id INTEGER PRIMARY KEY,
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    imported_name TEXT NOT NULL,
    module_path TEXT NOT NULL,
    resolved_file_id INTEGER REFERENCES files(id),
    resolved_symbol_id INTEGER REFERENCES symbols(id),
    line INTEGER NOT NULL,
    is_external BOOLEAN DEFAULT FALSE
);

-- Calls table
CREATE TABLE IF NOT EXISTS calls (
    id INTEGER PRIMARY KEY,
    caller_symbol_id INTEGER NOT NULL REFERENCES symbols(id) ON DELETE CASCADE,
    callee_symbol_id INTEGER REFERENCES symbols(id),
    callee_name TEXT NOT NULL,
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    line INTEGER NOT NULL,
    confidence REAL NOT NULL DEFAULT 0.5,
    resolution TEXT NOT NULL DEFAULT 'unresolved'
);

-- Heritage table
CREATE TABLE IF NOT EXISTS heritage (
    id INTEGER PRIMARY KEY,
    child_symbol_id INTEGER NOT NULL REFERENCES symbols(id) ON DELETE CASCADE,
    parent_name TEXT NOT NULL,
    parent_symbol_id INTEGER REFERENCES symbols(id),
    kind TEXT NOT NULL DEFAULT 'extends'
);

-- Git coupling table
CREATE TABLE IF NOT EXISTS coupling (
    id INTEGER PRIMARY KEY,
    file_a_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    file_b_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    co_changes INTEGER NOT NULL,
    strength REAL NOT NULL,
    UNIQUE(file_a_id, file_b_id)
);

-- Execution flows
CREATE TABLE IF NOT EXISTS flows (
    id INTEGER PRIMARY KEY,
    entry_symbol_id INTEGER NOT NULL REFERENCES symbols(id) ON DELETE CASCADE,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS flow_steps (
    id INTEGER PRIMARY KEY,
    flow_id INTEGER NOT NULL REFERENCES flows(id) ON DELETE CASCADE,
    symbol_id INTEGER NOT NULL REFERENCES symbols(id) ON DELETE CASCADE,
    step_order INTEGER NOT NULL,
    depth INTEGER NOT NULL
);

-- API boundary tables
CREATE TABLE IF NOT EXISTS api_endpoints (
    id INTEGER PRIMARY KEY,
    service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    handler_symbol_id INTEGER REFERENCES symbols(id) ON DELETE SET NULL,
    method TEXT NOT NULL,
    path_pattern TEXT NOT NULL,
    protocol TEXT NOT NULL DEFAULT 'http',
    file_id INTEGER REFERENCES files(id) ON DELETE CASCADE,
    line INTEGER,
    schema_source TEXT,
    UNIQUE(service_id, method, path_pattern, protocol)
);

CREATE TABLE IF NOT EXISTS api_calls (
    id INTEGER PRIMARY KEY,
    service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    caller_symbol_id INTEGER REFERENCES symbols(id) ON DELETE SET NULL,
    method TEXT NOT NULL,
    url_pattern TEXT NOT NULL,
    protocol TEXT NOT NULL DEFAULT 'http',
    file_id INTEGER REFERENCES files(id) ON DELETE CASCADE,
    line INTEGER,
    is_dynamic BOOLEAN DEFAULT FALSE,
    resolved_endpoint_id INTEGER REFERENCES api_endpoints(id),
    resolved_service_id INTEGER REFERENCES services(id)
);

CREATE TABLE IF NOT EXISTS service_edges (
    id INTEGER PRIMARY KEY,
    from_service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    to_service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    protocol TEXT NOT NULL DEFAULT 'http',
    call_count INTEGER NOT NULL DEFAULT 0,
    confidence REAL NOT NULL DEFAULT 0.5,
    UNIQUE(from_service_id, to_service_id, protocol)
);

-- Communities
CREATE TABLE IF NOT EXISTS communities (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    symbol_count INTEGER NOT NULL,
    internal_edges INTEGER NOT NULL,
    external_edges INTEGER NOT NULL,
    modularity REAL NOT NULL
);

-- Architectural rules
CREATE TABLE IF NOT EXISTS rule_violations (
    id INTEGER PRIMARY KEY,
    rule_name TEXT NOT NULL,
    from_file_id INTEGER REFERENCES files(id),
    to_file_id INTEGER REFERENCES files(id),
    from_symbol TEXT,
    to_symbol TEXT,
    line INTEGER,
    severity TEXT NOT NULL DEFAULT 'error'
);

-- Metadata key-value store
CREATE TABLE IF NOT EXISTS metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- All indexes
CREATE INDEX IF NOT EXISTS idx_files_service ON files(service_id);
CREATE INDEX IF NOT EXISTS idx_files_path ON files(absolute_path);
CREATE INDEX IF NOT EXISTS idx_files_community ON files(community_id);
CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
CREATE INDEX IF NOT EXISTS idx_symbols_qualified ON symbols(qualified_name);
CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_id);
CREATE INDEX IF NOT EXISTS idx_symbols_dead ON symbols(is_dead) WHERE is_dead = 1;
CREATE INDEX IF NOT EXISTS idx_symbols_test ON symbols(is_test) WHERE is_test = 1;
CREATE INDEX IF NOT EXISTS idx_symbols_community ON symbols(community_id);
CREATE INDEX IF NOT EXISTS idx_imports_file ON imports(file_id);
CREATE INDEX IF NOT EXISTS idx_imports_resolved ON imports(resolved_symbol_id);
CREATE INDEX IF NOT EXISTS idx_calls_caller ON calls(caller_symbol_id);
CREATE INDEX IF NOT EXISTS idx_calls_callee ON calls(callee_symbol_id);
CREATE INDEX IF NOT EXISTS idx_heritage_child ON heritage(child_symbol_id);
CREATE INDEX IF NOT EXISTS idx_heritage_parent ON heritage(parent_symbol_id);
CREATE INDEX IF NOT EXISTS idx_coupling_a ON coupling(file_a_id);
CREATE INDEX IF NOT EXISTS idx_coupling_b ON coupling(file_b_id);
CREATE INDEX IF NOT EXISTS idx_api_endpoints_service ON api_endpoints(service_id);
CREATE INDEX IF NOT EXISTS idx_api_endpoints_path ON api_endpoints(path_pattern);
CREATE INDEX IF NOT EXISTS idx_api_calls_service ON api_calls(service_id);
CREATE INDEX IF NOT EXISTS idx_service_edges_from ON service_edges(from_service_id);
CREATE INDEX IF NOT EXISTS idx_service_edges_to ON service_edges(to_service_id);
";

/// Create all schema tables and indexes.
pub fn create_tables(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(SCHEMA_SQL)?;

    // FTS5 virtual table for hybrid search
    let fts_exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='symbols_fts'",
        [],
        |row| row.get(0),
    )?;
    if !fts_exists {
        conn.execute_batch(
            "CREATE VIRTUAL TABLE symbols_fts USING fts5(
                name, qualified_name, signature,
                content=symbols, content_rowid=id
            );",
        )?;
    }

    Ok(())
}
