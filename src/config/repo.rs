use std::path::Path;

use super::RepoConfig;

/// Load repository configuration from ariadne.toml at the given root path.
///
/// If the configuration file does not exist, returns a default configuration.
/// If the file exists but cannot be parsed, returns an error.
pub fn load(root: &Path) -> anyhow::Result<RepoConfig> {
    let config_path = root.join("ariadne.toml");

    if !config_path.exists() {
        return Ok(RepoConfig::default());
    }

    let contents = std::fs::read_to_string(&config_path)?;
    let config: RepoConfig = toml::from_str(&contents)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn load_returns_default_when_file_missing() {
        let path = PathBuf::from("/nonexistent/path/that/should/not/exist");
        let config = load(&path).expect("should return default config");
        assert!(config.languages.is_none());
        assert!(config.rules.is_empty());
    }
}
