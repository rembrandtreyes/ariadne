use std::fs;
use std::path::Path;

use ariadne::config::RepoConfig;
use ariadne::pipeline::discovery::{discover, PathFilter};

fn config_with_excludes(patterns: &[&str]) -> RepoConfig {
    RepoConfig {
        exclude: Some(patterns.iter().map(|s| s.to_string()).collect()),
        ..RepoConfig::default()
    }
}

// ---------------------------------------------------------------------------
// PathFilter — the single acceptance gate shared by discovery and watch mode
// ---------------------------------------------------------------------------

#[test]
fn test_path_filter_accepts_ordinary_source_files() {
    let root = Path::new("/repo");
    let filter = PathFilter::new(root, &RepoConfig::default());
    assert!(
        filter
            .indexable_language(Path::new("/repo/src/main.rs"))
            .is_some(),
        "ordinary source files must be indexable"
    );
}

#[test]
fn test_path_filter_rejects_build_artifact_dirs() {
    // cargo build generates .rs files under target/ — if the watch path
    // indexes them, every build pollutes the graph with phantom symbols.
    let root = Path::new("/repo");
    let filter = PathFilter::new(root, &RepoConfig::default());
    for path in [
        "/repo/target/debug/build/foo-abc/out/gen.rs",
        "/repo/node_modules/lib/index.js",
        "/repo/.git/hooks/sample.py",
        "/repo/vendor/dep/dep.php",
    ] {
        assert!(
            filter.indexable_language(Path::new(path)).is_none(),
            "{path} must be excluded — it lives under a built-in excluded dir"
        );
    }
}

#[test]
fn test_path_filter_rejects_unsupported_extensions() {
    let root = Path::new("/repo");
    let filter = PathFilter::new(root, &RepoConfig::default());
    assert!(filter
        .indexable_language(Path::new("/repo/README.md"))
        .is_none());
    assert!(filter
        .indexable_language(Path::new("/repo/.ariadne.db"))
        .is_none());
}

#[test]
fn test_path_filter_honors_bare_name_excludes() {
    // Historical form: exclude = ["sandbox"] matches any component named
    // sandbox, exactly.
    let root = Path::new("/repo");
    let filter = PathFilter::new(root, &config_with_excludes(&["sandbox"]));
    assert!(filter
        .indexable_language(Path::new("/repo/sandbox/x.py"))
        .is_none());
    assert!(
        filter
            .indexable_language(Path::new("/repo/src/sandbox.py"))
            .is_some(),
        "a bare-name pattern matches components, not substrings of file names"
    );
}

#[test]
fn test_path_filter_honors_glob_excludes() {
    // Documented form: exclude = ["generated/**"] — glob against the
    // root-relative path. This is the contract the README promises.
    let root = Path::new("/repo");
    let filter = PathFilter::new(root, &config_with_excludes(&["generated/**", "*.gen.ts"]));
    assert!(
        filter
            .indexable_language(Path::new("/repo/generated/deep/x.ts"))
            .is_none(),
        "generated/** must exclude everything under generated/"
    );
    assert!(
        filter
            .indexable_language(Path::new("/repo/src/api.gen.ts"))
            .is_none(),
        "*.gen.ts must exclude generated files by extension pattern"
    );
    assert!(filter
        .indexable_language(Path::new("/repo/src/api.ts"))
        .is_some());
}

// ---------------------------------------------------------------------------
// discover() — end-to-end against a real directory tree
// ---------------------------------------------------------------------------

#[test]
fn test_discover_respects_glob_excludes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("generated/deep")).unwrap();
    fs::write(root.join("src/keep.py"), "def keep():\n    pass\n").unwrap();
    fs::write(
        root.join("generated/deep/skip.py"),
        "def skip():\n    pass\n",
    )
    .unwrap();

    let result = discover(root, &config_with_excludes(&["generated/**"])).expect("discover");
    let names: Vec<String> = result
        .files
        .iter()
        .filter_map(|f| f.path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();

    assert!(
        names.contains(&"keep.py".to_string()),
        "src files stay, got {names:?}"
    );
    assert!(
        !names.contains(&"skip.py".to_string()),
        "glob-excluded files must not be discovered, got {names:?}"
    );
}

#[test]
fn test_discover_respects_builtin_excludes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
    fs::write(root.join("src/app.js"), "function app() {}\n").unwrap();
    fs::write(
        root.join("node_modules/pkg/index.js"),
        "function dep() {}\n",
    )
    .unwrap();

    let result = discover(root, &RepoConfig::default()).expect("discover");
    assert_eq!(
        result.files.len(),
        1,
        "only src/app.js should be discovered, got {:?}",
        result.files
    );
}
