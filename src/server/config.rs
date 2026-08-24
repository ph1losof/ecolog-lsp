use parking_lot::RwLock as ParkingRwLock;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for a single external provider
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ProviderConfig {
    /// Whether this provider is enabled
    #[serde(default)]
    pub enabled: bool,
    /// Override binary path for this provider
    #[serde(default)]
    pub binary: Option<String>,
}

/// Configuration for external providers
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProvidersConfig {
    /// Directory containing provider binaries
    #[serde(default = "default_providers_path")]
    pub path: String,
    /// Individual provider configurations (keyed by provider name)
    #[serde(flatten)]
    pub providers: std::collections::HashMap<String, ProviderConfig>,
}

fn default_providers_path() -> String {
    // Use XDG_DATA_HOME or fall back to ~/.local/share
    std::env::var("XDG_DATA_HOME")
        .map(|xdg| format!("{}/ecolog/providers", xdg))
        .unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(|home| format!("{}/.local/share/ecolog/providers", home))
                .unwrap_or_else(|_| "~/.local/share/ecolog/providers".to_string())
        })
}

impl Default for ProvidersConfig {
    fn default() -> Self {
        Self {
            path: default_providers_path(),
            providers: std::collections::HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IndexingConfig {
    #[serde(default = "default_exclude_patterns")]
    pub exclude: Vec<String>,
    #[serde(default = "default_max_files")]
    pub max_files: usize,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    /// Number of files analyzed concurrently during workspace indexing.
    ///
    /// `0` picks a value from the core count that leaves headroom for the
    /// editor. Raise it to index large repositories faster at the cost of more
    /// CPU; lower it to keep indexing in the background.
    #[serde(default = "default_indexing_parallelism")]
    pub parallelism: usize,
}

fn default_exclude_patterns() -> Vec<String> {
    vec![
        "node_modules".to_string(),
        ".git".to_string(),
        "target".to_string(),
        "dist".to_string(),
        "build".to_string(),
        ".next".to_string(),
        "__pycache__".to_string(),
        "vendor".to_string(),
        ".venv".to_string(),
        "out".to_string(),
        ".cache".to_string(),
        ".tox".to_string(),
        "coverage".to_string(),
    ]
}

fn default_max_files() -> usize {
    5000
}

fn default_max_file_size() -> u64 {
    1_048_576
}

fn default_max_depth() -> usize {
    30
}

fn default_indexing_parallelism() -> usize {
    0
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            exclude: default_exclude_patterns(),
            max_files: default_max_files(),
            max_file_size: default_max_file_size(),
            max_depth: default_max_depth(),
            parallelism: default_indexing_parallelism(),
        }
    }
}

impl IndexingConfig {
    /// Resolves the configured parallelism to a concrete worker count.
    pub fn resolved_parallelism(&self) -> usize {
        if self.parallelism > 0 {
            return self.parallelism;
        }
        // Leave roughly half the cores for the editor and the rest of the server.
        (num_cpus::get() / 2).clamp(1, 8)
    }
}

pub struct CompiledEnvPatterns {
    patterns: Vec<glob::Pattern>,
}

impl CompiledEnvPatterns {
    pub fn compile(raw: &[impl AsRef<str>]) -> Self {
        Self {
            patterns: raw
                .iter()
                .filter_map(|p| glob::Pattern::new(p.as_ref()).ok())
                .collect(),
        }
    }

    pub fn matches(&self, file_name: &str) -> bool {
        self.patterns.iter().any(|p| p.matches(file_name))
    }
}

impl Default for CompiledEnvPatterns {
    fn default() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[derive(Default)]
pub struct EcologConfig {
    #[serde(default)]
    pub features: FeatureConfig,
    #[serde(default)]
    pub strict: StrictConfig,
    #[serde(default)]
    pub masking: MaskingConfig,
    #[serde(default)]
    pub inlay_hints: InlayHintConfig,
    #[serde(default)]
    pub workspace: abundantis::config::WorkspaceConfig,
    #[serde(default)]
    pub resolution: abundantis::config::ResolutionConfig,
    #[serde(default)]
    pub interpolation: abundantis::config::InterpolationConfig,
    #[serde(default)]
    pub cache: abundantis::config::CacheConfig,
    #[serde(default)]
    pub sources: abundantis::config::SourcesConfig,
    #[serde(default)]
    pub providers: ProvidersConfig,
    #[serde(default)]
    pub indexing: IndexingConfig,
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
    #[serde(default)]
    pub inlay_hints: bool,
}

/// Controls whether resolved values are hidden when shown in the editor.
///
/// Off by default: turning it on changes what every hover and completion shows,
/// which should be a deliberate choice rather than something an upgrade does.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MaskingConfig {
    /// Master switch. When false nothing is masked.
    #[serde(default)]
    pub enabled: bool,
    /// Mask values in hover tooltips.
    #[serde(default = "true_bool")]
    pub mask_in_hover: bool,
    /// Mask values in completion item documentation.
    #[serde(default = "true_bool")]
    pub mask_in_completion: bool,
    /// Mask values in inlay hints.
    #[serde(default = "true_bool")]
    pub mask_in_inlay_hints: bool,
    /// Character the value is replaced with.
    #[serde(default = "default_mask_char")]
    pub mask_char: char,
    /// How many trailing characters stay visible, for recognising a value
    /// without revealing it. `0` hides everything.
    #[serde(default)]
    pub show_last: usize,
}

fn default_mask_char() -> char {
    '*'
}

impl Default for MaskingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mask_in_hover: true,
            mask_in_completion: true,
            mask_in_inlay_hints: true,
            mask_char: default_mask_char(),
            show_last: 0,
        }
    }
}

impl MaskingConfig {
    /// Returns `value` with its characters replaced, honouring `show_last`.
    ///
    /// The mask is a fixed width so the original length is not leaked. An empty
    /// value stays empty: revealing that a variable is set but blank is not a
    /// disclosure, and callers render it as an explicit "empty" marker.
    pub fn mask(&self, value: &str) -> String {
        const MASK_WIDTH: usize = 8;

        if value.is_empty() {
            return String::new();
        }

        let chars: Vec<char> = value.chars().collect();
        let revealed = self.show_last.min(chars.len());
        let hidden = chars.len() - revealed;

        if hidden == 0 {
            // `show_last` covers the whole value; mask it entirely rather than
            // print it verbatim.
            return std::iter::repeat(self.mask_char).take(MASK_WIDTH).collect();
        }

        let mut out: String = std::iter::repeat(self.mask_char).take(MASK_WIDTH).collect();
        out.extend(&chars[hidden..]);
        out
    }

    /// Masks `value` when masking is enabled for the given surface.
    pub fn apply(&self, value: &str, surface: MaskSurface) -> std::borrow::Cow<'_, str> {
        let on = self.enabled
            && match surface {
                MaskSurface::Hover => self.mask_in_hover,
                MaskSurface::Completion => self.mask_in_completion,
                MaskSurface::InlayHint => self.mask_in_inlay_hints,
            };

        if on {
            std::borrow::Cow::Owned(self.mask(value))
        } else {
            std::borrow::Cow::Owned(value.to_string())
        }
    }
}

/// Where a value is about to be displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskSurface {
    Hover,
    Completion,
    InlayHint,
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
            inlay_hints: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InlayHintConfig {
    #[serde(default = "true_bool")]
    pub direct_references: bool,

    #[serde(default = "true_bool")]
    pub binding_declarations: bool,

    #[serde(default)]
    pub binding_usages: bool,

    #[serde(default = "true_bool")]
    pub property_accesses: bool,

    #[serde(default = "default_max_hint_length")]
    pub max_value_length: usize,

    #[serde(default)]
    pub max_hints_per_line: usize,
}

fn default_max_hint_length() -> usize {
    30
}

impl Default for InlayHintConfig {
    fn default() -> Self {
        Self {
            direct_references: true,
            binding_declarations: true,
            binding_usages: false,
            property_accesses: true,
            max_value_length: default_max_hint_length(),
            max_hints_per_line: 0,
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


impl EcologConfig {
    pub fn to_abundantis_config(&self) -> abundantis::config::AbundantisConfig {
        abundantis::config::AbundantisConfig {
            workspace: self.workspace.clone(),
            resolution: self.resolution.clone(),
            interpolation: self.interpolation.clone(),
            cache: self.cache.clone(),
            sources: self.sources.clone(),
        }
    }
}

/// Cached feature flags for lock-free access in hot paths.
/// These atomics are updated when config changes.
#[derive(Default)]
pub struct CachedFeatureFlags {
    pub hover: AtomicBool,
    pub completion: AtomicBool,
    pub diagnostics: AtomicBool,
    pub definition: AtomicBool,
    pub inlay_hints: AtomicBool,
}

impl CachedFeatureFlags {
    fn new() -> Self {
        Self {
            hover: AtomicBool::new(true),
            completion: AtomicBool::new(true),
            diagnostics: AtomicBool::new(true),
            definition: AtomicBool::new(true),
            inlay_hints: AtomicBool::new(false),
        }
    }

    fn update_from(&self, features: &FeatureConfig) {
        self.hover.store(features.hover, Ordering::Relaxed);
        self.completion.store(features.completion, Ordering::Relaxed);
        self.diagnostics.store(features.diagnostics, Ordering::Relaxed);
        self.definition.store(features.definition, Ordering::Relaxed);
        self.inlay_hints.store(features.inlay_hints, Ordering::Relaxed);
    }
}

pub struct ConfigManager {
    config: Arc<RwLock<EcologConfig>>,
    init_settings: Arc<RwLock<Option<serde_json::Value>>>,
    /// Cached feature flags for lock-free access.
    /// Updated whenever config is loaded or updated.
    pub cached_features: CachedFeatureFlags,
    cached_env_patterns: ParkingRwLock<CompiledEnvPatterns>,
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigManager {
    pub fn new() -> Self {
        let default_config = EcologConfig::default();
        let initial_patterns =
            CompiledEnvPatterns::compile(&default_config.workspace.env_files);
        Self {
            config: Arc::new(RwLock::new(default_config)),
            init_settings: Arc::new(RwLock::new(None)),
            cached_features: CachedFeatureFlags::new(),
            cached_env_patterns: ParkingRwLock::new(initial_patterns),
        }
    }

    #[inline]
    pub fn is_env_file(&self, file_name: &str) -> bool {
        self.cached_env_patterns.read().matches(file_name)
    }

    pub async fn refresh_env_patterns(&self) {
        let config = self.config.read().await;
        *self.cached_env_patterns.write() =
            CompiledEnvPatterns::compile(&config.workspace.env_files);
    }

    /// Check if hover feature is enabled (lock-free).
    #[inline]
    pub fn is_hover_enabled(&self) -> bool {
        self.cached_features.hover.load(Ordering::Relaxed)
    }

    /// Check if completion feature is enabled (lock-free).
    #[inline]
    pub fn is_completion_enabled(&self) -> bool {
        self.cached_features.completion.load(Ordering::Relaxed)
    }

    /// Check if diagnostics feature is enabled (lock-free).
    #[inline]
    pub fn is_diagnostics_enabled(&self) -> bool {
        self.cached_features.diagnostics.load(Ordering::Relaxed)
    }

    /// Check if definition feature is enabled (lock-free).
    #[inline]
    pub fn is_definition_enabled(&self) -> bool {
        self.cached_features.definition.load(Ordering::Relaxed)
    }

    /// Check if inlay hints feature is enabled (lock-free).
    #[inline]
    pub fn is_inlay_hints_enabled(&self) -> bool {
        self.cached_features.inlay_hints.load(Ordering::Relaxed)
    }

    /// Snapshot of the masking settings.
    ///
    /// Cloned rather than borrowed so callers do not hold the config lock while
    /// rendering.
    pub async fn masking(&self) -> MaskingConfig {
        self.get_config().read().await.masking.clone()
    }

    pub fn get_config(&self) -> Arc<RwLock<EcologConfig>> {
        self.config.clone()
    }

    pub async fn set_init_settings(&self, settings: Option<serde_json::Value>) {
        let mut lock = self.init_settings.write().await;
        *lock = settings;
    }

    pub async fn load_from_workspace(&self, root: &Path) -> Result<EcologConfig, String> {
        let mut config_json = serde_json::to_value(EcologConfig::default())
            .map_err(|e| format!("Failed to serialize defaults: {}", e))?;

        {
            let init_settings = self.init_settings.read().await;
            if let Some(settings) = init_settings.as_ref() {
                merge_json(&mut config_json, settings);
            }
        }

        let config_path = root.join("ecolog.toml");
        // Read straight away rather than checking `exists()` first: one syscall
        // instead of two, no blocking of the async runtime, and no window between
        // the check and the read.
        let existing = match tokio::fs::read_to_string(&config_path).await {
            Ok(content) => Some(content),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(format!("Failed to read config: {}", e)),
        };

        if let Some(toml_content) = existing {

            let toml_value: toml::Value = toml::from_str(&toml_content)
                .map_err(|e| format!("Failed to parse config: {}", e))?;
            let toml_json = toml_to_json(&toml_value);

            merge_json(&mut config_json, &toml_json);
        }

        let mut config: EcologConfig = serde_json::from_value(config_json)
            .map_err(|e| format!("Failed to deserialize merged config: {}", e))?;

        Self::apply_source_defaults(&mut config);

        // Update cached feature flags for lock-free access
        self.cached_features.update_from(&config.features);

        // Update cached env file patterns
        *self.cached_env_patterns.write() =
            CompiledEnvPatterns::compile(&config.workspace.env_files);

        let mut lock = self.config.write().await;
        *lock = config.clone();

        Ok(config)
    }

    fn apply_source_defaults(config: &mut EcologConfig) {
        use abundantis::config::{ResolutionConfig, SourcePrecedence};

        let old_default = vec![SourcePrecedence::Shell, SourcePrecedence::File];

        if config.resolution.precedence == old_default {
            config.resolution.precedence =
                ResolutionConfig::precedence_from_defaults(&config.sources.defaults);
        }
    }

    pub async fn update(&self, new_config: EcologConfig) {
        // Update cached feature flags for lock-free access
        self.cached_features.update_from(&new_config.features);

        // Update cached env file patterns
        *self.cached_env_patterns.write() =
            CompiledEnvPatterns::compile(&new_config.workspace.env_files);

        let mut lock = self.config.write().await;
        *lock = new_config;
    }

    pub async fn set_precedence(&self, precedence: Vec<abundantis::config::SourcePrecedence>) {
        let mut lock = self.config.write().await;
        lock.resolution.precedence = precedence;
    }

    pub async fn get_precedence(&self) -> Vec<abundantis::config::SourcePrecedence> {
        let lock = self.config.read().await;
        lock.resolution.precedence.clone()
    }

    pub async fn set_interpolation_enabled(&self, enabled: bool) {
        let mut lock = self.config.write().await;
        lock.interpolation.enabled = enabled;
    }

    pub async fn get_interpolation_enabled(&self) -> bool {
        let lock = self.config.read().await;
        lock.interpolation.enabled
    }

    pub async fn get_providers_config(&self) -> ProvidersConfig {
        let lock = self.config.read().await;
        lock.providers.clone()
    }
}

fn toml_to_json(toml: &toml::Value) -> serde_json::Value {
    match toml {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::Value::Number((*i).into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Array(arr) => serde_json::Value::Array(arr.iter().map(toml_to_json).collect()),
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

fn merge_json(base: &mut serde_json::Value, overlay: &serde_json::Value) {
    match (base, overlay) {
        (serde_json::Value::Object(base_map), serde_json::Value::Object(overlay_map)) => {
            for (key, overlay_val) in overlay_map {
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

        assert!(abundantis_config.interpolation.enabled);
    }

    #[test]
    fn test_config_manager_new() {
        let manager = ConfigManager::new();

        let _config = manager.get_config();
    }

    #[tokio::test]
    async fn test_config_manager_load_missing_file() {
        let manager = ConfigManager::new();
        let temp_dir = TempDir::new().unwrap();

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

        assert!(!config.features.hover);
        assert!(!config.features.diagnostics);

        assert!(config.features.completion);
        assert!(config.features.definition);
    }

    #[tokio::test]
    async fn test_config_manager_toml_overrides_init_settings() {
        let manager = ConfigManager::new();
        let temp_dir = TempDir::new().unwrap();

        let init_settings = serde_json::json!({
            "features": {
                "hover": false,
                "diagnostics": false,
                "completion": false
            }
        });
        manager.set_init_settings(Some(init_settings)).await;

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

        assert!(config.features.hover);
        assert!(config.features.diagnostics);

        assert!(!config.features.completion);

        assert!(config.features.definition);
    }

    #[tokio::test]
    async fn test_config_manager_workspace_root_from_init_settings() {
        let manager = ConfigManager::new();
        let temp_dir = TempDir::new().unwrap();

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
