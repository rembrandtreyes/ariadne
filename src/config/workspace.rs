use std::path::Path;

use super::WorkspaceConfig;

/// Load workspace configuration from ariadne-workspace.toml at the given root path.
///
/// If the configuration file does not exist, returns a default configuration.
/// If the file exists but cannot be parsed, returns an error.
pub fn load(root: &Path) -> anyhow::Result<WorkspaceConfig> {
    let config_path = root.join("ariadne-workspace.toml");

    if !config_path.exists() {
        return Ok(WorkspaceConfig::default());
    }

    let contents = std::fs::read_to_string(&config_path)?;
    let config: WorkspaceConfig = toml::from_str(&contents)?;
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
        assert!(config.services.is_empty());
        assert!(config.routing.is_empty());
        assert!(config.connections.is_empty());
    }
}
