use super::PluginInfo;
use std::path::Path;

pub struct PluginRegistry {
    plugins: Vec<PluginInfo>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    pub fn discover(&mut self) -> anyhow::Result<()> {
        for dir in super::plugin_dirs() {
            if dir.exists() {
                self.scan_directory(&dir)?;
            }
        }
        Ok(())
    }

    pub fn list(&self) -> &[PluginInfo] {
        &self.plugins
    }

    pub fn find_by_extension(&self, ext: &str) -> Option<&PluginInfo> {
        self.plugins.iter().find(|p| p.extensions.contains(&ext.to_string()))
    }

    fn scan_directory(&mut self, dir: &Path) -> anyhow::Result<()> {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("wasm") {
                    let name = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    self.plugins.push(PluginInfo {
                        name,
                        extensions: Vec::new(),
                        version: "0.0.0".to_string(),
                        path,
                    });
                }
            }
        }
        Ok(())
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
