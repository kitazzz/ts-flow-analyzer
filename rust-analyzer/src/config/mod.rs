use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct AnalyzerConfig {
    pub decision_table: DecisionTableConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DecisionTableConfig {
    pub success_when_true: Vec<String>,
    pub failure_when_false: Vec<String>,
    pub failure_when_present: Vec<String>,
}

impl Default for DecisionTableConfig {
    fn default() -> Self {
        Self {
            success_when_true: vec![
                "ok".to_string(),
                "allowed".to_string(),
                "success".to_string(),
            ],
            failure_when_false: vec![
                "ok".to_string(),
                "allowed".to_string(),
                "success".to_string(),
            ],
            failure_when_present: vec!["error".to_string(), "reason".to_string()],
        }
    }
}

pub fn load_config(config_path: Option<&str>) -> Result<AnalyzerConfig, String> {
    let Some(path) = resolve_config_path(config_path) else {
        return Ok(AnalyzerConfig::default());
    };

    let raw = fs::read_to_string(&path)
        .map_err(|err| format!("Error reading config {}: {}", path.display(), err))?;
    serde_yaml::from_str(&raw)
        .map_err(|err| format!("Error parsing config {}: {}", path.display(), err))
}

fn resolve_config_path(config_path: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = config_path {
        return Some(PathBuf::from(path));
    }

    let default = Path::new("config.yaml");
    default.exists().then(|| default.to_path_buf())
}
