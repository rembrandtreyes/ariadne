//! Pipeline Phase 9: Temporal Coupling Analysis
//!
//! Reads: `files` (to resolve file IDs from git paths)
//! Writes: `coupling` (file pairs with co-change counts and strength)
//!
//! Analyzes git history to detect files that frequently change together,
//! revealing implicit coupling not visible through imports or call graphs.

use crate::db::Database;
use std::path::Path;

/// Phase 9: Analyze git-based temporal coupling between files.
///
/// Uses git log to find files that frequently change together,
/// indicating implicit coupling that may not be visible in imports.
pub fn analyze_coupling(db: &Database, root: &Path) -> anyhow::Result<()> {
    // Attempt to open the git repository
    let repo = match git2::Repository::discover(root) {
        Ok(repo) => repo,
        Err(_) => {
            // No git repository found; skip coupling analysis
            return Ok(());
        }
    };

    // Walk the commit history (limited to recent commits for performance)
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    let mut commit_files: Vec<Vec<String>> = Vec::new();
    let max_commits = 500;
    let mut count = 0;

    for oid in revwalk {
        if count >= max_commits {
            break;
        }
        let oid = oid?;
        let commit = repo.find_commit(oid)?;

        if commit.parent_count() == 0 {
            count += 1;
            continue;
        }

        let parent = commit.parent(0)?;
        let diff = repo.diff_tree_to_tree(Some(&parent.tree()?), Some(&commit.tree()?), None)?;

        let mut files_in_commit = Vec::new();
        diff.foreach(
            &mut |delta, _| {
                if let Some(path) = delta.new_file().path() {
                    files_in_commit.push(path.to_string_lossy().to_string());
                }
                true
            },
            None,
            None,
            None,
        )?;

        if files_in_commit.len() > 1 && files_in_commit.len() <= 20 {
            commit_files.push(files_in_commit);
        }

        count += 1;
    }

    // Count co-changes between file pairs
    let mut co_changes: std::collections::HashMap<(String, String), i32> =
        std::collections::HashMap::new();
    for files in &commit_files {
        for i in 0..files.len() {
            for j in (i + 1)..files.len() {
                let pair = if files[i] < files[j] {
                    (files[i].clone(), files[j].clone())
                } else {
                    (files[j].clone(), files[i].clone())
                };
                *co_changes.entry(pair).or_insert(0) += 1;
            }
        }
    }

    // Insert coupling records for pairs that co-changed multiple times
    let conn = db.conn();
    for ((file_a, file_b), changes) in &co_changes {
        if *changes < 2 {
            continue;
        }

        let file_a_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM files WHERE path LIKE '%' || ?1",
                rusqlite::params![file_a],
                |row| row.get(0),
            )
            .ok();

        let file_b_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM files WHERE path LIKE '%' || ?1",
                rusqlite::params![file_b],
                |row| row.get(0),
            )
            .ok();

        if let (Some(a_id), Some(b_id)) = (file_a_id, file_b_id) {
            let total_commits = commit_files.len().max(1) as f64;
            let strength = *changes as f64 / total_commits;
            crate::db::write::insert_coupling(db, a_id, b_id, *changes, strength)?;
        }
    }

    Ok(())
}
