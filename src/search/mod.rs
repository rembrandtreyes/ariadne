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
    match fts_search(db, query, limit, options) {
        Ok(fts) => results = fts,
        Err(e) => {
            // LIKE below covers the miss, but a broken FTS index should not
            // degrade search silently.
            tracing::warn!(error = %e, "FTS search failed; falling back to LIKE");
        }
    }

    // Fall back to LIKE-based search if FTS returns nothing
    if results.is_empty() {
        results = like_search(db, query, limit, options)?;
    }

    // If fuzzy is enabled and results are too few, supplement with fuzzy
    if options.fuzzy && results.len() < limit {
        let remaining = limit - results.len();
        let fuzzy = fuzzy_search(db, query, remaining, options)?;
        results.extend(fuzzy);
        // The fuzzy pass re-finds symbols the passes above already returned —
        // keep the first (higher-priority) occurrence of each symbol id.
        let mut seen = std::collections::HashSet::new();
        results.retain(|r| match r.symbol_id {
            Some(id) => seen.insert(id),
            None => true,
        });
    }

    // Apply fuzzy re-ranking if enabled
    if options.fuzzy && !results.is_empty() {
        for result in &mut results {
            result.score = strsim::jaro_winkler(&result.name, query);
        }
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    results.truncate(limit);
    Ok(results)
}

/// Build the extra WHERE fragment and parameter values for the optional
/// kind/language/file filters. Numbering starts at `?3` because every search
/// query binds the match pattern as `?1` and the limit as `?2`. Applying
/// filters in SQL (not post-filtering) keeps LIMIT from starving results.
fn filter_sql(options: &SearchOptions) -> (String, Vec<String>) {
    let mut sql = String::new();
    let mut params = Vec::new();
    let mut idx = 3;
    if let Some(kind) = &options.kind_filter {
        sql.push_str(&format!(" AND s.kind = ?{idx}"));
        params.push(kind.clone());
        idx += 1;
    }
    if let Some(lang) = &options.language_filter {
        sql.push_str(&format!(" AND f.language = ?{idx}"));
        params.push(lang.clone());
        idx += 1;
    }
    if let Some(file) = &options.file_filter {
        sql.push_str(&format!(" AND f.path LIKE ?{idx} ESCAPE '\\'"));
        params.push(format!("%{}%", crate::db::escape_like(file)));
    }
    (sql, params)
}

/// Bind (pattern, limit, filters...) positionally: iterator order matches
/// the `?1`, `?2`, `?3`… numbering used by all three search queries.
fn bind_params(
    pattern: &str,
    limit: i64,
    filters: &[String],
) -> Vec<Box<dyn rusqlite::types::ToSql>> {
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
        vec![Box::new(pattern.to_string()), Box::new(limit)];
    for f in filters {
        params.push(Box::new(f.clone()));
    }
    params
}

/// FTS5-based full-text search on the symbols_fts table.
fn fts_search(
    db: &crate::db::Database,
    query: &str,
    limit: usize,
    options: &SearchOptions,
) -> anyhow::Result<Vec<SearchResult>> {
    let conn = db.conn();
    let escaped = query.replace('"', "\"\"");
    let fts_query = format!("\"{}\"*", escaped);
    let (extra_sql, filter_params) = filter_sql(options);
    let mut stmt = conn.prepare(&format!(
        "SELECT s.id, s.name, s.qualified_name, s.kind, f.path, s.line_start, rank
         FROM symbols_fts fts
         JOIN symbols s ON fts.rowid = s.id
         JOIN files f ON s.file_id = f.id
         WHERE symbols_fts MATCH ?1{extra_sql}
         ORDER BY rank
         LIMIT ?2",
    ))?;

    let params = bind_params(&fts_query, limit as i64, &filter_params);
    let results = stmt
        .query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| {
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
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}

/// LIKE-based fallback search.
fn like_search(
    db: &crate::db::Database,
    query: &str,
    limit: usize,
    options: &SearchOptions,
) -> anyhow::Result<Vec<SearchResult>> {
    let conn = db.conn();
    let (extra_sql, filter_params) = filter_sql(options);
    let mut stmt = conn.prepare(&format!(
        "SELECT s.id, s.name, s.qualified_name, s.kind, f.path, s.line_start
         FROM symbols s
         JOIN files f ON s.file_id = f.id
         WHERE s.name LIKE ?1 ESCAPE '\\'{extra_sql}
         ORDER BY s.name
         LIMIT ?2",
    ))?;

    let pattern = format!("%{}%", crate::db::escape_like(query));
    let params = bind_params(&pattern, limit as i64, &filter_params);
    let results = stmt
        .query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| {
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
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}

/// Fuzzy search using Levenshtein distance on symbol names.
fn fuzzy_search(
    db: &crate::db::Database,
    query: &str,
    limit: usize,
    options: &SearchOptions,
) -> anyhow::Result<Vec<SearchResult>> {
    let conn = db.conn();
    // Fetch a bounded number of candidates, then compute Levenshtein distance in Rust.
    // Use a short prefix filter (first 2 chars) to narrow candidates while still allowing
    // fuzzy matches where the full query isn't a substring of the symbol name.
    let prefix: String = query.chars().take(2).collect();
    let prefix_pattern = if prefix.chars().count() >= 2 {
        format!("{}%", crate::db::escape_like(&prefix))
    } else {
        "%".to_string()
    };
    let fetch_limit = (limit * 10).min(5000) as i64;
    let (extra_sql, filter_params) = filter_sql(options);
    let mut stmt = conn.prepare(&format!(
        "SELECT s.id, s.name, s.qualified_name, s.kind, f.path, s.line_start
         FROM symbols s JOIN files f ON s.file_id = f.id
         WHERE s.name LIKE ?1 ESCAPE '\\'{extra_sql}
         LIMIT ?2",
    ))?;

    let params = bind_params(&prefix_pattern, fetch_limit, &filter_params);
    let mut results: Vec<SearchResult> = stmt
        .query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| {
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
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit);

    Ok(results)
}
