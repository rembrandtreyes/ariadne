use std::collections::HashMap;
use std::path::Path;

use walkdir::WalkDir;

/// Result of auto-detecting a project's configuration from its file structure.
#[derive(Debug, Clone, Default)]
pub struct DetectedConfig {
    pub detected_languages: Vec<String>,
    pub excluded_paths: Vec<String>,
    pub frameworks: Vec<String>,
}

/// Directories that should always be excluded from analysis.
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
    ".hg",
    ".svn",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    ".next",
    ".nuxt",
    "coverage",
    ".cache",
];

/// Map file extensions to language names.
fn extension_to_language(ext: &str) -> Option<&'static str> {
    match ext {
        "py" => Some("python"),
        "js" => Some("javascript"),
        "jsx" => Some("javascript"),
        "ts" => Some("typescript"),
        "tsx" => Some("typescript"),
        "go" => Some("go"),
        "java" => Some("java"),
        "rs" => Some("rust"),
        "cs" => Some("csharp"),
        "rb" => Some("ruby"),
        "php" => Some("php"),
        _ => None,
    }
}

/// Detect frameworks from the presence of known configuration files.
fn detect_frameworks(root: &Path) -> Vec<String> {
    let mut frameworks = Vec::new();

    let framework_markers: &[(&str, &str)] = &[
        ("package.json", "node"),
        ("requirements.txt", "pip"),
        ("Pipfile", "pipenv"),
        ("pyproject.toml", "python-project"),
        ("go.mod", "go-modules"),
        ("Cargo.toml", "cargo"),
        ("pom.xml", "maven"),
        ("build.gradle", "gradle"),
        ("Gemfile", "bundler"),
        ("composer.json", "composer"),
        ("next.config.js", "nextjs"),
        ("next.config.ts", "nextjs"),
        ("nuxt.config.js", "nuxt"),
        ("nuxt.config.ts", "nuxt"),
        ("angular.json", "angular"),
        ("vue.config.js", "vue"),
        ("svelte.config.js", "svelte"),
        ("tailwind.config.js", "tailwind"),
        ("tailwind.config.ts", "tailwind"),
        ("tsconfig.json", "typescript-project"),
        ("Dockerfile", "docker"),
        ("docker-compose.yml", "docker-compose"),
        ("docker-compose.yaml", "docker-compose"),
    ];

    for (marker_file, framework_name) in framework_markers {
        if root.join(marker_file).exists() {
            let name = (*framework_name).to_string();
            if !frameworks.contains(&name) {
                frameworks.push(name);
            }
        }
    }

    frameworks
}

/// Auto-detect project configuration by scanning the directory tree.
///
/// Walks the directory, counts file extensions, identifies languages,
/// and detects frameworks from configuration file markers.
pub fn detect(root: &Path) -> DetectedConfig {
    let mut extension_counts: HashMap<String, usize> = HashMap::new();
    let mut found_excluded: Vec<String> = Vec::new();

    let walker = WalkDir::new(root).follow_links(false).into_iter();

    for entry in walker.flatten() {
        let path = entry.path();

        // Check if this path contains an excluded directory
        let path_str = path.to_string_lossy();
        for excluded in EXCLUDED_DIRS {
            if path_str.contains(excluded) && !found_excluded.contains(&excluded.to_string()) {
                found_excluded.push(excluded.to_string());
            }
        }

        // Only count files, not directories
        if !entry.file_type().is_file() {
            continue;
        }

        // Skip files inside excluded directories
        let is_excluded = EXCLUDED_DIRS
            .iter()
            .any(|dir| path_str.contains(&format!("/{dir}/")));
        if is_excluded {
            continue;
        }

        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            *extension_counts.entry(ext.to_lowercase()).or_insert(0) += 1;
        }
    }

    // Convert extensions to language names, sorted by file count descending
    let mut lang_counts: HashMap<&str, usize> = HashMap::new();
    for (ext, count) in &extension_counts {
        if let Some(lang) = extension_to_language(ext) {
            *lang_counts.entry(lang).or_insert(0) += count;
        }
    }

    let mut detected_languages: Vec<(&str, usize)> = lang_counts.into_iter().collect();
    detected_languages.sort_by(|a, b| b.1.cmp(&a.1));

    let detected_languages: Vec<String> = detected_languages
        .into_iter()
        .map(|(lang, _)| lang.to_string())
        .collect();

    let frameworks = detect_frameworks(root);

    DetectedConfig {
        detected_languages,
        excluded_paths: found_excluded,
        frameworks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_mapping_covers_all_languages() {
        assert_eq!(extension_to_language("py"), Some("python"));
        assert_eq!(extension_to_language("js"), Some("javascript"));
        assert_eq!(extension_to_language("jsx"), Some("javascript"));
        assert_eq!(extension_to_language("ts"), Some("typescript"));
        assert_eq!(extension_to_language("tsx"), Some("typescript"));
        assert_eq!(extension_to_language("go"), Some("go"));
        assert_eq!(extension_to_language("java"), Some("java"));
        assert_eq!(extension_to_language("rs"), Some("rust"));
        assert_eq!(extension_to_language("cs"), Some("csharp"));
        assert_eq!(extension_to_language("rb"), Some("ruby"));
        assert_eq!(extension_to_language("php"), Some("php"));
        assert_eq!(extension_to_language("txt"), None);
    }

    #[test]
    fn detect_returns_empty_for_nonexistent_dir() {
        let config = detect(Path::new("/nonexistent/path/that/should/not/exist"));
        assert!(config.detected_languages.is_empty());
    }
}
