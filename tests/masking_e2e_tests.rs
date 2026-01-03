// End-to-end integration tests for unified EcologConfig masking
// Tests all masking modes, pattern overrides, and source overrides

use ecolog_lsp::server::state::ServerState;
use ecolog_lsp::server::config::{EcologConfig, ConfigManager};
use ecolog_lsp::languages::LanguageRegistry;
use ecolog_lsp::analysis::DocumentManager;
use ecolog_lsp::analysis::QueryEngine;
use ecolog_lsp::server::handlers;
use shelter::Masker;
use tower_lsp::lsp_types::*;
use abundantis::Abundantis;
use tokio::sync::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

// Global atomic counter to ensure unique temp directory names
static MASKING_TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

struct MaskingTestFixture {
    pub temp_dir: std::path::PathBuf,
    pub state: ServerState,
}

impl MaskingTestFixture {
    pub async fn new_with_config(config: EcologConfig) -> Self {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let counter = MASKING_TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!("ecolog_masking_test_{}_{}", timestamp, counter));
        fs::create_dir_all(&temp_dir).unwrap();

        // Create .env files for testing source overrides
        Self::create_env_files(&temp_dir);

        // Setup Server
        let mut registry = LanguageRegistry::new();
        registry.register(Arc::new(ecolog_lsp::languages::javascript::JavaScript));

        let languages = Arc::new(registry);
        let query_engine = Arc::new(QueryEngine::new());
        let document_manager = Arc::new(DocumentManager::new(query_engine, languages.clone()));

        // Build abundantis
        let core = Arc::new(Abundantis::builder()
            .root(&temp_dir)
            .build().await
            .expect("Failed to build Abundantis"));

        // Create masker from config
        let shelter_config = config.masking.to_shelter_config();
        let masker = Arc::new(Mutex::new(Masker::new(shelter_config)));

        // Create config manager with masker
        let mut config_manager = ConfigManager::new();
        config_manager.set_masker(masker.clone());
        let config_manager = Arc::new(config_manager);

        // Apply the provided config to the manager
        config_manager.update(config).await;

        let state = ServerState::new(
            document_manager,
            languages,
            core,
            masker,
            config_manager,
        );

        Self { temp_dir, state }
    }

    fn create_env_files(temp_dir: &std::path::Path) {
        // Create .env (default)
        let env_path = temp_dir.join(".env");
        let mut env_file = OpenOptions::new().write(true).create(true).open(&env_path).unwrap();
        writeln!(env_file, "DEFAULT_VAR=default_value").unwrap();
        writeln!(env_file, "API_KEY=api_secret_12345").unwrap();
        writeln!(env_file, "DEBUG_VAR=debug_info_67890").unwrap();

        // Create .env.local (should show plain)
        let env_local_path = temp_dir.join(".env.local");
        let mut env_local = OpenOptions::new().write(true).create(true).open(&env_local_path).unwrap();
        writeln!(env_local, "LOCAL_SECRET=local_password").unwrap();
        writeln!(env_local, "API_KEY=local_api_key").unwrap();

        // Create .env.production (should be strict)
        let env_prod_path = temp_dir.join(".env.production");
        let mut env_prod = OpenOptions::new().write(true).create(true).open(&env_prod_path).unwrap();
        writeln!(env_prod, "PROD_SECRET=prod_password").unwrap();
        writeln!(env_prod, "API_KEY=prod_api_key").unwrap();
    }

    pub fn create_file(&self, name: &str, content: &str) -> Url {
        let path = self.temp_dir.join(name);
        let mut f = OpenOptions::new().write(true).create(true).open(&path).unwrap();
        write!(f, "{}", content).unwrap();
        Url::from_file_path(&path).unwrap()
    }
}

impl Drop for MaskingTestFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temp_dir);
    }
}

#[tokio::test]
async fn test_default_full_masking() {
    let mut config = EcologConfig::default();
    config.masking.hover = true; // Enable masking for hover
    let fixture = MaskingTestFixture::new_with_config(config).await;

    let uri = fixture.create_file("test.js", "process.env.DEFAULT_VAR");
    fixture.state.document_manager.open(
        uri.clone(),
        "javascript".to_string(),
        "process.env.DEFAULT_VAR".to_string(),
        0,
    ).await;

    let hover = handlers::handle_hover(HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position::new(0, 20),
        },
        work_done_progress_params: Default::default(),
    }, &fixture.state).await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(h_str.contains("******") || h_str.contains("********"),
        "Default value should be fully masked");
}

#[tokio::test]
async fn test_pattern_override_strict() {
    let mut config = EcologConfig::default();
    config.masking.hover = true; // Enable masking for hover
    config.masking.shelter.pattern_overrides.insert(
        "*_KEY".to_string(),
        "default".to_string(), // strict mode
    );

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let uri = fixture.create_file("test.js", "process.env.API_KEY");
    fixture.state.document_manager.open(
        uri.clone(),
        "javascript".to_string(),
        "process.env.API_KEY".to_string(),
        0,
    ).await;

    let hover = handlers::handle_hover(HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position::new(0, 15),
        },
        work_done_progress_params: Default::default(),
    }, &fixture.state).await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(h_str.contains("******") || h_str.contains("********"),
        "API_KEY should be fully masked per pattern override");
    assert!(!h_str.contains("api_secret"),
        "API_KEY actual value should not be visible");
}

#[tokio::test]
async fn test_pattern_override_partial() {
    let mut config = EcologConfig::default();
    config.masking.hover = true; // Enable masking for hover
    config.masking.shelter.pattern_overrides.insert(
        "DEBUG_*".to_string(),
        "partial".to_string(),
    );

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let uri = fixture.create_file("test.js", "process.env.DEBUG_VAR");
    fixture.state.document_manager.open(
        uri.clone(),
        "javascript".to_string(),
        "process.env.DEBUG_VAR".to_string(),
        0,
    ).await;

    let hover = handlers::handle_hover(HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position::new(0, 16),
        },
        work_done_progress_params: Default::default(),
    }, &fixture.state).await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // Default partial mode: start_len=3, end_len=3
    // Value "debug_info_67890" -> "deb**********890"
    assert!(h_str.contains("deb"), "DEBUG_VAR should start with 'deb'");
    assert!(h_str.contains("890"), "DEBUG_VAR should end with '890' (end_len=3)");
    assert!(!h_str.contains("debug_info"),
        "DEBUG_VAR should be partially masked");
}

#[tokio::test]
async fn test_pattern_override_plain() {
    let mut config = EcologConfig::default();
    config.masking.shelter.pattern_overrides.insert(
        "DEFAULT_*".to_string(),
        "plain".to_string(),
    );

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let uri = fixture.create_file("test.js", "process.env.DEFAULT_VAR");
    fixture.state.document_manager.open(
        uri.clone(),
        "javascript".to_string(),
        "process.env.DEFAULT_VAR".to_string(),
        0,
    ).await;

    let hover = handlers::handle_hover(HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position::new(0, 20),
        },
        work_done_progress_params: Default::default(),
    }, &fixture.state).await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(h_str.contains("default_value"),
        "DEFAULT_VAR should not be masked (plain mode)");
    assert!(!h_str.contains("******") && !h_str.contains("*****"),
        "Should not have masking characters");
}

#[tokio::test]
async fn test_source_override_plain() {
    let mut config = EcologConfig::default();
    config.masking.shelter.source_overrides.insert(
        ".env.local".to_string(),
        "plain".to_string(),
    );

    let fixture = MaskingTestFixture::new_with_config(config).await;

    // Variable from .env.local should NOT be masked
    let uri = fixture.create_file("test.js", "process.env.LOCAL_SECRET");
    fixture.state.document_manager.open(
        uri.clone(),
        "javascript".to_string(),
        "process.env.LOCAL_SECRET".to_string(),
        0,
    ).await;

    let hover = handlers::handle_hover(HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position::new(0, 20),
        },
        work_done_progress_params: Default::default(),
    }, &fixture.state).await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(h_str.contains("local_password"),
        "LOCAL_SECRET from .env.local should be visible (plain mode)");
    assert!(!h_str.contains("******") && !h_str.contains("*****"),
        "Should not be masked");
}

#[tokio::test]
async fn test_source_override_strict() {
    let mut config = EcologConfig::default();
    config.masking.hover = true; // Enable masking for hover
    config.masking.shelter.source_overrides.insert(
        ".env.production".to_string(),
        "default".to_string(), // strict mode
    );

    let fixture = MaskingTestFixture::new_with_config(config).await;

    // Variable from .env.production should be masked
    let uri = fixture.create_file("test.js", "process.env.PROD_SECRET");
    fixture.state.document_manager.open(
        uri.clone(),
        "javascript".to_string(),
        "process.env.PROD_SECRET".to_string(),
        0,
    ).await;

    let hover = handlers::handle_hover(HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position::new(0, 20),
        },
        work_done_progress_params: Default::default(),
    }, &fixture.state).await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(h_str.contains("******") || h_str.contains("********"),
        "PROD_SECRET from .env.production should be masked (strict mode)");
    assert!(!h_str.contains("prod_password"),
        "PROD_SECRET actual value should not be visible");
}

#[tokio::test]
async fn test_pattern_vs_source_override() {
    // Test that pattern override works regardless of source
    let mut config = EcologConfig::default();
    config.masking.hover = true; // Enable masking for hover
    config.masking.shelter.pattern_overrides.insert(
        "*_KEY".to_string(),
        "default".to_string(),
    );
    config.masking.shelter.source_overrides.insert(
        ".env.local".to_string(),
        "plain".to_string(),
    );

    let fixture = MaskingTestFixture::new_with_config(config).await;

    // API_KEY from .env.local should be MASKED (pattern override takes precedence)
    let uri = fixture.create_file("test.js", "process.env.API_KEY");
    fixture.state.document_manager.open(
        uri.clone(),
        "javascript".to_string(),
        "process.env.API_KEY".to_string(),
        0,
    ).await;

    let hover = handlers::handle_hover(HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position::new(0, 15),
        },
        work_done_progress_params: Default::default(),
    }, &fixture.state).await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(h_str.contains("******") || h_str.contains("********"),
        "API_KEY should be masked per pattern override");
    assert!(!h_str.contains("local_api_key"),
        "Pattern override should apply even for .env.local");
}

#[tokio::test]
async fn test_custom_partial_mode() {
    let mut config = EcologConfig::default();
    config.masking.hover = true; // Enable masking for hover
    // Define custom partial mode
    config.masking.shelter.pattern_overrides.insert(
        "CUSTOM_*".to_string(),
        "custom_partial".to_string(),
    );
    config.masking.shelter.modes.insert(
        "custom_partial".to_string(),
        shelter::config::ModeDefinition {
            type_name: "partial".to_string(),
            options: serde_json::json!({
                "start_len": 2,
                "end_len": 2,
            }),
        },
    );

    let fixture = MaskingTestFixture::new_with_config(config).await;

    // Add custom variable to .env
    let env_path = fixture.temp_dir.join(".env");
    let mut env_file = OpenOptions::new().write(true).append(true).open(&env_path).unwrap();
    writeln!(env_file, "CUSTOM_VAR=custom_value_123").unwrap();

    let uri = fixture.create_file("test.js", "process.env.CUSTOM_VAR");
    fixture.state.document_manager.open(
        uri.clone(),
        "javascript".to_string(),
        "process.env.CUSTOM_VAR".to_string(),
        0,
    ).await;

    let hover = handlers::handle_hover(HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position::new(0, 16),
        },
        work_done_progress_params: Default::default(),
    }, &fixture.state).await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // Custom partial mode: start_len=2, end_len=2
    // Value "custom_value_123" -> "cu************23"
    assert!(h_str.contains("cu"), "CUSTOM_VAR should start with 'cu'");
    assert!(h_str.contains("23"), "CUSTOM_VAR should end with '23' (end_len=2)");
    assert!(!h_str.contains("custom_value"),
        "Should be partially masked (cu...23)");
}

#[tokio::test]
async fn test_masking_disabled() {
    let mut config = EcologConfig::default();
    config.masking.hover = false;

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let uri = fixture.create_file("test.js", "process.env.DEFAULT_VAR");
    fixture.state.document_manager.open(
        uri.clone(),
        "javascript".to_string(),
        "process.env.DEFAULT_VAR".to_string(),
        0,
    ).await;

    let hover = handlers::handle_hover(HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position::new(0, 20),
        },
        work_done_progress_params: Default::default(),
    }, &fixture.state).await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(h_str.contains("default_value"),
        "DEFAULT_VAR should be visible (masking disabled)");
    assert!(!h_str.contains("******"),
        "Should not be masked when masking is disabled");
}

#[tokio::test]
async fn test_completion_masking() {
    let mut config = EcologConfig::default();
    config.masking.completion = true; // Enable masking for completion
    config.masking.shelter.pattern_overrides.insert(
        "*_KEY".to_string(),
        "default".to_string(),
    );

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let uri = fixture.create_file("test.js", "process.env.");
    fixture.state.document_manager.open(
        uri.clone(),
        "javascript".to_string(),
        "process.env.".to_string(),
        0,
    ).await;

    let completion = handlers::handle_completion(CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position::new(0, 12),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: None,
    }, &fixture.state).await;

    assert!(completion.is_some());
    let items = completion.unwrap();
    let api_key = items.iter().find(|i| i.label == "API_KEY");
    assert!(api_key.is_some(), "API_KEY should be in completions");

    let doc = format!("{:?}", api_key.unwrap().documentation);
    assert!(!doc.contains("api_secret"),
        "API_KEY value should be masked in completion");
    assert!(doc.contains("******") || doc.contains("********"),
        "Should show masked value");
}

#[tokio::test]
async fn test_mask_char_override() {
    let mut config = EcologConfig::default();
    config.masking.hover = true; // Enable masking for hover
    config.masking.shelter.mask_char = '•';
    config.masking.shelter.pattern_overrides.insert(
        "*_KEY".to_string(),
        "default".to_string(),
    );

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let uri = fixture.create_file("test.js", "process.env.API_KEY");
    fixture.state.document_manager.open(
        uri.clone(),
        "javascript".to_string(),
        "process.env.API_KEY".to_string(),
        0,
    ).await;

    let hover = handlers::handle_hover(HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position::new(0, 15),
        },
        work_done_progress_params: Default::default(),
    }, &fixture.state).await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(h_str.contains("•"), "Should use custom mask char •");
    // Note: Can't check for absence of '*' as markdown uses ** for bold
    assert!(!h_str.contains("api_secret"),
        "API_KEY actual value should not be visible");
}

#[tokio::test]
async fn test_mask_length_override() {
    let mut config = EcologConfig::default();
    config.masking.hover = true; // Enable masking for hover
    config.masking.shelter.mask_length = Some(10);
    config.masking.shelter.pattern_overrides.insert(
        "*_KEY".to_string(),
        "default".to_string(),
    );

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let uri = fixture.create_file("test.js", "process.env.API_KEY");
    fixture.state.document_manager.open(
        uri.clone(),
        "javascript".to_string(),
        "process.env.API_KEY".to_string(),
        0,
    ).await;

    let hover = handlers::handle_hover(HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position::new(0, 15),
        },
        work_done_progress_params: Default::default(),
    }, &fixture.state).await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // The masked value is between backticks after "Value":
    // Format is: **Value**: `**********`\n**Source**: `...`
    // Extract value between first `: ` and the next backtick
    if let Some(value_start) = h_str.find("Value`: `") {
        let value_part = &h_str[value_start + "Value`: `".len()..];
        if let Some(value_end) = value_part.find('`') {
            let masked_value = &value_part[..value_end];
            let mask_count = masked_value.chars().filter(|&c| c == '*' || c == '•').count();
            assert_eq!(mask_count, 10, "Masked value should be exactly 10 characters, got: {}", masked_value);
        }
    }
    assert!(!h_str.contains("api_secret"),
        "API_KEY actual value should not be visible");
}
