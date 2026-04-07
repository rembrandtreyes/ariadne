//! Pipeline Phase 10: Symbol Temporal History
//!
//! Reads: `files`, `symbols` (to map blame lines to symbol spans)
//! Writes: `symbol_history` (per-symbol aggregate: created_at, last_modified, count, authors, volatile)
//!
//! Uses git blame to determine when each symbol was created/modified and by how many authors.
//! Gracefully skips if no git repository is found or on shallow clones.

use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

use crate::db::query;
use crate::db::Database;

/// Per-symbol blame aggregate before DB insertion.
struct SymbolBlame {
    symbol_id: i64,
    oldest_timestamp: i64,
    newest_timestamp: i64,
    authors: HashSet<u64>,
    commit_set: HashSet<u64>,
}

/// Result of mapping blame data to a single symbol.
pub struct SymbolBlameResult {
    pub symbol_id: i64,
    pub created_at: Option<i64>,
    pub last_modified_at: Option<i64>,
    pub modification_count: i32,
    pub author_count: i32,
}

/// Map blame data to symbol spans. Pure function, testable without git2.
///
/// `blame_lines`: (line_number_1indexed, timestamp_secs, author_hash, commit_hash)
/// `symbols`: (symbol_id, line_start_1indexed, line_end_1indexed)
pub fn map_blame_to_symbols(
    blame_lines: &[(u32, i64, u64, u64)],
    symbols: &[(i64, u32, u32)],
) -> Vec<SymbolBlameResult> {
    let mut results = Vec::with_capacity(symbols.len());

    for &(sym_id, line_start, line_end) in symbols {
        let mut sb = SymbolBlame {
            symbol_id: sym_id,
            oldest_timestamp: i64::MAX,
            newest_timestamp: i64::MIN,
            authors: HashSet::new(),
            commit_set: HashSet::new(),
        };

        for &(line, ts, author_hash, commit_hash) in blame_lines {
            if line >= line_start && line <= line_end {
                if ts < sb.oldest_timestamp {
                    sb.oldest_timestamp = ts;
                }
                if ts > sb.newest_timestamp {
                    sb.newest_timestamp = ts;
                }
                sb.authors.insert(author_hash);
                sb.commit_set.insert(commit_hash);
            }
        }

        if sb.commit_set.is_empty() {
            results.push(SymbolBlameResult {
                symbol_id: sb.symbol_id,
                created_at: None,
                last_modified_at: None,
                modification_count: 0,
                author_count: 0,
            });
        } else {
            results.push(SymbolBlameResult {
                symbol_id: sb.symbol_id,
                created_at: Some(sb.oldest_timestamp),
                last_modified_at: Some(sb.newest_timestamp),
                modification_count: sb.commit_set.len() as i32,
                author_count: sb.authors.len() as i32,
            });
        }
    }

    results
}

/// Phase 10: Analyze git blame to build per-symbol temporal history.
pub fn analyze_git_history(db: &Database, root: &Path) -> anyhow::Result<()> {
    let repo = match git2::Repository::discover(root) {
        Ok(repo) => repo,
        Err(_) => return Ok(()),
    };

    // Skip shallow clones where blame data is unreliable
    if repo.is_shallow() {
        return Ok(());
    }

    let phase_start = Instant::now();
    let phase_timeout = std::time::Duration::from_secs(30);
    let file_timeout = std::time::Duration::from_millis(500);

    let files = query::all_files(db)?;

    // Determine a 30-day-ago threshold for volatility calculation
    let thirty_days_ago = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64 - 30 * 86400)
        .unwrap_or(0);

    for file_row in &files {
        if phase_start.elapsed() > phase_timeout {
            break;
        }

        let file_start = Instant::now();

        // Get symbols for this file
        let symbols = match query::get_file_symbols(db, file_row.id) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if symbols.is_empty() {
            continue;
        }

        // Resolve the relative path for blame
        let blame_path = std::path::Path::new(&file_row.path);

        // Run git blame on this file
        let blame = match repo.blame_file(blame_path, None) {
            Ok(b) => b,
            Err(_) => continue,
        };

        if file_start.elapsed() > file_timeout {
            continue;
        }

        // Build blame lines: (line_1indexed, timestamp, author_hash, commit_hash)
        let mut blame_lines = Vec::new();
        for hunk_idx in 0..blame.len() {
            let hunk = blame.get_index(hunk_idx);
            let Some(hunk) = hunk else { continue };

            let oid = hunk.final_commit_id();
            let commit_hash = hash_oid(&oid);

            // Get commit timestamp and author
            let (timestamp, author_hash) = match repo.find_commit(oid) {
                Ok(commit) => {
                    let ts = commit.time().seconds();
                    let sig = commit.author();
                    let author_name = sig.name().unwrap_or("unknown");
                    let ah = hash_str(author_name);
                    (ts, ah)
                }
                Err(_) => continue,
            };

            let start_line = hunk.final_start_line();
            let line_count = hunk.lines_in_hunk();
            for offset in 0..line_count {
                blame_lines.push((
                    (start_line + offset) as u32,
                    timestamp,
                    author_hash,
                    commit_hash,
                ));
            }
        }

        if file_start.elapsed() > file_timeout {
            continue;
        }

        // Build symbol spans
        let symbol_spans: Vec<(i64, u32, u32)> = symbols
            .iter()
            .map(|s| (s.id, s.line_start, s.line_end))
            .collect();

        // Map blame to symbols (pure function)
        let aggregates = map_blame_to_symbols(&blame_lines, &symbol_spans);

        // Insert results
        for agg in &aggregates {
            // Determine volatility: >3 distinct commits touching this symbol in last 30 days
            let recent_commits = blame_lines
                .iter()
                .filter(|(line, ts, _, _)| {
                    let sym = symbol_spans.iter().find(|(id, _, _)| *id == agg.symbol_id);
                    if let Some(&(_, ls, le)) = sym {
                        *line >= ls && *line <= le && *ts >= thirty_days_ago
                    } else {
                        false
                    }
                })
                .map(|(_, _, _, ch)| *ch)
                .collect::<HashSet<_>>();

            let is_volatile = recent_commits.len() > 3;

            crate::db::write::insert_symbol_history(
                db,
                agg.symbol_id,
                agg.created_at,
                agg.last_modified_at,
                agg.modification_count,
                agg.author_count,
                is_volatile,
            )?;
        }
    }

    Ok(())
}

fn hash_oid(oid: &git2::Oid) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    oid.as_bytes().hash(&mut hasher);
    hasher.finish()
}

fn hash_str(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_blame_to_symbols_basic() {
        // 3 blame lines, 1 symbol spanning lines 1-3
        let blame_lines = vec![
            (1u32, 1000i64, 100u64, 200u64), // line 1, ts=1000, author=100, commit=200
            (2, 2000, 100, 201),             // line 2, ts=2000, same author, different commit
            (3, 3000, 101, 202),             // line 3, ts=3000, different author
        ];
        let symbols = vec![(1i64, 1u32, 3u32)]; // symbol_id=1, lines 1-3

        let results = map_blame_to_symbols(&blame_lines, &symbols);
        assert_eq!(results.len(), 1);
        let r = &results[0];
        assert_eq!(r.symbol_id, 1);
        assert_eq!(r.created_at, Some(1000));
        assert_eq!(r.last_modified_at, Some(3000));
        assert_eq!(r.modification_count, 3); // 3 distinct commits
        assert_eq!(r.author_count, 2); // 2 distinct authors
    }

    #[test]
    fn test_map_blame_to_symbols_nested() {
        // Lines 1-10: outer function, lines 3-7: inner function
        let blame_lines = vec![
            (1, 1000, 100, 200),
            (3, 2000, 101, 201),
            (5, 3000, 100, 202),
            (7, 4000, 102, 203),
            (10, 5000, 100, 204),
        ];
        let symbols = vec![
            (1i64, 1u32, 10u32), // outer (all lines)
            (2i64, 3u32, 7u32),  // inner (lines 3-7)
        ];

        let results = map_blame_to_symbols(&blame_lines, &symbols);
        assert_eq!(results.len(), 2);

        // Outer: all 5 lines, 5 commits, 3 authors
        assert_eq!(results[0].symbol_id, 1);
        assert_eq!(results[0].modification_count, 5);
        assert_eq!(results[0].author_count, 3);

        // Inner: lines 3,5,7 -> 3 commits, 3 authors
        assert_eq!(results[1].symbol_id, 2);
        assert_eq!(results[1].modification_count, 3);
        assert_eq!(results[1].author_count, 3);
    }

    #[test]
    fn test_map_blame_to_symbols_empty() {
        let blame_lines: Vec<(u32, i64, u64, u64)> = vec![];
        let symbols = vec![(1i64, 1u32, 5u32)];

        let results = map_blame_to_symbols(&blame_lines, &symbols);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].created_at, None);
        assert_eq!(results[0].last_modified_at, None);
        assert_eq!(results[0].modification_count, 0);
        assert_eq!(results[0].author_count, 0);
    }

    #[test]
    fn test_map_blame_to_symbols_no_overlap() {
        // Blame lines outside symbol span
        let blame_lines = vec![(10, 1000, 100, 200), (11, 2000, 101, 201)];
        let symbols = vec![(1i64, 1u32, 5u32)]; // symbol is at lines 1-5

        let results = map_blame_to_symbols(&blame_lines, &symbols);
        assert_eq!(results[0].modification_count, 0); // no matching commits
    }
}
