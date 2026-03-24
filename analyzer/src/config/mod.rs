use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct AnalyzerConfig {
    pub decision_table: DecisionTableConfig,
    pub resolver_factories: ResolverFactoriesConfig,
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

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ResolverFactoriesConfig {
    pub presets: Vec<String>,
}

const KNOWN_PRESETS: &[&str] = &["tailor-sdk"];

impl ResolverFactoriesConfig {
    pub fn has_presets(&self) -> bool {
        !self.presets.is_empty()
    }

    /// Whether the tailor-sdk preset is active — gates transaction-body unwrap.
    pub fn has_tailor_sdk(&self) -> bool {
        self.presets.iter().any(|p| p == "tailor-sdk")
    }

    pub fn matches_import(&self, imported_name: &str, source: &str) -> bool {
        self.presets.iter().any(|preset| {
            matches!(
                preset.as_str(),
                "tailor-sdk"
                    if imported_name == "createResolver"
                        && matches!(source, "@tailor-platform/sdk" | "@tailor-platform/api")
            )
        })
    }

    /// Returns an error listing any unrecognized preset names.
    pub fn validate(&self) -> Result<(), String> {
        let unknown: Vec<&str> = self
            .presets
            .iter()
            .filter(|p| !KNOWN_PRESETS.contains(&p.as_str()))
            .map(|p| p.as_str())
            .collect();
        if unknown.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Unknown resolver_factories preset(s): {}. Known presets: {}",
                unknown.join(", "),
                KNOWN_PRESETS.join(", "),
            ))
        }
    }
}

pub fn load_config(config_path: Option<&str>) -> Result<AnalyzerConfig, String> {
    let Some(path) = resolve_config_path(config_path) else {
        return Ok(AnalyzerConfig::default());
    };

    let raw = fs::read_to_string(&path)
        .map_err(|err| format!("Error reading config {}: {}", path.display(), err))?;
    let config: AnalyzerConfig = serde_yaml::from_str(&raw)
        .map_err(|err| format!("Error parsing config {}: {}", path.display(), err))?;
    config.resolver_factories.validate()?;
    Ok(config)
}

fn resolve_config_path(config_path: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = config_path {
        return Some(PathBuf::from(path));
    }

    let preferred = Path::new("analyze.config.yaml");
    if preferred.exists() {
        return Some(preferred.to_path_buf());
    }

    let legacy = Path::new("config.yaml");
    legacy.exists().then(|| legacy.to_path_buf())
}
