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
