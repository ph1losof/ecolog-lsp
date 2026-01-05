use abundantis::Abundantis;
use ecolog_lsp::analysis::DocumentManager;
use ecolog_lsp::languages::LanguageRegistry;
use ecolog_lsp::server::config::ConfigManager;
use ecolog_lsp::server::handlers::handle_semantic_tokens_full;
use ecolog_lsp::server::state::ServerState;
use shelter::masker::Masker;
use shelter::MaskingConfig;
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tower_lsp::lsp_types::{SemanticTokensParams, TextDocumentIdentifier, Url};

#[tokio::test]
async fn test_glob_config_semantic_tokens() {
    // Setup unique temp dir
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_glob_test_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    // Create a custom env file matching the pattern we will configure
    let env_path = temp_dir.join("production.env.custom");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "API_KEY=12345").unwrap();

    // Setup Server
    let registry = LanguageRegistry::new();
    let languages = Arc::new(registry);
    let query_engine = Arc::new(ecolog_lsp::analysis::QueryEngine::new());
    let document_manager = Arc::new(DocumentManager::new(query_engine, languages.clone()));
    let config_manager = Arc::new(ConfigManager::new());
    let core = Arc::new(
        Abundantis::builder()
            .root(&temp_dir)
            .build()
            .await
            .expect("Failed to build Abundantis"),
    );
    let masker = Arc::new(Mutex::new(Masker::new(MaskingConfig::default())));

    let state = ServerState::new(
        document_manager,
        languages.clone(),
        core,
        masker,
        config_manager.clone(),
    );

    // Update Config with custom pattern
    {
        let config_arc = config_manager.get_config();
        let mut config = config_arc.write().await;
        config.workspace.env_files = vec!["*.env.custom".into()];
    }

    // Open the document
    let uri = Url::from_file_path(&env_path).unwrap();
    let content = fs::read_to_string(&env_path).unwrap();
    state
        .document_manager
        .open(uri.clone(), "plaintext".to_string(), content, 0)
        .await;

    // Call Semantic Tokens Handler
    let params = SemanticTokensParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let result = handle_semantic_tokens_full(params, &state).await;

    // Assert we got tokens (means file was recognized)
    assert!(
        result.is_some(),
        "Expected semantic tokens for custom env file pattern"
    );
    let tokens = result.unwrap();
    if let tower_lsp::lsp_types::SemanticTokensResult::Tokens(t) = tokens {
        assert!(!t.data.is_empty(), "Expected tokens data to be non-empty");
    } else {
        panic!("Expected tokens result");
    }

    // Test a file that should NOT match
    let other_path = temp_dir.join("other.txt");
    let mut f = File::create(&other_path).unwrap();
    writeln!(f, "SOME=VAL").unwrap();
    let uri_other = Url::from_file_path(&other_path).unwrap();
    state
        .document_manager
        .open(
            uri_other.clone(),
            "plaintext".to_string(),
            "SOME=VAL".to_string(),
            0,
        )
        .await;

    let params_other = SemanticTokensParams {
        text_document: TextDocumentIdentifier {
            uri: uri_other.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let result_other = handle_semantic_tokens_full(params_other, &state).await;
    assert!(
        result_other.is_none(),
        "Should not return tokens for non-matching file"
    );

    // Verify ecolog.file.list command
    let params = tower_lsp::lsp_types::ExecuteCommandParams {
        command: "ecolog.file.list".to_string(),
        arguments: vec![],
        work_done_progress_params: Default::default(),
    };

    let result = ecolog_lsp::server::handlers::handle_execute_command(params, &state).await;
    assert!(result.is_some(), "Expected result from ecolog.file.list");
    let val = result.unwrap();
    let files = val
        .get("files")
        .expect("Expected files list")
        .as_array()
        .expect("Expected array");

    // Check if our custom file is in the list
    let found = files
        .iter()
        .any(|v| v.as_str() == Some("production.env.custom"));
    assert!(
        found,
        "production.env.custom should be found by ecolog.file.list"
    );

    // Test a non-matching file
    let other_path = temp_dir.join("other.txt");
    let mut f = File::create(&other_path).unwrap();
    writeln!(f, "SOME=VAL").unwrap();

    // Re-run list
    let params_retry = tower_lsp::lsp_types::ExecuteCommandParams {
        command: "ecolog.file.list".to_string(),
        arguments: vec![],
        work_done_progress_params: Default::default(),
    };
    let result_retry =
        ecolog_lsp::server::handlers::handle_execute_command(params_retry, &state).await;
    let val_retry = result_retry.unwrap();
    let files_retry = val_retry.get("files").unwrap().as_array().unwrap();

    let found_other = files_retry.iter().any(|v| v.as_str() == Some("other.txt"));
    assert!(
        !found_other,
        "other.txt should NOT be found by ecolog.file.list"
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}
