use serde::{Deserialize, Serialize};

/// A single search result returned from a query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The matched symbol name
    pub name: String,
    /// Fully qualified name
    pub qualified_name: Option<String>,
    /// Database symbol id
    pub symbol_id: Option<i64>,
    /// The kind of symbol (function, class, etc.)
    pub kind: String,
    /// The file path where the symbol is defined
    pub file: String,
    /// The line number where the symbol starts
    pub line: u32,
    /// Relevance score (higher is better)
    pub score: f64,
    /// Optional snippet of source code around the match
    pub snippet: Option<String>,
}

/// Options for controlling search behavior.
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    /// Maximum number of results to return
    pub limit: Option<usize>,
    /// Filter by symbol kind
    pub kind_filter: Option<String>,
    /// Filter by language
    pub language_filter: Option<String>,
    /// Filter by file path pattern
    pub file_filter: Option<String>,
    /// Whether to use fuzzy matching
    pub fuzzy: bool,
}

/// Execute a search query against the indexed codebase.
///
/// Supports exact matches, prefix matches, and fuzzy matching
/// using string similarity (strsim crate).
pub fn search(
    db: &crate::db::Database,
    query: &str,
    options: &SearchOptions,
) -> anyhow::Result<Vec<SearchResult>> {
    let limit = options.limit.unwrap_or(50);
    let mut results = Vec::new();

    // Try FTS5 first
    let fts_results = fts_search(db, query, limit);
    if let Ok(fts) = fts_results {
        results = fts;
    }

    // Fall back to LIKE-based search if FTS returns nothing
    if results.is_empty() {
        results = like_search(db, query, limit)?;
    }

    // If fuzzy is enabled and results are too few, supplement with fuzzy
    if options.fuzzy && results.len() < limit {
        let remaining = limit - results.len();
        let fuzzy = fuzzy_search(db, query, remaining)?;
        results.extend(fuzzy);
    }

    // Apply fuzzy re-ranking if enabled
    if options.fuzzy && !results.is_empty() {
        for result in &mut results {
            result.score = strsim::jaro_winkler(&result.name, query);
        }
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    }

    results.truncate(limit);
    Ok(results)
}

/// FTS5-based full-text search on the symbols_fts table.
fn fts_search(
    db: &crate::db::Database,
    query: &str,
    limit: usize,
) -> anyhow::Result<Vec<SearchResult>> {
    let conn = db.conn();
    let escaped = query.replace('"', "\"\"");
    let fts_query = format!("\"{}\"*", escaped);
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name, s.qualified_name, s.kind, f.path, s.line_start, rank
         FROM symbols_fts fts
         JOIN symbols s ON fts.rowid = s.id
         JOIN files f ON s.file_id = f.id
         WHERE symbols_fts MATCH ?1
         ORDER BY rank
         LIMIT ?2",
    )?;

    let results = stmt
        .query_map(rusqlite::params![fts_query, limit as i64], |row| {
            Ok(SearchResult {
                symbol_id: Some(row.get(0)?),
                name: row.get(1)?,
                qualified_name: Some(row.get(2)?),
                kind: row.get(3)?,
                file: row.get(4)?,
                line: row.get(5)?,
                score: -(row.get::<_, f64>(6)?),
                snippet: None,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}

/// LIKE-based fallback search.
fn like_search(
    db: &crate::db::Database,
    query: &str,
    limit: usize,
) -> anyhow::Result<Vec<SearchResult>> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name, s.qualified_name, s.kind, f.path, s.line_start
         FROM symbols s
         JOIN files f ON s.file_id = f.id
         WHERE s.name LIKE ?1 ESCAPE '\\'
         ORDER BY s.name
         LIMIT ?2",
    )?;

    let pattern = format!("%{}%", crate::db::escape_like(query));
    let results = stmt
        .query_map(rusqlite::params![pattern, limit as i64], |row| {
            Ok(SearchResult {
                symbol_id: Some(row.get(0)?),
                name: row.get(1)?,
                qualified_name: Some(row.get(2)?),
                kind: row.get(3)?,
                file: row.get(4)?,
                line: row.get(5)?,
                score: 1.0,
                snippet: None,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}

/// Fuzzy search using Levenshtein distance on symbol names.
fn fuzzy_search(
    db: &crate::db::Database,
    query: &str,
    limit: usize,
) -> anyhow::Result<Vec<SearchResult>> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name, s.qualified_name, s.kind, f.path, s.line_start
         FROM symbols s JOIN files f ON s.file_id = f.id
         LIMIT 1000",
    )?;

    let mut results: Vec<SearchResult> = stmt
        .query_map([], |row| {
            let name: String = row.get(1)?;
            let distance = strsim::levenshtein(query, &name);
            let max_len = query.len().max(name.len());
            let score = if max_len > 0 {
                1.0 - (distance as f64 / max_len as f64)
            } else {
                0.0
            };

            Ok(SearchResult {
                symbol_id: Some(row.get(0)?),
                name,
                qualified_name: Some(row.get(2)?),
                kind: row.get(3)?,
                file: row.get(4)?,
                line: row.get(5)?,
                score,
                snippet: None,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);

    Ok(results)
}
