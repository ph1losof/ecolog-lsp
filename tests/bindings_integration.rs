use abundantis::Abundantis;
use ecolog_lsp::analysis::{
    DocumentManager, ModuleResolver, QueryEngine, WorkspaceIndex, WorkspaceIndexer,
};
use ecolog_lsp::languages::LanguageRegistry;
use ecolog_lsp::server::config::ConfigManager;
use ecolog_lsp::server::handlers::{compute_diagnostics, handle_hover};
use ecolog_lsp::server::state::ServerState;
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tower_lsp::lsp_types::{
    HoverParams, Position, TextDocumentIdentifier, TextDocumentPositionParams, Url,
};

#[tokio::test]
async fn test_bindings_integration() {
    
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_integ_test_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    
    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "DB_URL=postgres://localhost").unwrap();
    writeln!(env_file, "API_KEY=secret_key").unwrap();
    writeln!(env_file, "JSON_BLOB=some_data").unwrap();

    
    let mut registry = LanguageRegistry::new();
    registry.register(Arc::new(ecolog_lsp::languages::javascript::JavaScript));
    registry.register(Arc::new(ecolog_lsp::languages::typescript::TypeScript));
    registry.register(Arc::new(ecolog_lsp::languages::python::Python));
    let languages = Arc::new(registry);

    let query_engine = Arc::new(QueryEngine::new());
    let document_manager = Arc::new(DocumentManager::new(query_engine.clone(), languages.clone()));
    let config_manager = Arc::new(ConfigManager::new());
    let core = Arc::new(
        Abundantis::builder()
            .root(&temp_dir)
            .build()
            .await
            .expect("Failed to build Abundantis"),
    );
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
        config_manager,
        workspace_index,
        indexer,
        module_resolver,
    );

    
    let js_path = temp_dir.join("bracket.js");
    let js_content = r#"
const a = process.env['JSON_BLOB'];
a;
"#;
    let mut f = File::create(&js_path).unwrap();
    write!(f, "{}", js_content).unwrap();
    let uri_js = Url::from_file_path(&js_path).unwrap();

    state
        .document_manager
        .open(
            uri_js.clone(),
            "javascript".to_string(),
            js_content.to_string(),
            0,
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: uri_js.clone(),
                },
                position: Position::new(2, 0),
            },
            work_done_progress_params: Default::default(),
        },
        &state,
    )
    .await;

    assert!(hover.is_some(), "JS Bracket Access Hover failed");
    assert!(format!("{:?}", hover.unwrap()).contains("JSON_BLOB"));

    
    let ts_path = temp_dir.join("destruct.ts");
    let ts_content = r#"
const { API_KEY } = process.env;
API_KEY;
"#;
    let mut f = File::create(&ts_path).unwrap();
    write!(f, "{}", ts_content).unwrap();
    let uri_ts = Url::from_file_path(&ts_path).unwrap();

    state
        .document_manager
        .open(
            uri_ts.clone(),
            "typescript".to_string(),
            ts_content.to_string(),
            0,
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: uri_ts.clone(),
                },
                position: Position::new(2, 0),
            },
            work_done_progress_params: Default::default(),
        },
        &state,
    )
    .await;

    assert!(hover.is_some(), "TS Destructuring Hover failed");
    assert!(format!("{:?}", hover.unwrap()).contains("API_KEY"));

    
    let ts_bracket_path = temp_dir.join("bracket.ts");
    let ts_bracket_content = r#"
const b = process.env['JSON_BLOB'];
b;
"#;
    let mut f = File::create(&ts_bracket_path).unwrap();
    write!(f, "{}", ts_bracket_content).unwrap();
    let uri_ts_bracket = Url::from_file_path(&ts_bracket_path).unwrap();

    state
        .document_manager
        .open(
            uri_ts_bracket.clone(),
            "typescript".to_string(),
            ts_bracket_content.to_string(),
            0,
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: uri_ts_bracket.clone(),
                },
                position: Position::new(2, 0),
            },
            work_done_progress_params: Default::default(),
        },
        &state,
    )
    .await;

    assert!(hover.is_some(), "TS Bracket Access Hover failed");
    assert!(format!("{:?}", hover.unwrap()).contains("JSON_BLOB"));

    
    let scope_path = temp_dir.join("scope.js");
    let scope_content = r#"
function test() {
  const secret = process.env.API_KEY;
}
secret;
"#;
    let mut f = File::create(&scope_path).unwrap();
    write!(f, "{}", scope_content).unwrap();
    let uri_scope = Url::from_file_path(&scope_path).unwrap();

    state
        .document_manager
        .open(
            uri_scope.clone(),
            "javascript".to_string(),
            scope_content.to_string(),
            0,
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: uri_scope.clone(),
                },
                position: Position::new(4, 0),
            },
            work_done_progress_params: Default::default(),
        },
        &state,
    )
    .await;

    assert!(
        hover.is_none(),
        "Scope Isolation Failed: Should not hover out-of-scope variable"
    );

    
    let alias_path = temp_dir.join("alias_msg.js");
    let alias_content = r#"
const env = process.env;
env;
"#;
    let mut f = File::create(&alias_path).unwrap();
    write!(f, "{}", alias_content).unwrap();
    let uri_alias = Url::from_file_path(&alias_path).unwrap();

    state
        .document_manager
        .open(
            uri_alias.clone(),
            "javascript".to_string(),
            alias_content.to_string(),
            0,
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    
    let hover_decl = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: uri_alias.clone(),
                },
                position: Position::new(1, 6),
            },
            work_done_progress_params: Default::default(),
        },
        &state,
    )
    .await;

    println!("Alias Decl Hover: {:?}", hover_decl);
    assert!(
        hover_decl.is_some(),
        "Expected hover for object alias declaration"
    );
    let hover_str = format!("{:?}", hover_decl.unwrap());
    assert!(
        hover_str.contains("Environment Object"),
        "Detailed message should indicate Environment Object, got: {}",
        hover_str
    );
    assert!(
        !hover_str.contains("(undefined)"),
        "Should not show (undefined) for object alias"
    );

    
    let _ = fs::remove_dir_all(&temp_dir);
}


#[tokio::test]
async fn test_destructuring_diagnostics() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_destruct_diag_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    
    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "DB_URL=postgres://localhost").unwrap();

    
    let mut registry = LanguageRegistry::new();
    registry.register(Arc::new(ecolog_lsp::languages::javascript::JavaScript));
    let languages = Arc::new(registry);

    let query_engine = Arc::new(QueryEngine::new());
    let document_manager = Arc::new(DocumentManager::new(query_engine.clone(), languages.clone()));
    let config_manager = Arc::new(ConfigManager::new());
    let core = Arc::new(
        Abundantis::builder()
            .root(&temp_dir)
            .build()
            .await
            .expect("Failed to build Abundantis"),
    );
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
        config_manager,
        workspace_index,
        indexer,
        module_resolver,
    );

    
    let js_direct = temp_dir.join("direct.js");
    let js_direct_content = "const a = process.env.UNDEFINED_VAR;";
    let mut f = File::create(&js_direct).unwrap();
    write!(f, "{}", js_direct_content).unwrap();
    let uri_direct = Url::from_file_path(&js_direct).unwrap();

    state
        .document_manager
        .open(
            uri_direct.clone(),
            "javascript".to_string(),
            js_direct_content.to_string(),
            0,
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let diagnostics_direct = compute_diagnostics(&uri_direct, &state).await;
    println!("Direct access diagnostics: {:?}", diagnostics_direct);
    assert!(
        !diagnostics_direct.is_empty(),
        "Should have diagnostic for undefined UNDEFINED_VAR in direct access"
    );
    assert!(
        diagnostics_direct
            .iter()
            .any(|d| d.message.contains("UNDEFINED_VAR")),
        "Diagnostic should mention UNDEFINED_VAR"
    );

    
    let js_destruct = temp_dir.join("destruct.js");
    let js_destruct_content = "const { UNDEFINED_VAR } = process.env;";
    let mut f = File::create(&js_destruct).unwrap();
    write!(f, "{}", js_destruct_content).unwrap();
    let uri_destruct = Url::from_file_path(&js_destruct).unwrap();

    state
        .document_manager
        .open(
            uri_destruct.clone(),
            "javascript".to_string(),
            js_destruct_content.to_string(),
            0,
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let diagnostics_destruct = compute_diagnostics(&uri_destruct, &state).await;
    println!("Destructuring diagnostics: {:?}", diagnostics_destruct);
    assert!(
        !diagnostics_destruct.is_empty(),
        "Should have diagnostic for undefined UNDEFINED_VAR in destructuring"
    );
    assert!(
        diagnostics_destruct
            .iter()
            .any(|d| d.message.contains("UNDEFINED_VAR")),
        "Diagnostic should mention UNDEFINED_VAR"
    );

    
    let js_renamed = temp_dir.join("renamed.js");
    let js_renamed_content = "const { UNDEFINED_VAR: myVar } = process.env;";
    let mut f = File::create(&js_renamed).unwrap();
    write!(f, "{}", js_renamed_content).unwrap();
    let uri_renamed = Url::from_file_path(&js_renamed).unwrap();

    state
        .document_manager
        .open(
            uri_renamed.clone(),
            "javascript".to_string(),
            js_renamed_content.to_string(),
            0,
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let diagnostics_renamed = compute_diagnostics(&uri_renamed, &state).await;
    println!(
        "Renamed destructuring diagnostics: {:?}",
        diagnostics_renamed
    );
    assert!(
        !diagnostics_renamed.is_empty(),
        "Should have diagnostic for undefined UNDEFINED_VAR in renamed destructuring"
    );
    assert!(
        diagnostics_renamed
            .iter()
            .any(|d| d.message.contains("UNDEFINED_VAR")),
        "Diagnostic should mention UNDEFINED_VAR"
    );

    
    let js_defined = temp_dir.join("defined.js");
    let js_defined_content = "const { DB_URL } = process.env;";
    let mut f = File::create(&js_defined).unwrap();
    write!(f, "{}", js_defined_content).unwrap();
    let uri_defined = Url::from_file_path(&js_defined).unwrap();

    state
        .document_manager
        .open(
            uri_defined.clone(),
            "javascript".to_string(),
            js_defined_content.to_string(),
            0,
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let diagnostics_defined = compute_diagnostics(&uri_defined, &state).await;
    println!("Defined var diagnostics: {:?}", diagnostics_defined);
    assert!(
        diagnostics_defined.is_empty()
            || !diagnostics_defined
                .iter()
                .any(|d| d.message.contains("DB_URL")),
        "Should NOT have diagnostic for defined DB_URL"
    );

    
    let _ = fs::remove_dir_all(&temp_dir);
}
