pub mod debounce;
pub mod incremental;

use notify::{Watcher, RecursiveMode, Event, EventKind};
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

    println!("Watching {} for changes...", root.display());

    loop {
        let events = watcher.poll_events(debounce);
        if !events.is_empty() {
            let changed_files: Vec<_> = events
                .iter()
                .flat_map(|e| e.paths.iter())
                .collect();

            let indexable: Vec<&Path> = changed_files
                .iter()
                .filter(|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .map(|ext| crate::parse::types::Language::from_extension(ext).is_some())
                        .unwrap_or(false)
                })
                .map(|p| p.as_path())
                .collect();

            if !indexable.is_empty() {
                println!("Re-indexing {} changed files...", indexable.len());
                if let Err(e) = incremental::reindex_files(&db, root, &indexable) {
                    eprintln!("Re-index error: {}", e);
                }
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
    println!("Starting watch mode with MCP server...");

    // Spawn the file watcher in a background thread
    std::thread::spawn(move || {
        if let Err(e) = watch_and_reindex(&root, &db_path, debounce_ms) {
            eprintln!("Watch error: {}", e);
        }
    });

    // Run MCP server on the tokio runtime
    crate::mcp::serve_stdio().await
}
