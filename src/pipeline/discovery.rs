use crate::config::RepoConfig;
use crate::parse::types::Language;
use std::collections::HashSet;
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
/// Three layers, all of which must pass for a file to be indexable:
/// 1. no path component is on the built-in EXCLUDED_DIRS list,
/// 2. no path component exactly equals a user exclude pattern (the
///    historical `exclude = ["vendor"]` form),
/// 3. the path relative to the root does not match any user glob pattern
///    (the documented `exclude = ["vendor/**", "generated/**"]` form).
pub struct PathFilter {
    root: PathBuf,
    extra_names: Vec<String>,
    globs: Option<globset::GlobSet>,
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
        }
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
        // Name-based pruning only — glob patterns are checked per file below,
        // where the root-relative path is available.
        let name = e.file_name().to_string_lossy();
        !filter.is_excluded_component(&name)
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

    Ok(DiscoveryResult {
        files,
        languages: languages.into_iter().collect(),
        frameworks: detect_frameworks(root),
    })
}
