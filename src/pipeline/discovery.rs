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

/// Phase 1: Discover source files in the repository.
///
/// Filters directories using the hardcoded EXCLUDED_DIRS list and any
/// additional exclude patterns specified in the RepoConfig.
pub fn discover(root: &Path, config: &RepoConfig) -> anyhow::Result<DiscoveryResult> {
    let mut files = Vec::new();
    let mut languages = HashSet::new();

    // Merge hardcoded exclusions with user-configured exclude patterns
    let extra_excludes: Vec<String> = config.exclude.as_ref().cloned().unwrap_or_default();

    for entry in WalkDir::new(root).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        if EXCLUDED_DIRS.contains(&name.as_ref()) {
            return false;
        }
        // Check user-configured exclude patterns against the directory/file name
        if extra_excludes
            .iter()
            .any(|pat| name.as_ref() == pat.as_str())
        {
            return false;
        }
        true
    }) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                if let Some(lang) = Language::from_extension(ext) {
                    languages.insert(lang);
                    files.push(DiscoveredFile {
                        path: entry.path().to_path_buf(),
                        language: lang,
                    });
                }
            }
        }
    }

    Ok(DiscoveryResult {
        files,
        languages: languages.into_iter().collect(),
    })
}
