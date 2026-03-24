pub mod autodetect;
pub mod repo;
pub mod workspace;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Repository-level configuration (ariadne.toml)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepoConfig {
    pub languages: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
    pub include: Option<Vec<String>>,
    pub entry_points: Option<Vec<String>>,
    #[serde(default)]
    pub rules: Vec<ArchRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchRule {
    pub name: String,
    pub description: Option<String>,
    pub from: String,
    pub to: String,
    pub severity: RuleSeverity,
    #[serde(rename = "type")]
    pub rule_type: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RuleSeverity {
    #[default]
    Error,
    Warning,
}

/// Workspace-level configuration (ariadne-workspace.toml)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceConfig {
    #[serde(default)]
    pub services: Vec<ServiceConfig>,
    #[serde(default)]
    pub routing: Vec<RoutingRule>,
    #[serde(default)]
    pub connections: Vec<ServiceConnection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub path: PathBuf,
    pub base_url: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    pub pattern: String,
    pub service: String,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConnection {
    pub from: String,
    pub to: String,
    pub protocol: Option<String>,
}
