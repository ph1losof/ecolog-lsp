use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Deserialize)]
pub struct EcologConfig {
    #[serde(default)]
    pub features: FeatureConfig,
    #[serde(default)]
    pub strict: StrictConfig,
    #[serde(default)]
    pub workspace: abundantis::config::WorkspaceConfig,
    #[serde(default)]
    pub resolution: abundantis::config::ResolutionConfig,
    #[serde(default)]
    pub interpolation: abundantis::config::InterpolationConfig,
    #[serde(default)]
    pub cache: abundantis::config::CacheConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeatureConfig {
    #[serde(default = "true_bool")]
    pub hover: bool,
    #[serde(default = "true_bool")]
    pub completion: bool,
    #[serde(default = "true_bool")]
    pub diagnostics: bool,
    #[serde(default = "true_bool")]
    pub definition: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StrictConfig {
    #[serde(default = "true_bool")]
    pub hover: bool,
    #[serde(default = "true_bool")]
    pub completion: bool,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            hover: true,
            completion: true,
            diagnostics: true,
            definition: true,
        }
    }
}

impl Default for StrictConfig {
    fn default() -> Self {
        Self {
            hover: true,
            completion: true,
        }
    }
}

impl Default for EcologConfig {
    fn default() -> Self {
        Self {
            features: FeatureConfig::default(),
            strict: StrictConfig::default(),
            workspace: abundantis::config::WorkspaceConfig::default(),
            resolution: abundantis::config::ResolutionConfig::default(),
            interpolation: abundantis::config::InterpolationConfig::default(),
            cache: abundantis::config::CacheConfig::default(),
        }
    }
}

impl EcologConfig {
    pub fn to_abundantis_config(&self) -> abundantis::config::AbundantisConfig {
        abundantis::config::AbundantisConfig {
            workspace: self.workspace.clone(),
            resolution: self.resolution.clone(),
            interpolation: self.interpolation.clone(),
            cache: self.cache.clone(),
        }
    }
}

pub struct ConfigManager {
    config: Arc<RwLock<EcologConfig>>,
}

impl ConfigManager {
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(EcologConfig::default())),
        }
    }

    pub fn get_config(&self) -> Arc<RwLock<EcologConfig>> {
        self.config.clone()
    }

    pub async fn load_from_workspace(&self, root: &Path) -> Result<EcologConfig, String> {
        let config_path = root.join("ecolog.toml");
        let new_config = if config_path.exists() {
            fs::read_to_string(&config_path)
                .map_err(|e| format!("Failed to read config: {}", e))
                .and_then(|content| {
                    toml::from_str::<EcologConfig>(&content)
                        .map_err(|e| format!("Failed to parse config: {}", e))
                })?
        } else {
            EcologConfig::default()
        };

        let mut lock = self.config.write().await;
        *lock = new_config.clone();

        Ok(new_config)
    }

    pub async fn update(&self, new_config: EcologConfig) {
        let mut lock = self.config.write().await;
        *lock = new_config;
    }
}

fn true_bool() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_feature_config_default() {
        let config = FeatureConfig::default();
        assert!(config.hover);
        assert!(config.completion);
        assert!(config.diagnostics);
        assert!(config.definition);
    }

    #[test]
    fn test_strict_config_default() {
        let config = StrictConfig::default();
        assert!(config.hover);
        assert!(config.completion);
    }

    #[test]
    fn test_ecolog_config_default() {
        let config = EcologConfig::default();
        assert!(config.features.hover);
        assert!(config.features.completion);
        assert!(config.features.diagnostics);
        assert!(config.features.definition);
        assert!(config.strict.hover);
        assert!(config.strict.completion);
    }

    #[test]
    fn test_ecolog_config_to_abundantis() {
        let config = EcologConfig::default();
        let abundantis_config = config.to_abundantis_config();
        // Just verify it doesn't panic and returns valid config
        assert!(abundantis_config.interpolation.enabled);
    }

    #[test]
    fn test_config_manager_new() {
        let manager = ConfigManager::new();
        // Just verify it creates successfully
        let _config = manager.get_config();
    }

    #[tokio::test]
    async fn test_config_manager_load_missing_file() {
        let manager = ConfigManager::new();
        let temp_dir = TempDir::new().unwrap();

        // No config file = use defaults
        let result = manager.load_from_workspace(temp_dir.path()).await;
        assert!(result.is_ok());

        let config = result.unwrap();
        assert!(config.features.hover);
    }

    #[tokio::test]
    async fn test_config_manager_load_valid_file() {
        let manager = ConfigManager::new();
        let temp_dir = TempDir::new().unwrap();

        let config_content = r#"
[features]
hover = false
completion = true
diagnostics = true
definition = false

[strict]
hover = false
completion = false
"#;

        let config_path = temp_dir.path().join("ecolog.toml");
        let mut file = std::fs::File::create(&config_path).unwrap();
        file.write_all(config_content.as_bytes()).unwrap();

        let result = manager.load_from_workspace(temp_dir.path()).await;
        assert!(result.is_ok());

        let config = result.unwrap();
        assert!(!config.features.hover);
        assert!(config.features.completion);
        assert!(config.features.diagnostics);
        assert!(!config.features.definition);
        assert!(!config.strict.hover);
        assert!(!config.strict.completion);
    }

    #[tokio::test]
    async fn test_config_manager_load_invalid_file() {
        let manager = ConfigManager::new();
        let temp_dir = TempDir::new().unwrap();

        let config_path = temp_dir.path().join("ecolog.toml");
        let mut file = std::fs::File::create(&config_path).unwrap();
        file.write_all(b"invalid toml content {{{").unwrap();

        let result = manager.load_from_workspace(temp_dir.path()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse config"));
    }

    #[tokio::test]
    async fn test_config_manager_update() {
        let manager = ConfigManager::new();

        let new_config = EcologConfig {
            features: FeatureConfig {
                hover: false,
                ..FeatureConfig::default()
            },
            ..EcologConfig::default()
        };

        manager.update(new_config).await;

        let config = manager.get_config();
        let lock = config.read().await;
        assert!(!lock.features.hover);
    }
}
