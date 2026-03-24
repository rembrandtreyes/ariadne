#![cfg(feature = "watch")]

use std::fs;
use std::time::Duration;

use ariadne::watch::FileWatcher;

#[test]
fn test_file_watcher_creation() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let _watcher = FileWatcher::new(dir.path()).expect("should create FileWatcher");
}

#[test]
fn test_poll_empty_returns_empty() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let watcher = FileWatcher::new(dir.path()).expect("should create FileWatcher");

    let events = watcher.poll_events(Duration::from_millis(100));
    assert!(
        events.is_empty(),
        "polling a quiet directory should return no events"
    );
}

#[test]
fn test_file_watcher_detects_new_file() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let watcher = FileWatcher::new(dir.path()).expect("should create FileWatcher");

    // Give the watcher a moment to initialize
    std::thread::sleep(Duration::from_millis(200));

    // Write a new file into the watched directory
    let file_path = dir.path().join("hello.py");
    fs::write(&file_path, "def hello():\n    pass\n").expect("should write file");

    // Poll with a generous timeout to allow the OS to deliver the event
    let events = watcher.poll_events(Duration::from_secs(2));
    assert!(
        !events.is_empty(),
        "watcher should detect the new file creation"
    );

    // Verify at least one event references our file
    let has_our_file = events
        .iter()
        .any(|e| e.paths.iter().any(|p| p.ends_with("hello.py")));
    assert!(
        has_our_file,
        "at least one event should reference hello.py, got events: {:?}",
        events
    );
}
