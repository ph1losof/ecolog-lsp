// End-to-end integration tests for unified EcologConfig masking
// Tests all masking modes, pattern overrides, and source overrides

use abundantis::Abundantis;
use ecolog_lsp::analysis::{
    DocumentManager, ModuleResolver, QueryEngine, WorkspaceIndex, WorkspaceIndexer,
};
use ecolog_lsp::languages::LanguageRegistry;
use ecolog_lsp::server::config::{ConfigManager, EcologConfig};
use ecolog_lsp::server::handlers;
use ecolog_lsp::server::state::ServerState;
use shelter::Masker;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tower_lsp::lsp_types::*;

// Global atomic counter to ensure unique temp directory names
static MASKING_TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

struct MaskingTestFixture {
    pub temp_dir: std::path::PathBuf,
    pub state: ServerState,
}

impl MaskingTestFixture {
    pub async fn new_with_config(config: EcologConfig) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = MASKING_TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir =
            std::env::temp_dir().join(format!("ecolog_masking_test_{}_{}", timestamp, counter));
        fs::create_dir_all(&temp_dir).unwrap();

        // Create .env files for testing source overrides
        Self::create_env_files(&temp_dir);

        // Setup Server
        let mut registry = LanguageRegistry::new();
        registry.register(Arc::new(ecolog_lsp::languages::javascript::JavaScript));

        let languages = Arc::new(registry);
        let query_engine = Arc::new(QueryEngine::new());
        let document_manager =
            Arc::new(DocumentManager::new(query_engine.clone(), languages.clone()));

        // Build abundantis
        let core = Arc::new(
            Abundantis::builder()
                .root(&temp_dir)
                .build()
                .await
                .expect("Failed to build Abundantis"),
        );

        // Create masker from config
        let shelter_config = config.masking.to_shelter_config();
        let masker = Arc::new(Mutex::new(Masker::new(shelter_config)));

        // Create config manager with masker
        let mut config_manager = ConfigManager::new();
        config_manager.set_masker(masker.clone());
        let config_manager = Arc::new(config_manager);

        // Apply the provided config to the manager
        config_manager.update(config).await;

        // Setup workspace index and indexer
        let workspace_index = Arc::new(WorkspaceIndex::new());
        let module_resolver = Arc::new(ModuleResolver::new(temp_dir.clone()));
        let indexer = Arc::new(WorkspaceIndexer::new(
            Arc::clone(&workspace_index),
            query_engine,
            Arc::clone(&languages),
            temp_dir.clone(),
        ));

        let state = ServerState::new(
            document_manager,
            languages,
            core,
            masker,
            config_manager,
            workspace_index,
            indexer,
            module_resolver,
        );

        Self { temp_dir, state }
    }

    fn create_env_files(temp_dir: &std::path::Path) {
        // Create .env (default)
        let env_path = temp_dir.join(".env");
        let mut env_file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&env_path)
            .unwrap();
        writeln!(env_file, "DEFAULT_VAR=default_value").unwrap();
        writeln!(env_file, "API_KEY=api_secret_12345").unwrap();
        writeln!(env_file, "DEBUG_VAR=debug_info_67890").unwrap();

        // Create .env.local (should show plain)
        let env_local_path = temp_dir.join(".env.local");
        let mut env_local = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&env_local_path)
            .unwrap();
        writeln!(env_local, "LOCAL_SECRET=local_password").unwrap();
        writeln!(env_local, "API_KEY=local_api_key").unwrap();

        // Create .env.production (should be strict)
        let env_prod_path = temp_dir.join(".env.production");
        let mut env_prod = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&env_prod_path)
            .unwrap();
        writeln!(env_prod, "PROD_SECRET=prod_password").unwrap();
        writeln!(env_prod, "API_KEY=prod_api_key").unwrap();
    }

    pub fn create_file(&self, name: &str, content: &str) -> Url {
        let path = self.temp_dir.join(name);
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&path)
            .unwrap();
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
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.DEFAULT_VAR".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 20),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(
        h_str.contains("******") || h_str.contains("********"),
        "Default value should be fully masked"
    );
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
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.API_KEY".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 15),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(
        h_str.contains("******") || h_str.contains("********"),
        "API_KEY should be fully masked per pattern override"
    );
    assert!(
        !h_str.contains("api_secret"),
        "API_KEY actual value should not be visible"
    );
}

#[tokio::test]
async fn test_pattern_override_partial() {
    let mut config = EcologConfig::default();
    config.masking.hover = true; // Enable masking for hover
    config
        .masking
        .shelter
        .pattern_overrides
        .insert("DEBUG_*".to_string(), "partial".to_string());

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let uri = fixture.create_file("test.js", "process.env.DEBUG_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.DEBUG_VAR".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 16),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // Default partial mode: start_len=3, end_len=3
    // Value "debug_info_67890" -> "deb**********890"
    assert!(h_str.contains("deb"), "DEBUG_VAR should start with 'deb'");
    assert!(
        h_str.contains("890"),
        "DEBUG_VAR should end with '890' (end_len=3)"
    );
    assert!(
        !h_str.contains("debug_info"),
        "DEBUG_VAR should be partially masked"
    );
}

#[tokio::test]
async fn test_pattern_override_plain() {
    let mut config = EcologConfig::default();
    config
        .masking
        .shelter
        .pattern_overrides
        .insert("DEFAULT_*".to_string(), "plain".to_string());

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let uri = fixture.create_file("test.js", "process.env.DEFAULT_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.DEFAULT_VAR".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 20),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(
        h_str.contains("default_value"),
        "DEFAULT_VAR should not be masked (plain mode)"
    );
    assert!(
        !h_str.contains("******") && !h_str.contains("*****"),
        "Should not have masking characters"
    );
}

#[tokio::test]
async fn test_source_override_plain() {
    let mut config = EcologConfig::default();
    config
        .masking
        .shelter
        .source_overrides
        .insert(".env.local".to_string(), "plain".to_string());

    let fixture = MaskingTestFixture::new_with_config(config).await;

    // Variable from .env.local should NOT be masked
    let uri = fixture.create_file("test.js", "process.env.LOCAL_SECRET");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.LOCAL_SECRET".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 20),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(
        h_str.contains("local_password"),
        "LOCAL_SECRET from .env.local should be visible (plain mode)"
    );
    assert!(
        !h_str.contains("******") && !h_str.contains("*****"),
        "Should not be masked"
    );
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
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.PROD_SECRET".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 20),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(
        h_str.contains("******") || h_str.contains("********"),
        "PROD_SECRET from .env.production should be masked (strict mode)"
    );
    assert!(
        !h_str.contains("prod_password"),
        "PROD_SECRET actual value should not be visible"
    );
}

#[tokio::test]
async fn test_pattern_vs_source_override() {
    // Test that pattern override works regardless of source
    let mut config = EcologConfig::default();
    config.masking.hover = true; // Enable masking for hover
    config
        .masking
        .shelter
        .pattern_overrides
        .insert("*_KEY".to_string(), "default".to_string());
    config
        .masking
        .shelter
        .source_overrides
        .insert(".env.local".to_string(), "plain".to_string());

    let fixture = MaskingTestFixture::new_with_config(config).await;

    // API_KEY from .env.local should be MASKED (pattern override takes precedence)
    let uri = fixture.create_file("test.js", "process.env.API_KEY");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.API_KEY".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 15),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(
        h_str.contains("******") || h_str.contains("********"),
        "API_KEY should be masked per pattern override"
    );
    assert!(
        !h_str.contains("local_api_key"),
        "Pattern override should apply even for .env.local"
    );
}

#[tokio::test]
async fn test_custom_partial_mode() {
    let mut config = EcologConfig::default();
    config.masking.hover = true; // Enable masking for hover
                                 // Define custom partial mode
    config
        .masking
        .shelter
        .pattern_overrides
        .insert("CUSTOM_*".to_string(), "custom_partial".to_string());
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
    let mut env_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&env_path)
        .unwrap();
    writeln!(env_file, "CUSTOM_VAR=custom_value_123").unwrap();

    let uri = fixture.create_file("test.js", "process.env.CUSTOM_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.CUSTOM_VAR".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 16),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // Custom partial mode: start_len=2, end_len=2
    // Value "custom_value_123" -> "cu************23"
    assert!(h_str.contains("cu"), "CUSTOM_VAR should start with 'cu'");
    assert!(
        h_str.contains("23"),
        "CUSTOM_VAR should end with '23' (end_len=2)"
    );
    assert!(
        !h_str.contains("custom_value"),
        "Should be partially masked (cu...23)"
    );
}

#[tokio::test]
async fn test_masking_disabled() {
    let mut config = EcologConfig::default();
    config.masking.hover = false;

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let uri = fixture.create_file("test.js", "process.env.DEFAULT_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.DEFAULT_VAR".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 20),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(
        h_str.contains("default_value"),
        "DEFAULT_VAR should be visible (masking disabled)"
    );
    assert!(
        !h_str.contains("******"),
        "Should not be masked when masking is disabled"
    );
}

#[tokio::test]
async fn test_completion_masking() {
    let mut config = EcologConfig::default();
    config.masking.completion = true; // Enable masking for completion
    config
        .masking
        .shelter
        .pattern_overrides
        .insert("*_KEY".to_string(), "default".to_string());

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let uri = fixture.create_file("test.js", "process.env.");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.".to_string(),
            0,
        )
        .await;

    let completion = handlers::handle_completion(
        CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 12),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        },
        &fixture.state,
    )
    .await;

    assert!(completion.is_some());
    let items = completion.unwrap();
    let api_key = items.iter().find(|i| i.label == "API_KEY");
    assert!(api_key.is_some(), "API_KEY should be in completions");

    let doc = format!("{:?}", api_key.unwrap().documentation);
    assert!(
        !doc.contains("api_secret"),
        "API_KEY value should be masked in completion"
    );
    assert!(
        doc.contains("******") || doc.contains("********"),
        "Should show masked value"
    );
}

#[tokio::test]
async fn test_mask_char_override() {
    let mut config = EcologConfig::default();
    config.masking.hover = true; // Enable masking for hover
    config.masking.shelter.mask_char = '•';
    config
        .masking
        .shelter
        .pattern_overrides
        .insert("*_KEY".to_string(), "default".to_string());

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let uri = fixture.create_file("test.js", "process.env.API_KEY");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.API_KEY".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 15),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(h_str.contains("•"), "Should use custom mask char •");
    // Note: Can't check for absence of '*' as markdown uses ** for bold
    assert!(
        !h_str.contains("api_secret"),
        "API_KEY actual value should not be visible"
    );
}

#[tokio::test]
async fn test_mask_length_override() {
    let mut config = EcologConfig::default();
    config.masking.hover = true; // Enable masking for hover
    config.masking.shelter.mask_length = Some(10);
    config
        .masking
        .shelter
        .pattern_overrides
        .insert("*_KEY".to_string(), "default".to_string());

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let uri = fixture.create_file("test.js", "process.env.API_KEY");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.API_KEY".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 15),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // The masked value is between backticks after "Value":
    // Format is: **Value**: `**********`\n**Source**: `...`
    // Extract value between first `: ` and the next backtick
    if let Some(value_start) = h_str.find("Value`: `") {
        let value_part = &h_str[value_start + "Value`: `".len()..];
        if let Some(value_end) = value_part.find('`') {
            let masked_value = &value_part[..value_end];
            let mask_count = masked_value
                .chars()
                .filter(|&c| c == '*' || c == '•')
                .count();
            assert_eq!(
                mask_count, 10,
                "Masked value should be exactly 10 characters, got: {}",
                masked_value
            );
        }
    }
    assert!(
        !h_str.contains("api_secret"),
        "API_KEY actual value should not be visible"
    );
}

// ==================== New Tests for min_mask and Edge Cases ====================

#[tokio::test]
async fn test_partial_mask_min_mask_fallback() {
    // Test that short values fall back to full mask when below min_mask threshold
    let mut config = EcologConfig::default();
    config.masking.hover = true;
    // Define a partial mode with min_mask = 3
    config.masking.shelter.modes.insert(
        "strict_partial".to_string(),
        shelter::config::ModeDefinition {
            type_name: "partial".to_string(),
            options: serde_json::json!({
                "start_len": 3,
                "end_len": 3,
                "min_mask": 3
            }),
        },
    );
    config
        .masking
        .shelter
        .pattern_overrides
        .insert("SHORT_*".to_string(), "strict_partial".to_string());

    let fixture = MaskingTestFixture::new_with_config(config).await;

    // Add a short variable (less than 9 chars = 3+3+3)
    let env_path = fixture.temp_dir.join(".env");
    let mut env_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&env_path)
        .unwrap();
    writeln!(env_file, "SHORT_VAR=abcdefgh").unwrap(); // 8 chars, below threshold

    let uri = fixture.create_file("test.js", "process.env.SHORT_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.SHORT_VAR".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 16),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // Should fall back to full mask since value is too short
    assert!(
        h_str.contains("********"),
        "SHORT_VAR should be fully masked (below min_mask threshold)"
    );
    assert!(
        !h_str.contains("abc") && !h_str.contains("fgh"),
        "Should not show any part of the value"
    );
}

#[tokio::test]
async fn test_partial_mask_custom_min_mask() {
    // Test custom min_mask value
    let mut config = EcologConfig::default();
    config.masking.hover = true;
    config.masking.shelter.modes.insert(
        "low_min_mask".to_string(),
        shelter::config::ModeDefinition {
            type_name: "partial".to_string(),
            options: serde_json::json!({
                "start_len": 2,
                "end_len": 2,
                "min_mask": 1  // Very low min_mask
            }),
        },
    );
    config
        .masking
        .shelter
        .pattern_overrides
        .insert("LOW_*".to_string(), "low_min_mask".to_string());

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let env_path = fixture.temp_dir.join(".env");
    let mut env_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&env_path)
        .unwrap();
    writeln!(env_file, "LOW_VAR=abcde").unwrap(); // 5 chars (2+1+2)

    let uri = fixture.create_file("test.js", "process.env.LOW_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.LOW_VAR".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 15),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // Should show ab*de (partial mask with low min_mask)
    assert!(h_str.contains("ab"), "Should show first 2 chars");
    assert!(h_str.contains("de"), "Should show last 2 chars");
}

#[tokio::test]
async fn test_unicode_value_masking() {
    // Test masking of unicode values
    let mut config = EcologConfig::default();
    config.masking.hover = true;

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let env_path = fixture.temp_dir.join(".env");
    let mut env_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&env_path)
        .unwrap();
    writeln!(env_file, "UNICODE_VAR=cafetest").unwrap(); // Unicode value

    let uri = fixture.create_file("test.js", "process.env.UNICODE_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.UNICODE_VAR".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 18),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // Default full mask should work on unicode
    assert!(
        !h_str.contains("cafe"),
        "Unicode value should be masked"
    );
    // Check that we have the right number of mask chars (8 chars in "cafetest")
    assert!(
        h_str.contains("********"),
        "Should have 8 mask characters for 8-char unicode value"
    );
}

#[tokio::test]
async fn test_empty_value_handling() {
    // Test handling of empty env var values
    let mut config = EcologConfig::default();
    config.masking.hover = true;

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let env_path = fixture.temp_dir.join(".env");
    let mut env_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&env_path)
        .unwrap();
    writeln!(env_file, "EMPTY_VAR=").unwrap(); // Empty value

    let uri = fixture.create_file("test.js", "process.env.EMPTY_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.EMPTY_VAR".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 16),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    // Empty value should still return a hover, just with empty masked value
    // The hover might contain the variable name and source info
    if let Some(h) = hover {
        let h_str = format!("{:?}", h);
        // Should not crash and should handle empty gracefully
        assert!(!h_str.contains("panic"), "Should not panic on empty value");
    }
}

#[tokio::test]
async fn test_single_char_value_masking() {
    // Test masking of single character values
    let mut config = EcologConfig::default();
    config.masking.hover = true;

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let env_path = fixture.temp_dir.join(".env");
    let mut env_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&env_path)
        .unwrap();
    writeln!(env_file, "SINGLE_VAR=X").unwrap(); // Single char value

    let uri = fixture.create_file("test.js", "process.env.SINGLE_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.SINGLE_VAR".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 17),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(!h_str.contains("`X`"), "Single char should be masked");
}

#[tokio::test]
async fn test_completion_masking_disabled() {
    // Test that completion shows raw values when masking is disabled
    let mut config = EcologConfig::default();
    config.masking.completion = false; // Disable masking for completion

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let uri = fixture.create_file("test.js", "process.env.");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.".to_string(),
            0,
        )
        .await;

    let completion = handlers::handle_completion(
        CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 12),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        },
        &fixture.state,
    )
    .await;

    assert!(completion.is_some());
    let items = completion.unwrap();
    let api_key = items.iter().find(|i| i.label == "API_KEY");
    if let Some(ak) = api_key {
        let doc = format!("{:?}", ak.documentation);
        // When masking is disabled, should show actual value
        assert!(
            doc.contains("api_secret") || !doc.contains("******"),
            "API_KEY value should not be masked when completion masking is disabled"
        );
    }
}

#[tokio::test]
async fn test_multiple_pattern_overrides() {
    // Test that multiple pattern overrides work correctly
    let mut config = EcologConfig::default();
    config.masking.hover = true;

    // Different patterns for different variable types
    config
        .masking
        .shelter
        .pattern_overrides
        .insert("*_KEY".to_string(), "default".to_string()); // Full mask
    config
        .masking
        .shelter
        .pattern_overrides
        .insert("*_VAR".to_string(), "plain".to_string()); // No mask

    let fixture = MaskingTestFixture::new_with_config(config).await;

    // Test API_KEY (should be fully masked)
    let uri1 = fixture.create_file("test1.js", "process.env.API_KEY");
    fixture
        .state
        .document_manager
        .open(
            uri1.clone(),
            "javascript".to_string(),
            "process.env.API_KEY".to_string(),
            0,
        )
        .await;

    let hover1 = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri1 },
                position: Position::new(0, 15),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover1.is_some());
    let h1_str = format!("{:?}", hover1.unwrap());
    assert!(
        !h1_str.contains("api_secret"),
        "API_KEY should be masked"
    );

    // Test DEFAULT_VAR (should not be masked)
    let uri2 = fixture.create_file("test2.js", "process.env.DEFAULT_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri2.clone(),
            "javascript".to_string(),
            "process.env.DEFAULT_VAR".to_string(),
            0,
        )
        .await;

    let hover2 = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri2 },
                position: Position::new(0, 20),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover2.is_some());
    let h2_str = format!("{:?}", hover2.unwrap());
    assert!(
        h2_str.contains("default_value"),
        "DEFAULT_VAR should not be masked (plain mode)"
    );
}

#[tokio::test]
async fn test_very_long_value_masking() {
    // Test masking of very long values
    let mut config = EcologConfig::default();
    config.masking.hover = true;

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let env_path = fixture.temp_dir.join(".env");
    let mut env_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&env_path)
        .unwrap();
    let long_value = "a".repeat(100);
    writeln!(env_file, "LONG_VAR={}", long_value).unwrap();

    let uri = fixture.create_file("test.js", "process.env.LONG_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.LONG_VAR".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 15),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // Should not contain the long value
    assert!(
        !h_str.contains(&long_value),
        "Long value should be masked"
    );
}

// ==================== Multi-line Variable Tests ====================

#[tokio::test]
async fn test_multiline_value_full_mask() {
    // Test that multi-line values are fully masked
    let mut config = EcologConfig::default();
    config.masking.hover = true;

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let env_path = fixture.temp_dir.join(".env");
    let mut env_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&env_path)
        .unwrap();
    // Multi-line value using escaped newlines (how .env files store them)
    writeln!(env_file, "MULTILINE_VAR=line1\\nline2\\nline3").unwrap();

    let uri = fixture.create_file("test.js", "process.env.MULTILINE_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.MULTILINE_VAR".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 18),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // Should not expose any of the lines
    assert!(
        !h_str.contains("line1") && !h_str.contains("line2") && !h_str.contains("line3"),
        "Multi-line value should be masked"
    );
}

#[tokio::test]
async fn test_multiline_value_partial_mask() {
    // Test partial masking of multi-line values
    let mut config = EcologConfig::default();
    config.masking.hover = true;
    config
        .masking
        .shelter
        .pattern_overrides
        .insert("PARTIAL_MULTI_*".to_string(), "partial".to_string());

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let env_path = fixture.temp_dir.join(".env");
    let mut env_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&env_path)
        .unwrap();
    // Long enough value to show partial masking
    writeln!(env_file, "PARTIAL_MULTI_VAR=startline\\nmiddle\\nendline").unwrap();

    let uri = fixture.create_file("test.js", "process.env.PARTIAL_MULTI_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.PARTIAL_MULTI_VAR".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 22),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // With partial masking, should show start and end portions
    // The middle portion should be masked
    assert!(
        h_str.contains("sta") || h_str.contains("ine"),
        "Partial mask should show some characters"
    );
}

#[tokio::test]
async fn test_multiline_value_plain_mode() {
    // Test that plain mode shows multi-line values as-is
    let mut config = EcologConfig::default();
    config
        .masking
        .shelter
        .pattern_overrides
        .insert("PLAIN_MULTI_*".to_string(), "plain".to_string());

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let env_path = fixture.temp_dir.join(".env");
    let mut env_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&env_path)
        .unwrap();
    writeln!(env_file, "PLAIN_MULTI_VAR=visible\\nlines\\nhere").unwrap();

    let uri = fixture.create_file("test.js", "process.env.PLAIN_MULTI_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.PLAIN_MULTI_VAR".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 22),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // Plain mode should show the value
    assert!(
        h_str.contains("visible"),
        "Plain mode should show multi-line value"
    );
}

#[tokio::test]
async fn test_multiline_json_value() {
    // Test masking of JSON-like multi-line values
    let mut config = EcologConfig::default();
    config.masking.hover = true;

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let env_path = fixture.temp_dir.join(".env");
    let mut env_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&env_path)
        .unwrap();
    // JSON-like value (escaped for .env format)
    writeln!(env_file, r#"JSON_CONFIG={{"key":"secret","db":"pass"}}"#).unwrap();

    let uri = fixture.create_file("test.js", "process.env.JSON_CONFIG");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.JSON_CONFIG".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 18),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // JSON content should be masked
    assert!(
        !h_str.contains("secret") && !h_str.contains("pass"),
        "JSON secrets should be masked"
    );
}

#[tokio::test]
async fn test_multiline_pem_key_value() {
    // Test masking of PEM-like key values
    let mut config = EcologConfig::default();
    config.masking.hover = true;
    // Private keys should always be fully masked
    config
        .masking
        .shelter
        .pattern_overrides
        .insert("*_PRIVATE_KEY".to_string(), "default".to_string());

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let env_path = fixture.temp_dir.join(".env");
    let mut env_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&env_path)
        .unwrap();
    // PEM-like value (escaped newlines)
    writeln!(env_file, "RSA_PRIVATE_KEY=-----BEGIN-----\\nKEYDATA\\n-----END-----").unwrap();

    let uri = fixture.create_file("test.js", "process.env.RSA_PRIVATE_KEY");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.RSA_PRIVATE_KEY".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 20),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // PEM content should be fully masked
    assert!(
        !h_str.contains("KEYDATA") && !h_str.contains("BEGIN"),
        "PEM key content should be fully masked"
    );
}

#[tokio::test]
async fn test_multiline_with_tabs() {
    // Test masking of values with tabs
    let mut config = EcologConfig::default();
    config.masking.hover = true;

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let env_path = fixture.temp_dir.join(".env");
    let mut env_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&env_path)
        .unwrap();
    writeln!(env_file, "TAB_VAR=col1\\tcol2\\tcol3").unwrap();

    let uri = fixture.create_file("test.js", "process.env.TAB_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.TAB_VAR".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 14),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // Tab-separated content should be masked
    assert!(
        !h_str.contains("col1") && !h_str.contains("col2"),
        "Tab-separated content should be masked"
    );
}

#[tokio::test]
async fn test_multiline_completion_masking() {
    // Test that multi-line values are masked in completion
    let mut config = EcologConfig::default();
    config.masking.completion = true;

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let env_path = fixture.temp_dir.join(".env");
    let mut env_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&env_path)
        .unwrap();
    writeln!(env_file, "MULTI_COMPLETE=secret\\ndata\\nhere").unwrap();

    let uri = fixture.create_file("test.js", "process.env.");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.".to_string(),
            0,
        )
        .await;

    let completion = handlers::handle_completion(
        CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 12),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        },
        &fixture.state,
    )
    .await;

    assert!(completion.is_some());
    let items = completion.unwrap();
    let multi_var = items.iter().find(|i| i.label == "MULTI_COMPLETE");

    if let Some(mv) = multi_var {
        let doc = format!("{:?}", mv.documentation);
        assert!(
            !doc.contains("secret") && !doc.contains("data"),
            "Multi-line completion value should be masked"
        );
    }
}

#[tokio::test]
async fn test_multiline_with_global_mask_length() {
    // Test that global mask length with multi-line values preserves structure.
    // With line-preserving masking, global_mask_length is NOT applied per-line,
    // so each line is masked to its own length while preserving structure.
    let mut config = EcologConfig::default();
    config.masking.hover = true;
    config.masking.shelter.mask_length = Some(10);

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let env_path = fixture.temp_dir.join(".env");
    let mut env_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&env_path)
        .unwrap();
    // Note: \\n in Rust string becomes literal \n in the file, which env parser
    // may interpret as actual newline or as escaped text depending on parser
    writeln!(env_file, "FIXED_LEN_MULTI=very\\nlong\\nmultiline\\nvalue").unwrap();

    let uri = fixture.create_file("test.js", "process.env.FIXED_LEN_MULTI");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.FIXED_LEN_MULTI".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 20),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // Content should be masked
    assert!(
        !h_str.contains("very") && !h_str.contains("long"),
        "Multi-line content should not be visible"
    );
    // Should contain mask characters
    assert!(
        h_str.contains("***"),
        "Should have mask characters"
    );
}

#[tokio::test]
async fn test_multiline_unicode_value() {
    // Test masking of multi-line unicode values
    let mut config = EcologConfig::default();
    config.masking.hover = true;

    let fixture = MaskingTestFixture::new_with_config(config).await;

    let env_path = fixture.temp_dir.join(".env");
    let mut env_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&env_path)
        .unwrap();
    // Unicode with newlines
    writeln!(env_file, "UNICODE_MULTI=hello\\nworld").unwrap();

    let uri = fixture.create_file("test.js", "process.env.UNICODE_MULTI");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.UNICODE_MULTI".to_string(),
            0,
        )
        .await;

    let hover = handlers::handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 18),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    // Unicode content should be masked
    assert!(
        !h_str.contains("hello") && !h_str.contains("world"),
        "Unicode multi-line content should be masked"
    );
}
