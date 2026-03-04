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
pub fn discover(root: &Path, _config: &RepoConfig) -> anyhow::Result<DiscoveryResult> {
    let mut files = Vec::new();
    let mut languages = HashSet::new();

    for entry in WalkDir::new(root).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        !EXCLUDED_DIRS.contains(&name.as_ref())
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
