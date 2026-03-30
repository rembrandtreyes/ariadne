pub mod debounce;
pub mod incremental;

use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

pub struct FileWatcher {
    _watcher: notify::RecommendedWatcher,
    receiver: mpsc::Receiver<notify::Result<Event>>,
}

impl FileWatcher {
    pub fn new(root: &Path) -> anyhow::Result<Self> {
        let (tx, rx) = mpsc::channel();

        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })?;

        watcher.watch(root, RecursiveMode::Recursive)?;

        Ok(Self {
            _watcher: watcher,
            receiver: rx,
        })
    }

    pub fn poll_events(&self, timeout: Duration) -> Vec<Event> {
        let mut events = Vec::new();

        while let Ok(Ok(event)) = self.receiver.recv_timeout(timeout) {
            match event.kind {
                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                    events.push(event);
                }
                _ => {}
            }
        }

        events
    }
}

/// Watch for file changes and re-index them in the database.
pub fn watch_and_reindex(root: &Path, db_path: &Path, debounce_ms: u64) -> anyhow::Result<()> {
    let db = crate::db::Database::open(db_path)?;
    let watcher = FileWatcher::new(root)?;
    let debounce = Duration::from_millis(debounce_ms);

    tracing::info!(path = %root.display(), "Watching for changes");

    loop {
        let events = watcher.poll_events(debounce);
        if !events.is_empty() {
            // Separate events by kind: creates/modifies vs removals
            let mut modified_paths = Vec::new();
            let mut deleted_paths = Vec::new();

            for event in &events {
                for path in &event.paths {
                    let is_indexable = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|ext| crate::parse::types::Language::from_extension(ext).is_some())
                        .unwrap_or(false);

                    if !is_indexable {
                        continue;
                    }

                    match event.kind {
                        EventKind::Remove(_) => deleted_paths.push(path.as_path()),
                        EventKind::Create(_) | EventKind::Modify(_) => {
                            modified_paths.push(path.as_path())
                        }
                        _ => {}
                    }
                }
            }

            let has_changes = !modified_paths.is_empty() || !deleted_paths.is_empty();

            if !deleted_paths.is_empty() {
                tracing::info!(count = deleted_paths.len(), "Cleaning up deleted files");
                if let Err(e) = incremental::handle_deleted_files(&db, &deleted_paths) {
                    tracing::error!(error = %e, "Delete cleanup error");
                }
            }

            if !modified_paths.is_empty() {
                tracing::info!(count = modified_paths.len(), "Re-indexing changed files");
                if let Err(e) = incremental::reindex_files(&db, root, &modified_paths) {
                    tracing::error!(error = %e, "Re-index error");
                }
            }

            // Re-run resolution phases and bump generation so MCP cache refreshes
            if has_changes {
                if let Err(e) = incremental::run_post_reindex_resolution(&db) {
                    tracing::error!(error = %e, "Post-reindex resolution error");
                }
                if let Err(e) = incremental::bump_generation(&db) {
                    tracing::error!(error = %e, "Generation bump error");
                }
                let rate = crate::db::query::resolution_rate(&db).unwrap_or(0.0);
                tracing::info!(
                    resolution_rate = format!("{:.0}%", rate * 100.0),
                    "Incremental reindex complete"
                );
            }
        }
    }
}

/// Watch for file changes and simultaneously serve the MCP server.
pub async fn watch_and_serve(
    root: std::path::PathBuf,
    db_path: std::path::PathBuf,
    debounce_ms: u64,
) -> anyhow::Result<()> {
    tracing::info!("Starting watch mode with MCP server");

    // Spawn the file watcher in a background thread
    std::thread::spawn(move || {
        if let Err(e) = watch_and_reindex(&root, &db_path, debounce_ms) {
            tracing::error!(error = %e, "Watch error");
        }
    });

    // Run MCP server on the tokio runtime
    crate::mcp::serve_stdio().await
}
