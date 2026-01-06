//! Cross-module import tracking integration tests.
//!
//! Tests for hover, definition, and references on imported env vars.

use abundantis::Abundantis;
use ecolog_lsp::analysis::{
    DocumentManager, ModuleResolver, QueryEngine, WorkspaceIndex, WorkspaceIndexer,
};
use ecolog_lsp::languages::LanguageRegistry;
use ecolog_lsp::server::config::{ConfigManager, EcologConfig};
use ecolog_lsp::server::handlers::handle_hover;
use ecolog_lsp::server::state::ServerState;
use shelter::masker::Masker;
use shelter::MaskingConfig;
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
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
    let masker = Arc::new(Mutex::new(Masker::new(MaskingConfig::default())));
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
        masker,
        config_manager,
        workspace_index,
        indexer,
        module_resolver,
    )
}

/// Test that hover works on an imported env var.
///
/// Setup:
/// - change-settings.input.ts: export const a = process.env.ANOTHER
/// - test.ts: import { a } from './change-settings.input'; a;
///
/// Expected: Hover on `a` in test.ts shows ANOTHER env var info.
#[tokio::test]
async fn test_hover_on_imported_env_var() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_cross_module_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    // Create .env file
    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "ANOTHER=test_value").unwrap();

    // Create the exporting file
    let config_path = temp_dir.join("change-settings.input.ts");
    let config_content = "export const a = process.env.ANOTHER;";
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    // Create the importing file
    let test_path = temp_dir.join("test.ts");
    let test_content = r#"import { a } from './change-settings.input';
a;"#;
    let mut f = File::create(&test_path).unwrap();
    write!(f, "{}", test_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    // Run workspace indexing to populate the export index
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    // Verify exports were indexed
    let config_uri = Url::from_file_path(&config_path).unwrap();
    let exports = state.workspace_index.get_exports(&config_uri);
    assert!(exports.is_some(), "Should have exports for config file");
    let exports = exports.unwrap();
    assert!(
        exports.named_exports.contains_key("a"),
        "Should have 'a' export"
    );

    // Open the test file (triggers document analysis)
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

    // Wait for analysis
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Hover on 'a' (line 1, col 0) - the usage of imported `a`
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

/// Test the export extraction specifically.
#[tokio::test]
async fn test_export_extraction() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_export_test_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    // Create file with env var export
    let config_path = temp_dir.join("config.ts");
    let config_content = "export const dbUrl = process.env.DATABASE_URL;";
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    // Run indexing
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    // Check exports
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

    // The resolution should be EnvVar
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

/// Test the import context extraction.
#[tokio::test]
async fn test_import_context_extraction() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_import_test_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    // Create a file with imports
    let test_path = temp_dir.join("test.ts");
    let test_content = r#"import { foo } from './config';
import { bar as baz } from './utils';
import defaultExport from './default';
foo; baz;"#;
    let mut f = File::create(&test_path).unwrap();
    write!(f, "{}", test_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    // Open the file
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

    // Check import context
    let doc = state.document_manager.get(&test_uri).expect("Should have document");
    let import_ctx = &doc.import_context;

    println!("Import context aliases: {:?}", import_ctx.aliases);

    // Check that foo is in aliases
    assert!(
        import_ctx.aliases.contains_key("foo"),
        "Should have 'foo' in aliases"
    );
    let (module, original) = import_ctx.aliases.get("foo").unwrap();
    assert_eq!(module.as_str(), "./config");
    assert_eq!(original.as_str(), "foo");

    // Check that baz (aliased from bar) is in aliases
    assert!(
        import_ctx.aliases.contains_key("baz"),
        "Should have 'baz' in aliases"
    );
    let (module, original) = import_ctx.aliases.get("baz").unwrap();
    assert_eq!(module.as_str(), "./utils");
    assert_eq!(original.as_str(), "bar");

    // Check default import
    assert!(
        import_ctx.aliases.contains_key("defaultExport"),
        "Should have 'defaultExport' in aliases"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Test module resolution.
#[tokio::test]
async fn test_module_resolution() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_module_res_test_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    // Create files
    File::create(temp_dir.join("config.ts")).unwrap();
    File::create(temp_dir.join("change-settings.input.ts")).unwrap();

    let state = setup_test_state(&temp_dir).await;

    // Test resolution
    let test_uri = Url::from_file_path(temp_dir.join("test.ts")).unwrap();
    let lang = state.languages.get_for_uri(&test_uri).unwrap();

    // Resolve ./config
    let resolved = state.module_resolver.resolve_to_uri("./config", &test_uri, lang.as_ref());
    println!("Resolved ./config: {:?}", resolved);
    assert!(resolved.is_some(), "Should resolve ./config");
    assert!(resolved.unwrap().path().ends_with("config.ts"));

    // Resolve ./change-settings.input
    let resolved = state.module_resolver.resolve_to_uri("./change-settings.input", &test_uri, lang.as_ref());
    println!("Resolved ./change-settings.input: {:?}", resolved);
    assert!(resolved.is_some(), "Should resolve ./change-settings.input");
    assert!(resolved.unwrap().path().ends_with("change-settings.input.ts"));

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Test destructured export extraction.
/// `export const { DB_URL } = process.env;`
#[tokio::test]
async fn test_destructured_export_extraction() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_destructured_export_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    // Create file with destructured env export
    let config_path = temp_dir.join("config.ts");
    let config_content = "export const { DB_URL, API_KEY } = process.env;";
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    // Run indexing
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    // Check exports
    let config_uri = Url::from_file_path(&config_path).unwrap();
    let exports = state.workspace_index.get_exports(&config_uri);

    println!("Destructured exports: {:?}", exports);

    assert!(exports.is_some(), "Should have exports for the config file");
    let exports = exports.unwrap();

    // Check DB_URL export
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

    // Check API_KEY export
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

/// Test aliased destructured export extraction.
/// `export const { DB_URL: dbUrl } = process.env;`
#[tokio::test]
async fn test_aliased_destructured_export_extraction() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_aliased_destruct_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    // Create file with aliased destructured env export
    let config_path = temp_dir.join("config.ts");
    let config_content = "export const { DB_URL: dbUrl, API_KEY: apiKey } = process.env;";
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    // Run indexing
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    // Check exports
    let config_uri = Url::from_file_path(&config_path).unwrap();
    let exports = state.workspace_index.get_exports(&config_uri);

    println!("Aliased destructured exports: {:?}", exports);

    assert!(exports.is_some(), "Should have exports for the config file");
    let exports = exports.unwrap();

    // Check dbUrl export (aliased from DB_URL)
    assert!(
        exports.named_exports.contains_key("dbUrl"),
        "Should have dbUrl export, got keys: {:?}",
        exports.named_exports.keys().collect::<Vec<_>>()
    );

    let db_url_export = exports.named_exports.get("dbUrl").unwrap();
    println!("dbUrl export: {:?}", db_url_export);
    println!("dbUrl export resolution: {:?}", db_url_export.resolution);

    // The exported name is "dbUrl", but it resolves to env var "DB_URL"
    assert!(
        matches!(
            &db_url_export.resolution,
            ecolog_lsp::types::ExportResolution::EnvVar { name } if name == "DB_URL"
        ),
        "dbUrl should resolve to DB_URL env var, got: {:?}",
        db_url_export.resolution
    );

    // Check apiKey export (aliased from API_KEY)
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

/// Test env object export extraction.
/// `export const env = process.env;`
#[tokio::test]
async fn test_env_object_export_extraction() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_env_object_export_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    // Create file with env object export
    let config_path = temp_dir.join("config.ts");
    let config_content = "export const env = process.env;";
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    // Run indexing
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    // Check exports
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

/// Test hover on destructured imported env var.
/// config.ts: export const { DB_URL } = process.env;
/// test.ts: import { DB_URL } from './config'; DB_URL;
#[tokio::test]
async fn test_hover_on_destructured_import() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_destructured_hover_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    // Create .env file
    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "DB_URL=postgres://localhost/test").unwrap();

    // Create the exporting file with destructured export
    let config_path = temp_dir.join("config.ts");
    let config_content = "export const { DB_URL } = process.env;";
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    // Create the importing file
    let test_path = temp_dir.join("test.ts");
    let test_content = r#"import { DB_URL } from './config';
DB_URL;"#;
    let mut f = File::create(&test_path).unwrap();
    write!(f, "{}", test_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    // Run workspace indexing
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    // Verify exports were indexed with correct resolution
    let config_uri = Url::from_file_path(&config_path).unwrap();
    let exports = state.workspace_index.get_exports(&config_uri);
    assert!(exports.is_some(), "Should have exports for config file");
    let exports = exports.unwrap();
    assert!(
        exports.named_exports.contains_key("DB_URL"),
        "Should have 'DB_URL' export"
    );

    // Open the test file
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

    // Hover on 'DB_URL' (line 1, col 0)
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

/// Test hover on env object property access.
/// config.ts: export const env = process.env;
/// test.ts: import { env } from './config'; env.SECRET_KEY;
#[tokio::test]
async fn test_hover_on_env_object_import() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_env_object_hover_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    // Create .env file
    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "SECRET_KEY=super_secret_123").unwrap();

    // Create the exporting file with env object export
    let config_path = temp_dir.join("config.ts");
    let config_content = "export const env = process.env;";
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    // Create the importing file that uses env.SECRET_KEY
    let test_path = temp_dir.join("test.ts");
    let test_content = r#"import { env } from './config';
env.SECRET_KEY;"#;
    let mut f = File::create(&test_path).unwrap();
    write!(f, "{}", test_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    // Run workspace indexing
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    // Open the test file
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

    // Hover on 'SECRET_KEY' (line 1, col 4) - after "env."
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

/// Test default export.
/// config.ts: const dbUrl = process.env.DATABASE_URL; export default dbUrl;
/// test.ts: import dbUrl from './config'; dbUrl;
#[tokio::test]
async fn test_hover_on_default_export() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("ecolog_default_export_{}", timestamp));
    fs::create_dir_all(&temp_dir).unwrap();

    // Create .env file
    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "DATABASE_URL=postgres://localhost/db").unwrap();

    // Create the exporting file with default export
    let config_path = temp_dir.join("config.ts");
    let config_content = r#"const dbUrl = process.env.DATABASE_URL;
export default dbUrl;"#;
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    // Create the importing file
    let test_path = temp_dir.join("test.ts");
    let test_content = r#"import dbUrl from './config';
dbUrl;"#;
    let mut f = File::create(&test_path).unwrap();
    write!(f, "{}", test_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    // Run workspace indexing
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    // Verify default export was indexed
    let config_uri = Url::from_file_path(&config_path).unwrap();
    let exports = state.workspace_index.get_exports(&config_uri);
    assert!(exports.is_some(), "Should have exports for config file");
    let exports = exports.unwrap();

    println!("Default export: {:?}", exports.default_export);

    assert!(
        exports.default_export.is_some(),
        "Should have default export"
    );

    // Open the test file
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

    // Hover on 'dbUrl' (line 1, col 0)
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

/// Test completion on imported env object.
/// config.ts: export const env = process.env;
/// test.ts: import { env } from './config'; env.|
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

    // Create .env file
    let env_path = temp_dir.join(".env");
    let mut env_file = File::create(&env_path).unwrap();
    writeln!(env_file, "SECRET_KEY=super_secret_123").unwrap();
    writeln!(env_file, "DATABASE_URL=postgres://localhost/db").unwrap();

    // Create the exporting file with env object export
    let config_path = temp_dir.join("config.ts");
    let config_content = "export const env = process.env;";
    let mut f = File::create(&config_path).unwrap();
    write!(f, "{}", config_content).unwrap();

    // Create the importing file - cursor is after "env."
    let test_path = temp_dir.join("test.ts");
    let test_content = r#"import { env } from './config';
env."#;
    let mut f = File::create(&test_path).unwrap();
    write!(f, "{}", test_content).unwrap();

    let state = setup_test_state(&temp_dir).await;

    // Run workspace indexing
    let config = default_config();
    state.indexer.index_workspace(&config).await.unwrap();

    // Open the test file
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

    // Request completion at "env.|" (line 1, col 4)
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

    // Check that we have the expected env vars
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
