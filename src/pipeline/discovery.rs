use crate::config::RepoConfig;
use crate::parse::types::Language;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug)]
pub struct DiscoveredFile {
    pub path: PathBuf,
    pub language: Language,
}

#[derive(Debug)]
pub struct DiscoveryResult {
    pub files: Vec<DiscoveredFile>,
    pub languages: Vec<Language>,
    pub frameworks: Vec<String>,
}

const EXCLUDED_DIRS: &[&str] = &[
    "node_modules",
    ".venv",
    "venv",
    "__pycache__",
    "vendor",
    "target",
    "build",
    "dist",
    ".git",
    ".svn",
    ".hg",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    ".next",
    ".nuxt",
    "coverage",
    ".cache",
];

/// Path-acceptance filter shared by full discovery and watch mode, so the
/// two ingestion paths cannot disagree about what belongs in the index.
///
/// Four layers, all of which must pass for a file to be indexable:
/// 1. no path component is on the built-in EXCLUDED_DIRS list,
/// 2. no path component exactly equals a user exclude pattern (the
///    historical `exclude = ["vendor"]` form),
/// 3. the path relative to the root does not match any user glob pattern
///    (the documented `exclude = ["vendor/**", "generated/**"]` form),
/// 4. the path is not ignored by the repo's .gitignore files (root or
///    nested). Only in-repo .gitignore files are honored — never the user's
///    global gitignore or .git/info/exclude, which vary by machine and would
///    make two indexes of the same tree disagree. A .git directory is NOT
///    required (rsync'd repo copies carry .gitignore but no .git).
pub struct PathFilter {
    root: PathBuf,
    extra_names: Vec<String>,
    globs: Option<globset::GlobSet>,
    /// (base dir, matcher) pairs, deepest base first so the most specific
    /// .gitignore is consulted first — the first definitive verdict wins.
    gitignores: Vec<(PathBuf, ignore::gitignore::Gitignore)>,
}

/// Collect every .gitignore in the tree (pruning built-in excluded dirs) and
/// compile each with its containing directory as the match base. A .gitignore
/// that is itself inside a gitignored directory is skipped, so a vendored or
/// worktree-copied repo cannot re-include its own files.
fn collect_gitignores(root: &Path) -> Vec<(PathBuf, ignore::gitignore::Gitignore)> {
    let mut gitignore_paths: Vec<PathBuf> = WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            e.depth() == 0 || !EXCLUDED_DIRS.contains(&name.as_ref())
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.file_name() == ".gitignore")
        .map(|e| e.path().to_path_buf())
        .collect();
    // Shallowest first so parent matchers exist before nested ones are vetted.
    gitignore_paths.sort_by_key(|p| (p.components().count(), p.clone()));

    let mut matchers: Vec<(PathBuf, ignore::gitignore::Gitignore)> = Vec::new();
    for gi_path in gitignore_paths {
        let base = match gi_path.parent() {
            Some(b) => b.to_path_buf(),
            None => continue,
        };
        // Skip .gitignore files living under an already-ignored directory.
        let ignored_by_parents = matchers.iter().any(|(mbase, m)| {
            gi_path
                .strip_prefix(mbase)
                .ok()
                .map(|rel| m.matched_path_or_any_parents(rel, false).is_ignore())
                .unwrap_or(false)
        });
        if ignored_by_parents {
            continue;
        }
        let (matcher, err) = ignore::gitignore::Gitignore::new(&gi_path);
        if let Some(err) = err {
            tracing::warn!(path = %gi_path.display(), error = %err, "partially invalid .gitignore");
        }
        matchers.push((base, matcher));
    }
    // Deepest base first for lookups: most specific .gitignore wins.
    matchers.sort_by(|a, b| {
        b.0.components()
            .count()
            .cmp(&a.0.components().count())
            .then_with(|| a.0.cmp(&b.0))
    });
    matchers
}

impl PathFilter {
    pub fn new(root: &Path, config: &RepoConfig) -> Self {
        let patterns: Vec<String> = config.exclude.as_ref().cloned().unwrap_or_default();

        let mut builder = globset::GlobSetBuilder::new();
        let mut has_glob = false;
        for pat in &patterns {
            if pat.contains(['*', '?', '[', '/']) {
                if let Ok(glob) = globset::Glob::new(pat) {
                    builder.add(glob);
                    has_glob = true;
                } else {
                    tracing::warn!(pattern = %pat, "Invalid exclude glob pattern; ignored");
                }
            }
        }
        let globs = if has_glob { builder.build().ok() } else { None };

        Self {
            root: root.to_path_buf(),
            extra_names: patterns,
            globs,
            gitignores: collect_gitignores(root),
        }
    }

    /// True when the path is excluded by a .gitignore. Matchers are consulted
    /// deepest-first; the first definitive verdict (ignore or whitelist) wins.
    pub fn is_gitignored(&self, path: &Path, is_dir: bool) -> bool {
        let abs;
        let path = if path.is_absolute() {
            path
        } else {
            abs = self.root.join(path);
            &abs
        };
        for (base, matcher) in &self.gitignores {
            if let Ok(rel) = path.strip_prefix(base) {
                match matcher.matched_path_or_any_parents(rel, is_dir) {
                    ignore::Match::Ignore(_) => return true,
                    ignore::Match::Whitelist(_) => return false,
                    ignore::Match::None => {}
                }
            }
        }
        false
    }

    /// True when a single path component (directory or file name) is
    /// excluded outright. Used by discovery to prune whole subtrees.
    pub fn is_excluded_component(&self, name: &str) -> bool {
        EXCLUDED_DIRS.contains(&name) || self.extra_names.iter().any(|pat| pat.as_str() == name)
    }

    /// Full acceptance test for one file path (absolute or root-relative).
    /// Returns the detected language when the file should be indexed.
    pub fn indexable_language(&self, path: &Path) -> Option<Language> {
        let ext = path.extension().and_then(|e| e.to_str())?;
        let lang = Language::from_extension(ext)?;

        let relative = path.strip_prefix(&self.root).unwrap_or(path);
        for component in relative.components() {
            let name = component.as_os_str().to_string_lossy();
            if self.is_excluded_component(&name) {
                return None;
            }
        }
        if let Some(globs) = &self.globs {
            if globs.is_match(relative) {
                return None;
            }
        }
        if self.is_gitignored(path, false) {
            return None;
        }
        Some(lang)
    }
}

/// Detect frameworks from config-file markers plus package manifests.
/// Shared by full discovery and watch-mode post-reindex resolution so
/// entry-point rules stay identical between the two paths.
pub fn detect_frameworks(root: &Path) -> Vec<String> {
    let mut frameworks = crate::config::autodetect::detect(root).frameworks;
    for f in super::framework_entry_points::detect_frameworks_from_manifest(root) {
        if !frameworks.contains(&f) {
            frameworks.push(f);
        }
    }
    frameworks
}

/// Phase 0 (untimed, pre-transaction): Discover source files in the repository.
///
/// Filters directories using the hardcoded EXCLUDED_DIRS list and any
/// additional exclude patterns specified in the RepoConfig.
pub fn discover(root: &Path, config: &RepoConfig) -> anyhow::Result<DiscoveryResult> {
    let mut files = Vec::new();
    let mut languages = HashSet::new();

    let filter = PathFilter::new(root, config);

    for entry in WalkDir::new(root).into_iter().filter_entry(|e| {
        // Name-based + gitignore pruning — glob patterns are checked per file
        // below, where the root-relative path is available. Gitignored
        // directories are pruned here so huge ignored trees (worktree copies,
        // build output) are never descended into.
        let name = e.file_name().to_string_lossy();
        if filter.is_excluded_component(&name) {
            return false;
        }
        e.depth() == 0 || !filter.is_gitignored(e.path(), e.file_type().is_dir())
    }) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(lang) = filter.indexable_language(entry.path()) {
                languages.insert(lang);
                files.push(DiscoveredFile {
                    path: entry.path().to_path_buf(),
                    language: lang,
                });
            }
        }
    }

    // Order languages (file count desc, name asc): .first() is persisted as
    // the service's primary_language, so set iteration order must never
    // decide it — the dominant language should, deterministically.
    let mut lang_counts: HashMap<Language, usize> = HashMap::new();
    for file in &files {
        *lang_counts.entry(file.language).or_insert(0) += 1;
    }
    let mut ordered: Vec<Language> = languages.into_iter().collect();
    ordered.sort_unstable_by(|a, b| {
        lang_counts[b]
            .cmp(&lang_counts[a])
            .then_with(|| a.display_name().cmp(b.display_name()))
    });

    Ok(DiscoveryResult {
        files,
        languages: ordered,
        frameworks: detect_frameworks(root),
    })
}
