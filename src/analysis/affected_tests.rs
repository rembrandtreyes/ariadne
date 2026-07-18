use crate::db::Database;
use rusqlite::params;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Serialize)]
pub struct AffectedTestsResult {
    pub test_files: Vec<String>,
    pub test_functions: Vec<String>,
    pub changed_files: usize,
    pub total_tests_affected: usize,
}

pub fn find_affected_tests(db: &Database, diff_ref: &str) -> anyhow::Result<AffectedTestsResult> {
    // 1. Get changed files from git diff
    let changed_files = get_changed_files(diff_ref)?;
    find_affected_tests_for_files(db, &changed_files)
}

/// Core analysis over an explicit changed-file list (git-free, testable).
pub fn find_affected_tests_for_files(
    db: &Database,
    changed_files: &[String],
) -> anyhow::Result<AffectedTestsResult> {
    let conn = db.conn();
    let changed_count = changed_files.len();

    if changed_files.is_empty() {
        return Ok(AffectedTestsResult {
            test_files: Vec::new(),
            test_functions: Vec::new(),
            changed_files: 0,
            total_tests_affected: 0,
        });
    }

    // 2. Find symbol IDs in changed files
    let mut changed_symbol_ids: Vec<i64> = Vec::new();
    for file_path in changed_files {
        let mut stmt = conn.prepare(
            "SELECT id FROM files WHERE path LIKE '%' || ?1 OR absolute_path LIKE '%' || ?1",
        )?;
        let file_ids: Vec<i64> = stmt
            .query_map(params![file_path], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        for file_id in file_ids {
            let mut sym_stmt = conn.prepare("SELECT id FROM symbols WHERE file_id = ?1")?;
            let sym_ids: Vec<i64> = sym_stmt
                .query_map(params![file_id], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            changed_symbol_ids.extend(sym_ids);
        }
    }

    // 3. Build reverse call graph (callee -> callers)
    let mut callers_of: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut call_stmt = conn.prepare(
        "SELECT caller_symbol_id, callee_symbol_id FROM calls WHERE callee_symbol_id IS NOT NULL",
    )?;
    let calls: Vec<(i64, i64)> = call_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    for (caller, callee) in &calls {
        callers_of.entry(*callee).or_default().push(*caller);
    }

    // 4. BFS backward from changed symbols to find all transitively affected symbols
    let mut affected: HashSet<i64> = changed_symbol_ids.iter().copied().collect();
    let mut queue: VecDeque<i64> = changed_symbol_ids.into_iter().collect();

    while let Some(sym_id) = queue.pop_front() {
        if let Some(callers) = callers_of.get(&sym_id) {
            for &caller_id in callers {
                if affected.insert(caller_id) {
                    queue.push_back(caller_id);
                }
            }
        }
    }

    // 5. Filter to test symbols only
    let mut test_stmt = conn.prepare(
        "SELECT s.id, s.name, f.path FROM symbols s JOIN files f ON s.file_id = f.id WHERE s.is_test = 1",
    )?;
    let all_tests: Vec<(i64, String, String)> = test_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut test_functions = Vec::new();
    let mut test_file_set = HashSet::new();
    for (id, name, path) in &all_tests {
        if affected.contains(id) {
            test_functions.push(name.clone());
            test_file_set.insert(path.clone());
        }
    }

    // Sorted: set iteration order must never reach serialized output.
    let mut test_files: Vec<String> = test_file_set.into_iter().collect();
    test_files.sort_unstable();
    let total = test_functions.len();

    Ok(AffectedTestsResult {
        test_files,
        test_functions,
        changed_files: changed_count,
        total_tests_affected: total,
    })
}

/// Get changed files using git2 to parse the diff between refs.
fn get_changed_files(diff_ref: &str) -> anyhow::Result<Vec<String>> {
    let repo = git2::Repository::discover(".")?;

    let (old_tree, new_tree) = if diff_ref.contains("..") {
        // Range format: "main..HEAD"
        let parts: Vec<&str> = diff_ref.split("..").collect();
        let from = repo.revparse_single(parts[0])?.peel_to_tree()?;
        let to = repo.revparse_single(parts[1])?.peel_to_tree()?;
        (from, to)
    } else {
        // Single ref: diff between ref and HEAD
        let old = repo.revparse_single(diff_ref)?.peel_to_tree()?;
        let new = repo.revparse_single("HEAD")?.peel_to_tree()?;
        (old, new)
    };

    let diff = repo.diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)?;

    let mut files = Vec::new();
    for delta in diff.deltas() {
        if let Some(path) = delta.new_file().path() {
            files.push(path.to_string_lossy().to_string());
        } else if let Some(path) = delta.old_file().path() {
            files.push(path.to_string_lossy().to_string());
        }
    }
    Ok(files)
}
