use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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
    /// Initialization settings from client (to merge with ecolog.toml)
    init_settings: Arc<RwLock<Option<serde_json::Value>>>,
}

impl ConfigManager {
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(EcologConfig::default())),
            init_settings: Arc::new(RwLock::new(None)),
        }
    }

    pub fn get_config(&self) -> Arc<RwLock<EcologConfig>> {
        self.config.clone()
    }

    /// Store initialization settings from LSP initialize() params
    pub async fn set_init_settings(&self, settings: Option<serde_json::Value>) {
        let mut lock = self.init_settings.write().await;
        *lock = settings;
    }

    /// Load configuration with precedence merging:
    /// defaults → init_settings → ecolog.toml
    pub async fn load_from_workspace(&self, root: &Path) -> Result<EcologConfig, String> {
        // Start with defaults as JSON (so we can do proper deep merging)
        let mut config_json = serde_json::to_value(&EcologConfig::default())
            .map_err(|e| format!("Failed to serialize defaults: {}", e))?;

        // Merge initialization settings (if provided)
        {
            let init_settings = self.init_settings.read().await;
            if let Some(settings) = init_settings.as_ref() {
                merge_json(&mut config_json, settings);
            }
        }

        // Merge ecolog.toml (highest precedence)
        // Parse TOML as raw Value first to preserve which fields were actually set
        let config_path = root.join("ecolog.toml");
        if config_path.exists() {
            let toml_content = fs::read_to_string(&config_path)
                .map_err(|e| format!("Failed to read config: {}", e))?;

            // Parse TOML to toml::Value, then convert to serde_json::Value
            // This preserves only the fields that were actually specified in the file
            let toml_value: toml::Value = toml::from_str(&toml_content)
                .map_err(|e| format!("Failed to parse config: {}", e))?;
            let toml_json = toml_to_json(&toml_value);

            merge_json(&mut config_json, &toml_json);
        }

        // Convert final merged JSON back to EcologConfig
        let config: EcologConfig = serde_json::from_value(config_json)
            .map_err(|e| format!("Failed to deserialize merged config: {}", e))?;

        let mut lock = self.config.write().await;
        *lock = config.clone();

        Ok(config)
    }

    pub async fn update(&self, new_config: EcologConfig) {
        let mut lock = self.config.write().await;
        *lock = new_config;
    }

    /// Set the resolution precedence at runtime
    pub async fn set_precedence(&self, precedence: Vec<abundantis::config::SourcePrecedence>) {
        let mut lock = self.config.write().await;
        lock.resolution.precedence = precedence;
    }

    /// Get the current resolution precedence
    pub async fn get_precedence(&self) -> Vec<abundantis::config::SourcePrecedence> {
        let lock = self.config.read().await;
        lock.resolution.precedence.clone()
    }
}

/// Convert toml::Value to serde_json::Value
/// This preserves only the fields that were actually specified in the TOML
fn toml_to_json(toml: &toml::Value) -> serde_json::Value {
    match toml {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::Value::Number((*i).into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(toml_to_json).collect())
        }
        toml::Value::Table(table) => {
            let map: serde_json::Map<String, serde_json::Value> = table
                .iter()
                .map(|(k, v)| (k.clone(), toml_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
    }
}

/// Deep merge JSON values (overlay wins for non-null values)
fn merge_json(base: &mut serde_json::Value, overlay: &serde_json::Value) {
    match (base, overlay) {
        (serde_json::Value::Object(base_map), serde_json::Value::Object(overlay_map)) => {
            for (key, overlay_val) in overlay_map {
                // Skip null values in overlay (allows partial configs)
                if overlay_val.is_null() {
                    continue;
                }
                match base_map.get_mut(key) {
                    Some(base_val) => merge_json(base_val, overlay_val),
                    None => {
                        base_map.insert(key.clone(), overlay_val.clone());
                    }
                }
            }
        }
        (base, overlay) => {
            // Non-object: overlay replaces base (unless overlay is null)
            if !overlay.is_null() {
                *base = overlay.clone();
            }
        }
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

    #[tokio::test]
    async fn test_config_manager_init_settings_only() {
        let manager = ConfigManager::new();
        let temp_dir = TempDir::new().unwrap();

        // Set init settings (no ecolog.toml)
        let init_settings = serde_json::json!({
            "features": {
                "hover": false,
                "diagnostics": false
            }
        });
        manager.set_init_settings(Some(init_settings)).await;

        let result = manager.load_from_workspace(temp_dir.path()).await;
        assert!(result.is_ok());

        let config = result.unwrap();
        // Init settings should be applied
        assert!(!config.features.hover);
        assert!(!config.features.diagnostics);
        // Defaults should remain for unspecified values
        assert!(config.features.completion);
        assert!(config.features.definition);
    }

    #[tokio::test]
    async fn test_config_manager_toml_overrides_init_settings() {
        let manager = ConfigManager::new();
        let temp_dir = TempDir::new().unwrap();

        // Set init settings
        let init_settings = serde_json::json!({
            "features": {
                "hover": false,
                "diagnostics": false,
                "completion": false
            }
        });
        manager.set_init_settings(Some(init_settings)).await;

        // Create ecolog.toml that overrides some settings
        let config_content = r#"
[features]
hover = true
diagnostics = true
"#;
        let config_path = temp_dir.path().join("ecolog.toml");
        let mut file = std::fs::File::create(&config_path).unwrap();
        file.write_all(config_content.as_bytes()).unwrap();

        let result = manager.load_from_workspace(temp_dir.path()).await;
        assert!(result.is_ok());

        let config = result.unwrap();
        // ecolog.toml should override init settings
        assert!(config.features.hover); // toml overrides init_settings
        assert!(config.features.diagnostics); // toml overrides init_settings
        // Values not in toml should come from init_settings
        assert!(!config.features.completion); // from init_settings
        // definition was not in init_settings, should be default (true)
        assert!(config.features.definition);
    }

    #[tokio::test]
    async fn test_config_manager_workspace_root_from_init_settings() {
        let manager = ConfigManager::new();
        let temp_dir = TempDir::new().unwrap();

        // Set workspace root via init settings
        let init_settings = serde_json::json!({
            "workspace": {
                "root": "/custom/workspace/root"
            }
        });
        manager.set_init_settings(Some(init_settings)).await;

        let result = manager.load_from_workspace(temp_dir.path()).await;
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(
            config.workspace.root,
            Some(std::path::PathBuf::from("/custom/workspace/root"))
        );
    }
}
