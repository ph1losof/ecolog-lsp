







use abundantis::source::remote::ProviderManager;
use abundantis::Abundantis;
use ecolog_lsp::analysis::{
    DocumentManager, ModuleResolver, QueryEngine, WorkspaceIndex, WorkspaceIndexer,
};
use ecolog_lsp::languages::LanguageRegistry;
use ecolog_lsp::server::config::ConfigManager;
use ecolog_lsp::server::handlers::handle_hover;
use ecolog_lsp::server::state::ServerState;
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tower_lsp::lsp_types::{
    HoverParams, Position, TextDocumentIdentifier, TextDocumentPositionParams, Url,
};

async fn setup_test_state(temp_dir: &std::path::Path) -> ServerState {
    let mut registry = LanguageRegistry::new();
    registry.register(Arc::new(ecolog_lsp::languages::javascript::JavaScript));
    registry.register(Arc::new(ecolog_lsp::languages::typescript::TypeScript));
    let languages = Arc::new(registry);

    let query_engine = Arc::new(QueryEngine::new());
    let document_manager = Arc::new(DocumentManager::new(query_engine.clone(), languages.clone()));
    let config_manager = Arc::new(ConfigManager::new());
    let core = Arc::new(
        Abundantis::builder()
            .root(temp_dir)
            .build()
            .await
            .expect("Failed to build Abundantis"),
    );
    let workspace_index = Arc::new(WorkspaceIndex::new());
    let module_resolver = Arc::new(ModuleResolver::new(temp_dir.to_path_buf()));
    let indexer = Arc::new(WorkspaceIndexer::new(
        Arc::clone(&workspace_index),
        query_engine,
        Arc::clone(&languages),
        temp_dir.to_path_buf(),
    ));

    let providers_config = abundantis::config::ProvidersConfig::default();
    let provider_manager = Arc::new(ProviderManager::new(providers_config));

    ServerState::new(
        document_manager,
        languages,
        core,
        config_manager,
        workspace_index,
        indexer,
        module_resolver,
        provider_manager,
    )
}





#[tokio::test]
async fn test_multi_level_binding_chain() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_chain_test_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    
    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "DB_URL=postgres://localhost").unwrap();

    let state = setup_test_state(&temp_dir).await;

    let js_path = temp_dir.join("chain.js");
    let js_content = r#"const a = process.env.DB_URL;
const b = a;
b;"#;
    let mut f = File::create(&js_path).unwrap();
    write!(f, "{}", js_content).unwrap();
    let uri = Url::from_file_path(&js_path).unwrap();

    state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            js_content.to_string(),
            0,
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(2, 0),
            },
            work_done_progress_params: Default::default(),
        },
        &state,
    )
    .await;

    println!("Chain test hover: {:?}", hover);
    assert!(
        hover.is_some(),
        "Should resolve 'b' through the chain to DB_URL"
    );
    let hover_str = format!("{:?}", hover.unwrap());
    assert!(
        hover_str.contains("DB_URL") || hover_str.contains("postgres"),
        "Hover should resolve to DB_URL, got: {}",
        hover_str
    );

    let _ = fs::remove_dir_all(&temp_dir);
}





#[tokio::test]
async fn test_object_alias_destructure() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_destruct_test_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "DB_URL=postgres://localhost").unwrap();

    let state = setup_test_state(&temp_dir).await;

    let js_path = temp_dir.join("destruct.js");
    let js_content = r#"const c = process.env;
const { DB_URL } = c;
DB_URL;"#;
    let mut f = File::create(&js_path).unwrap();
    write!(f, "{}", js_content).unwrap();
    let uri = Url::from_file_path(&js_path).unwrap();

    state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            js_content.to_string(),
            0,
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(2, 0),
            },
            work_done_progress_params: Default::default(),
        },
        &state,
    )
    .await;

    println!("Destructure test hover: {:?}", hover);
    assert!(
        hover.is_some(),
        "Should resolve destructured DB_URL from alias"
    );
    let hover_str = format!("{:?}", hover.unwrap());
    assert!(
        hover_str.contains("DB_URL") || hover_str.contains("postgres"),
        "Hover should resolve to DB_URL, got: {}",
        hover_str
    );

    let _ = fs::remove_dir_all(&temp_dir);
}






#[tokio::test]
async fn test_reassignment_through_alias() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_reassign_test_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "ALPHA=secret_alpha").unwrap();

    let state = setup_test_state(&temp_dir).await;

    let js_path = temp_dir.join("reassign.js");
    let js_content = r#"const c = process.env;
let d = c;
let { ALPHA: alpha } = d;
alpha;"#;
    let mut f = File::create(&js_path).unwrap();
    write!(f, "{}", js_content).unwrap();
    let uri = Url::from_file_path(&js_path).unwrap();

    state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            js_content.to_string(),
            0,
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(3, 0),
            },
            work_done_progress_params: Default::default(),
        },
        &state,
    )
    .await;

    println!("Reassignment test hover: {:?}", hover);
    assert!(
        hover.is_some(),
        "Should resolve 'alpha' through the chain to ALPHA"
    );
    let hover_str = format!("{:?}", hover.unwrap());
    assert!(
        hover_str.contains("ALPHA") || hover_str.contains("secret_alpha"),
        "Hover should resolve to ALPHA, got: {}",
        hover_str
    );

    let _ = fs::remove_dir_all(&temp_dir);
}







#[tokio::test]
async fn test_scope_isolation_detailed() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_scope_test_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "DB_URL=postgres://localhost").unwrap();

    let state = setup_test_state(&temp_dir).await;

    let js_path = temp_dir.join("scope.js");
    let js_content = r#"function test() {
  const { DB_URL: something } = process.env;
  something;
}
something;"#;
    let mut f = File::create(&js_path).unwrap();
    write!(f, "{}", js_content).unwrap();
    let uri = Url::from_file_path(&js_path).unwrap();

    state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            js_content.to_string(),
            0,
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    
    let hover_inside = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(2, 2),
            },
            work_done_progress_params: Default::default(),
        },
        &state,
    )
    .await;

    println!("Scope test hover inside: {:?}", hover_inside);
    assert!(
        hover_inside.is_some(),
        "Should resolve 'something' inside the function"
    );
    let hover_str = format!("{:?}", hover_inside.unwrap());
    assert!(
        hover_str.contains("DB_URL") || hover_str.contains("postgres"),
        "Hover inside function should resolve to DB_URL, got: {}",
        hover_str
    );

    
    let hover_outside = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(4, 0),
            },
            work_done_progress_params: Default::default(),
        },
        &state,
    )
    .await;

    println!("Scope test hover outside: {:?}", hover_outside);
    assert!(
        hover_outside.is_none(),
        "Should NOT resolve 'something' outside the function scope"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}
