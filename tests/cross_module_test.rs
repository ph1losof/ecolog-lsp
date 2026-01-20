



use abundantis::Abundantis;
use ecolog_lsp::analysis::{
    DocumentManager, ModuleResolver, QueryEngine, WorkspaceIndex, WorkspaceIndexer,
};
use ecolog_lsp::languages::LanguageRegistry;
use ecolog_lsp::server::config::{ConfigManager, EcologConfig};
use ecolog_lsp::server::handlers::handle_hover;
use ecolog_lsp::server::state::ServerState;
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tower_lsp::lsp_types::{
    HoverParams, Position, TextDocumentIdentifier, TextDocumentPositionParams, Url,
};

fn default_config() -> EcologConfig {
    EcologConfig::default()
}

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

    ServerState::new(
        document_manager,
        languages,
        core,
        config_manager,
        workspace_index,
        indexer,
        module_resolver,
    )
}








#[tokio::test]
async fn test_hover_on_imported_env_var() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_cross_module_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    
    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "ANOTHER=test_value").unwrap();

    
    let config_path = temp_dir.join("change-settings.input.ts");
    let config_content = "export const a = process.env.ANOTHER;";
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    
    let test_path = temp_dir.join("test.ts");
    let test_content = r#"import { a } from './change-settings.input';
a;"#;
    let mut f = File::create(&test_path).unwrap();
    write!(f, "{}", test_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    
    let config_uri = Url::from_file_path(&config_path).unwrap();
    let exports = state.workspace_index.get_exports(&config_uri);
    assert!(exports.is_some(), "Should have exports for config file");
    let exports = exports.unwrap();
    assert!(
        exports.named_exports.contains_key("a"),
        "Should have 'a' export"
    );

    
    let test_uri = Url::from_file_path(&test_path).unwrap();
    state
        .document_manager
        .open(
            test_uri.clone(),
            "typescript".to_string(),
            test_content.to_string(),
            0,
        )
        .await;

    
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: test_uri.clone() },
                position: Position::new(1, 0),
            },
            work_done_progress_params: Default::default(),
        },
        &state,
    )
    .await;

    assert!(
        hover.is_some(),
        "Should show hover info for imported env var"
    );

    let hover_str = format!("{:?}", hover.unwrap());
    assert!(
        hover_str.contains("ANOTHER") || hover_str.contains("test_value"),
        "Hover should contain env var info, got: {}",
        hover_str
    );

    let _ = fs::remove_dir_all(&temp_dir);
}


#[tokio::test]
async fn test_export_extraction() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_export_test_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    
    let config_path = temp_dir.join("config.ts");
    let config_content = "export const dbUrl = process.env.DATABASE_URL;";
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    
    let config_uri = Url::from_file_path(&config_path).unwrap();
    let exports = state.workspace_index.get_exports(&config_uri);

    println!("Exports: {:?}", exports);

    assert!(exports.is_some(), "Should have exports for the config file");
    let exports = exports.unwrap();

    assert!(
        exports.named_exports.contains_key("dbUrl"),
        "Should have dbUrl export"
    );

    let db_url_export = exports.named_exports.get("dbUrl").unwrap();
    println!("dbUrl export resolution: {:?}", db_url_export.resolution);

    
    assert!(
        matches!(
            &db_url_export.resolution,
            ecolog_lsp::types::ExportResolution::EnvVar { name } if name == "DATABASE_URL"
        ),
        "dbUrl should resolve to DATABASE_URL, got: {:?}",
        db_url_export.resolution
    );

    let _ = fs::remove_dir_all(&temp_dir);
}


#[tokio::test]
async fn test_import_context_extraction() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_import_test_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    
    let test_path = temp_dir.join("test.ts");
    let test_content = r#"import { foo } from './config';
import { bar as baz } from './utils';
import defaultExport from './default';
foo; baz;"#;
    let mut f = File::create(&test_path).unwrap();
    write!(f, "{}", test_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    
    let test_uri = Url::from_file_path(&test_path).unwrap();
    state
        .document_manager
        .open(
            test_uri.clone(),
            "typescript".to_string(),
            test_content.to_string(),
            0,
        )
        .await;

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    
    let doc = state.document_manager.get(&test_uri).expect("Should have document");
    let import_ctx = &doc.import_context;

    println!("Import context aliases: {:?}", import_ctx.aliases);

    
    assert!(
        import_ctx.aliases.contains_key("foo"),
        "Should have 'foo' in aliases"
    );
    let (module, original) = import_ctx.aliases.get("foo").unwrap();
    assert_eq!(module.as_str(), "./config");
    assert_eq!(original.as_str(), "foo");

    
    assert!(
        import_ctx.aliases.contains_key("baz"),
        "Should have 'baz' in aliases"
    );
    let (module, original) = import_ctx.aliases.get("baz").unwrap();
    assert_eq!(module.as_str(), "./utils");
    assert_eq!(original.as_str(), "bar");

    
    assert!(
        import_ctx.aliases.contains_key("defaultExport"),
        "Should have 'defaultExport' in aliases"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}


#[tokio::test]
async fn test_module_resolution() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_module_res_test_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    
    File::create(temp_dir.join("config.ts")).unwrap();
    File::create(temp_dir.join("change-settings.input.ts")).unwrap();

    let state = setup_test_state(&temp_dir).await;

    
    let test_uri = Url::from_file_path(temp_dir.join("test.ts")).unwrap();
    let lang = state.languages.get_for_uri(&test_uri).unwrap();

    
    let resolved = state.module_resolver.resolve_to_uri("./config", &test_uri, lang.as_ref());
    println!("Resolved ./config: {:?}", resolved);
    assert!(resolved.is_some(), "Should resolve ./config");
    assert!(resolved.unwrap().path().ends_with("config.ts"));

    
    let resolved = state.module_resolver.resolve_to_uri("./change-settings.input", &test_uri, lang.as_ref());
    println!("Resolved ./change-settings.input: {:?}", resolved);
    assert!(resolved.is_some(), "Should resolve ./change-settings.input");
    assert!(resolved.unwrap().path().ends_with("change-settings.input.ts"));

    let _ = fs::remove_dir_all(&temp_dir);
}



#[tokio::test]
async fn test_destructured_export_extraction() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_destructured_export_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    
    let config_path = temp_dir.join("config.ts");
    let config_content = "export const { DB_URL, API_KEY } = process.env;";
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    
    let config_uri = Url::from_file_path(&config_path).unwrap();
    let exports = state.workspace_index.get_exports(&config_uri);

    println!("Destructured exports: {:?}", exports);

    assert!(exports.is_some(), "Should have exports for the config file");
    let exports = exports.unwrap();

    
    assert!(
        exports.named_exports.contains_key("DB_URL"),
        "Should have DB_URL export, got keys: {:?}",
        exports.named_exports.keys().collect::<Vec<_>>()
    );

    let db_url_export = exports.named_exports.get("DB_URL").unwrap();
    println!("DB_URL export resolution: {:?}", db_url_export.resolution);

    assert!(
        matches!(
            &db_url_export.resolution,
            ecolog_lsp::types::ExportResolution::EnvVar { name } if name == "DB_URL"
        ),
        "DB_URL should resolve to DB_URL env var, got: {:?}",
        db_url_export.resolution
    );

    
    assert!(
        exports.named_exports.contains_key("API_KEY"),
        "Should have API_KEY export"
    );

    let api_key_export = exports.named_exports.get("API_KEY").unwrap();
    assert!(
        matches!(
            &api_key_export.resolution,
            ecolog_lsp::types::ExportResolution::EnvVar { name } if name == "API_KEY"
        ),
        "API_KEY should resolve to API_KEY env var, got: {:?}",
        api_key_export.resolution
    );

    let _ = fs::remove_dir_all(&temp_dir);
}



#[tokio::test]
async fn test_aliased_destructured_export_extraction() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_aliased_destruct_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    
    let config_path = temp_dir.join("config.ts");
    let config_content = "export const { DB_URL: dbUrl, API_KEY: apiKey } = process.env;";
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    
    let config_uri = Url::from_file_path(&config_path).unwrap();
    let exports = state.workspace_index.get_exports(&config_uri);

    println!("Aliased destructured exports: {:?}", exports);

    assert!(exports.is_some(), "Should have exports for the config file");
    let exports = exports.unwrap();

    
    assert!(
        exports.named_exports.contains_key("dbUrl"),
        "Should have dbUrl export, got keys: {:?}",
        exports.named_exports.keys().collect::<Vec<_>>()
    );

    let db_url_export = exports.named_exports.get("dbUrl").unwrap();
    println!("dbUrl export: {:?}", db_url_export);
    println!("dbUrl export resolution: {:?}", db_url_export.resolution);

    
    assert!(
        matches!(
            &db_url_export.resolution,
            ecolog_lsp::types::ExportResolution::EnvVar { name } if name == "DB_URL"
        ),
        "dbUrl should resolve to DB_URL env var, got: {:?}",
        db_url_export.resolution
    );

    
    assert!(
        exports.named_exports.contains_key("apiKey"),
        "Should have apiKey export"
    );

    let api_key_export = exports.named_exports.get("apiKey").unwrap();
    assert!(
        matches!(
            &api_key_export.resolution,
            ecolog_lsp::types::ExportResolution::EnvVar { name } if name == "API_KEY"
        ),
        "apiKey should resolve to API_KEY env var, got: {:?}",
        api_key_export.resolution
    );

    let _ = fs::remove_dir_all(&temp_dir);
}



#[tokio::test]
async fn test_env_object_export_extraction() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_env_object_export_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    
    let config_path = temp_dir.join("config.ts");
    let config_content = "export const env = process.env;";
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    
    let config_uri = Url::from_file_path(&config_path).unwrap();
    let exports = state.workspace_index.get_exports(&config_uri);

    println!("Env object exports: {:?}", exports);

    assert!(exports.is_some(), "Should have exports for the config file");
    let exports = exports.unwrap();

    assert!(
        exports.named_exports.contains_key("env"),
        "Should have 'env' export, got keys: {:?}",
        exports.named_exports.keys().collect::<Vec<_>>()
    );

    let env_export = exports.named_exports.get("env").unwrap();
    println!("env export resolution: {:?}", env_export.resolution);

    assert!(
        matches!(
            &env_export.resolution,
            ecolog_lsp::types::ExportResolution::EnvObject { canonical_name } if canonical_name == "env" || canonical_name == "process.env"
        ),
        "env should resolve to EnvObject, got: {:?}",
        env_export.resolution
    );

    let _ = fs::remove_dir_all(&temp_dir);
}




#[tokio::test]
async fn test_hover_on_destructured_import() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_destructured_hover_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    
    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "DB_URL=postgres:

    
    let config_path = temp_dir.join("config.ts");
    let config_content = "export const { DB_URL } = process.env;";
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    
    let test_path = temp_dir.join("test.ts");
    let test_content = r#"import { DB_URL } from './config';
DB_URL;"#;
    let mut f = File::create(&test_path).unwrap();
    write!(f, "{}", test_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    
    let config_uri = Url::from_file_path(&config_path).unwrap();
    let exports = state.workspace_index.get_exports(&config_uri);
    assert!(exports.is_some(), "Should have exports for config file");
    let exports = exports.unwrap();
    assert!(
        exports.named_exports.contains_key("DB_URL"),
        "Should have 'DB_URL' export"
    );

    
    let test_uri = Url::from_file_path(&test_path).unwrap();
    state
        .document_manager
        .open(
            test_uri.clone(),
            "typescript".to_string(),
            test_content.to_string(),
            0,
        )
        .await;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: test_uri.clone() },
                position: Position::new(1, 0),
            },
            work_done_progress_params: Default::default(),
        },
        &state,
    )
    .await;

    assert!(
        hover.is_some(),
        "Should show hover info for destructured imported env var"
    );

    let hover_str = format!("{:?}", hover.unwrap());
    assert!(
        hover_str.contains("DB_URL") || hover_str.contains("postgres"),
        "Hover should contain env var info, got: {}",
        hover_str
    );

    let _ = fs::remove_dir_all(&temp_dir);
}




#[tokio::test]
async fn test_hover_on_env_object_import() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_env_object_hover_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    
    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "SECRET_KEY=super_secret_123").unwrap();

    
    let config_path = temp_dir.join("config.ts");
    let config_content = "export const env = process.env;";
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    
    let test_path = temp_dir.join("test.ts");
    let test_content = r#"import { env } from './config';
env.SECRET_KEY;"#;
    let mut f = File::create(&test_path).unwrap();
    write!(f, "{}", test_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    
    let test_uri = Url::from_file_path(&test_path).unwrap();
    state
        .document_manager
        .open(
            test_uri.clone(),
            "typescript".to_string(),
            test_content.to_string(),
            0,
        )
        .await;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: test_uri.clone() },
                position: Position::new(1, 4),
            },
            work_done_progress_params: Default::default(),
        },
        &state,
    )
    .await;

    assert!(
        hover.is_some(),
        "Should show hover info for env object property access"
    );

    let hover_str = format!("{:?}", hover.unwrap());
    assert!(
        hover_str.contains("SECRET_KEY") || hover_str.contains("super_secret"),
        "Hover should contain env var info, got: {}",
        hover_str
    );

    let _ = fs::remove_dir_all(&temp_dir);
}




#[tokio::test]
async fn test_hover_on_default_export() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_default_export_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    
    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "DATABASE_URL=postgres:

    
    let config_path = temp_dir.join("config.ts");
    let config_content = r#"const dbUrl = process.env.DATABASE_URL;
export default dbUrl;"#;
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    
    let test_path = temp_dir.join("test.ts");
    let test_content = r#"import dbUrl from './config';
dbUrl;"#;
    let mut f = File::create(&test_path).unwrap();
    write!(f, "{}", test_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    
    let config_uri = Url::from_file_path(&config_path).unwrap();
    let exports = state.workspace_index.get_exports(&config_uri);
    assert!(exports.is_some(), "Should have exports for config file");
    let exports = exports.unwrap();

    println!("Default export: {:?}", exports.default_export);

    assert!(
        exports.default_export.is_some(),
        "Should have default export"
    );

    
    let test_uri = Url::from_file_path(&test_path).unwrap();
    state
        .document_manager
        .open(
            test_uri.clone(),
            "typescript".to_string(),
            test_content.to_string(),
            0,
        )
        .await;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: test_uri.clone() },
                position: Position::new(1, 0),
            },
            work_done_progress_params: Default::default(),
        },
        &state,
    )
    .await;

    assert!(
        hover.is_some(),
        "Should show hover info for default imported env var"
    );

    let hover_str = format!("{:?}", hover.unwrap());
    assert!(
        hover_str.contains("DATABASE_URL") || hover_str.contains("postgres"),
        "Hover should contain env var info, got: {}",
        hover_str
    );

    let _ = fs::remove_dir_all(&temp_dir);
}




#[tokio::test]
async fn test_completion_on_imported_env_object() {
    use ecolog_lsp::server::handlers::handle_completion;
    use tower_lsp::lsp_types::{CompletionParams, PartialResultParams, WorkDoneProgressParams};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_completion_import_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    
    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "SECRET_KEY=super_secret_123").unwrap();
    writeln!(env_file, "DATABASE_URL=postgres:

    
    let config_path = temp_dir.join("config.ts");
    let config_content = "export const env = process.env;";
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    
    let test_path = temp_dir.join("test.ts");
    let test_content = r#"import { env } from './config';
env."#;
    let mut f = File::create(&test_path).unwrap();
    write!(f, "{}", test_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    
    let test_uri = Url::from_file_path(&test_path).unwrap();
    state
        .document_manager
        .open(
            test_uri.clone(),
            "typescript".to_string(),
            test_content.to_string(),
            0,
        )
        .await;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    
    let completions = handle_completion(
        CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: test_uri.clone() },
                position: Position::new(1, 4),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        },
        &state,
    )
    .await;

    assert!(
        completions.is_some(),
        "Should provide completions for imported env object"
    );

    let completions = completions.unwrap();
    assert!(
        !completions.is_empty(),
        "Should have completion items"
    );

    
    let labels: Vec<_> = completions.iter().map(|c| c.label.as_str()).collect();
    println!("Completion labels: {:?}", labels);

    assert!(
        labels.contains(&"SECRET_KEY"),
        "Should have SECRET_KEY in completions"
    );
    assert!(
        labels.contains(&"DATABASE_URL"),
        "Should have DATABASE_URL in completions"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}









#[tokio::test]
async fn test_wildcard_reexport_env_var() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_wildcard_reexport_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    
    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "DATABASE_URL=postgres:

    
    let config_path = temp_dir.join("config.ts");
    let config_content = "export const dbUrl = process.env.DATABASE_URL;";
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    
    let index_path = temp_dir.join("index.ts");
    let index_content = r#"export * from "./config";"#;
    let mut f = File::create(&index_path).unwrap();
    write!(f, "{}", index_content).unwrap();

    
    let app_path = temp_dir.join("app.ts");
    let app_content = r#"import { dbUrl } from './index';
dbUrl;"#;
    let mut f = File::create(&app_path).unwrap();
    write!(f, "{}", app_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    
    let config_uri = Url::from_file_path(&config_path).unwrap();
    let exports = state.workspace_index.get_exports(&config_uri);
    assert!(exports.is_some(), "Should have exports for config file");
    let exports = exports.unwrap();
    assert!(
        exports.named_exports.contains_key("dbUrl"),
        "Config should have 'dbUrl' export"
    );

    
    let index_uri = Url::from_file_path(&index_path).unwrap();
    let index_exports = state.workspace_index.get_exports(&index_uri);
    assert!(index_exports.is_some(), "Should have exports for index file");
    let index_exports = index_exports.unwrap();
    assert!(
        !index_exports.wildcard_reexports.is_empty(),
        "Index should have wildcard re-exports, got: {:?}",
        index_exports.wildcard_reexports
    );

    
    let app_uri = Url::from_file_path(&app_path).unwrap();
    state
        .document_manager
        .open(
            app_uri.clone(),
            "typescript".to_string(),
            app_content.to_string(),
            0,
        )
        .await;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: app_uri.clone() },
                position: Position::new(1, 0),
            },
            work_done_progress_params: Default::default(),
        },
        &state,
    )
    .await;

    assert!(
        hover.is_some(),
        "Should show hover info for env var imported through wildcard re-export"
    );

    let hover_str = format!("{:?}", hover.unwrap());
    assert!(
        hover_str.contains("DATABASE_URL") || hover_str.contains("postgres"),
        "Hover should contain env var info, got: {}",
        hover_str
    );

    let _ = fs::remove_dir_all(&temp_dir);
}






#[tokio::test]
async fn test_wildcard_reexport_chain() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_wildcard_chain_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    
    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "X=chain_value").unwrap();

    
    let c_path = temp_dir.join("c.ts");
    let c_content = "export const x = process.env.X;";
    let mut f = File::create(&c_path).unwrap();
    write!(f, "{}", c_content).unwrap();

    let b_path = temp_dir.join("b.ts");
    let b_content = r#"export * from "./c";"#;
    let mut f = File::create(&b_path).unwrap();
    write!(f, "{}", b_content).unwrap();

    let a_path = temp_dir.join("a.ts");
    let a_content = r#"export * from "./b";"#;
    let mut f = File::create(&a_path).unwrap();
    write!(f, "{}", a_content).unwrap();

    
    let app_path = temp_dir.join("app.ts");
    let app_content = r#"import { x } from './a';
x;"#;
    let mut f = File::create(&app_path).unwrap();
    write!(f, "{}", app_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    
    let app_uri = Url::from_file_path(&app_path).unwrap();
    state
        .document_manager
        .open(
            app_uri.clone(),
            "typescript".to_string(),
            app_content.to_string(),
            0,
        )
        .await;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: app_uri.clone() },
                position: Position::new(1, 0),
            },
            work_done_progress_params: Default::default(),
        },
        &state,
    )
    .await;

    assert!(
        hover.is_some(),
        "Should show hover info for env var imported through wildcard re-export chain"
    );

    let hover_str = format!("{:?}", hover.unwrap());
    assert!(
        hover_str.contains("X") || hover_str.contains("chain_value"),
        "Hover should contain env var info, got: {}",
        hover_str
    );

    let _ = fs::remove_dir_all(&temp_dir);
}






#[tokio::test]
async fn test_wildcard_reexport_cycle_detection() {
    use ecolog_lsp::analysis::CrossModuleResolver;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_wildcard_cycle_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    
    let a_path = temp_dir.join("a.ts");
    let a_content = r#"export * from "./b";"#;
    let mut f = File::create(&a_path).unwrap();
    write!(f, "{}", a_content).unwrap();

    let b_path = temp_dir.join("b.ts");
    let b_content = r#"export * from "./a";"#;
    let mut f = File::create(&b_path).unwrap();
    write!(f, "{}", b_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    
    let resolver = CrossModuleResolver::new(
        Arc::clone(&state.workspace_index),
        Arc::clone(&state.module_resolver),
        Arc::clone(&state.languages),
    );

    let a_uri = Url::from_file_path(&a_path).unwrap();

    
    let result = resolver.resolve_import(&a_uri, "./b", "nonexistent", false);

    
    assert!(
        matches!(result, ecolog_lsp::analysis::CrossModuleResolution::Unresolved),
        "Should return Unresolved for cyclic wildcard re-exports, got: {:?}",
        result
    );

    let _ = fs::remove_dir_all(&temp_dir);
}
