use serde::Deserialize;
use std::path::Path;
use std::fs;
use tokio::sync::{Mutex, RwLock};
use std::sync::Arc;
use shelter::Masker;

#[derive(Debug, Clone, Deserialize)]
pub struct EcologConfig {
    #[serde(default)]
    pub features: FeatureConfig,
    #[serde(default)]
    pub strict: StrictConfig,
    #[serde(default)]
    pub masking: UnifiedMaskingConfig,
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

#[derive(Debug, Clone, Deserialize)]
pub struct UnifiedMaskingConfig {
    #[serde(default = "false_bool")]
    pub hover: bool,
    #[serde(default = "false_bool")]
    pub completion: bool,
    #[serde(flatten)]
    pub shelter: shelter::MaskingConfig,
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

impl Default for UnifiedMaskingConfig {
    fn default() -> Self {
        Self {
            hover: false,
            completion: false,
            shelter: shelter::MaskingConfig::default(),
        }
    }
}

impl UnifiedMaskingConfig {
    pub fn to_shelter_config(&self) -> shelter::MaskingConfig {
        self.shelter.clone()
    }

    pub fn should_mask_hover(&self) -> bool {
        self.hover
    }

    pub fn should_mask_completion(&self) -> bool {
        self.completion
    }
}

impl Default for EcologConfig {
    fn default() -> Self {
        Self {
            features: FeatureConfig::default(),
            strict: StrictConfig::default(),
            masking: UnifiedMaskingConfig::default(),
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
    masker: Option<Arc<Mutex<Masker>>>,
}

impl ConfigManager {
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(EcologConfig::default())),
            masker: None,
        }
    }

    pub fn set_masker(&mut self, masker: Arc<Mutex<Masker>>) {
        self.masker = Some(masker);
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

        if let Some(ref masker_arc) = self.masker {
            let mut masker = masker_arc.lock().await;
            let shelter_config = new_config.masking.to_shelter_config();
            *masker = Masker::new(shelter_config);
            tracing::info!("Updated masker with new configuration");
        }

        Ok(new_config)
    }

    pub async fn update(&self, new_config: EcologConfig) {
        let mut lock = self.config.write().await;
        *lock = new_config;
    }
}

fn true_bool() -> bool { true }
fn false_bool() -> bool { false }
