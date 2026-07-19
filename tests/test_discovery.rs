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

// ---------------------------------------------------------------------------
// .gitignore honoring — root, nested, negation, watch parity
// (No .git dir is created in these fixtures on purpose: rsync'd repo copies
// have a .gitignore but no .git, and discovery must still honor it.)
// ---------------------------------------------------------------------------

#[test]
fn test_discover_honors_root_gitignore() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join(".claude/worktrees/copy/src")).unwrap();
    fs::write(root.join(".gitignore"), ".claude/worktrees/\n").unwrap();
    fs::write(root.join("src/app.ts"), "export function app() {}\n").unwrap();
    fs::write(
        root.join(".claude/worktrees/copy/src/app.ts"),
        "export function app() {}\n",
    )
    .unwrap();

    let result = discover(root, &RepoConfig::default()).expect("discover");
    let paths: Vec<String> = result
        .files
        .iter()
        .map(|f| f.path.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        result.files.len(),
        1,
        "gitignored worktree copies must not be discovered, got {paths:?}"
    );
}

#[test]
fn test_discover_honors_nested_gitignore() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    fs::create_dir_all(root.join("src/gen")).unwrap();
    fs::write(root.join("src/gen/.gitignore"), "local.ts\n").unwrap();
    fs::write(root.join("src/gen/local.ts"), "export const x = 1;\n").unwrap();
    fs::write(root.join("src/gen/kept.ts"), "export const y = 2;\n").unwrap();

    let result = discover(root, &RepoConfig::default()).expect("discover");
    let names: Vec<String> = result
        .files
        .iter()
        .filter_map(|f| f.path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    assert!(
        !names.contains(&"local.ts".to_string()),
        "nested .gitignore must exclude local.ts, got {names:?}"
    );
    assert!(names.contains(&"kept.ts".to_string()));
}

#[test]
fn test_discover_gitignore_negation_reincludes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    fs::create_dir_all(root.join("gen")).unwrap();
    fs::write(root.join(".gitignore"), "gen/*.py\n!gen/keep.py\n").unwrap();
    fs::write(root.join("gen/skip.py"), "def skip():\n    pass\n").unwrap();
    fs::write(root.join("gen/keep.py"), "def keep():\n    pass\n").unwrap();

    let result = discover(root, &RepoConfig::default()).expect("discover");
    let names: Vec<String> = result
        .files
        .iter()
        .filter_map(|f| f.path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    assert!(
        names.contains(&"keep.py".to_string()),
        "negated pattern must re-include keep.py, got {names:?}"
    );
    assert!(
        !names.contains(&"skip.py".to_string()),
        "gen/*.py must exclude skip.py, got {names:?}"
    );
}

#[test]
fn test_path_filter_gitignore_watch_parity() {
    // Watch mode admits files through the same PathFilter — a gitignored file
    // must be rejected by indexable_language, not just by full discovery.
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    fs::create_dir_all(root.join("scratch")).unwrap();
    fs::write(root.join(".gitignore"), "scratch/\n").unwrap();
    fs::write(root.join("scratch/tmp.ts"), "export const t = 1;\n").unwrap();
    fs::write(root.join("main.ts"), "export const m = 1;\n").unwrap();

    let filter = PathFilter::new(root, &RepoConfig::default());
    assert!(
        filter
            .indexable_language(&root.join("scratch/tmp.ts"))
            .is_none(),
        "watch path must reject gitignored files"
    );
    assert!(filter.indexable_language(&root.join("main.ts")).is_some());
}

/// languages must be ordered (file count desc, name asc): the first entry is
/// persisted as the service's primary_language, so set iteration order must
/// never decide it.
#[test]
fn test_discover_languages_deterministic_dominant_first() {
    use ariadne::parse::types::Language;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("src")).unwrap();

    // Python dominates (3 files); seven other languages with 1 file each.
    for name in ["a", "b", "c"] {
        fs::write(root.join(format!("src/{name}.py")), "def f():\n    pass\n").unwrap();
    }
    for (file, content) in [
        ("one.js", "function f() {}\n"),
        ("two.ts", "export function f() {}\n"),
        ("three.go", "package main\n"),
        ("four.java", "class Four {}\n"),
        ("five.rs", "fn f() {}\n"),
        ("six.rb", "def f; end\n"),
        ("seven.php", "<?php function f() {}\n"),
    ] {
        fs::write(root.join(format!("src/{file}")), content).unwrap();
    }

    let expected = vec![
        Language::Python, // 3 files — dominant
        Language::Go,     // the rest: 1 file each, name asc
        Language::Java,
        Language::JavaScript,
        Language::Php,
        Language::Ruby,
        Language::Rust,
        Language::TypeScript,
    ];

    for run in 1..=4 {
        let result = discover(root, &RepoConfig::default()).expect("discover");
        assert_eq!(
            result.languages, expected,
            "languages must be (count desc, name asc) on run {run}"
        );
    }
}
