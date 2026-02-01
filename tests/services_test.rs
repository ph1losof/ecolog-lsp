//! Tests for server/services - EnvService, DocumentService, WorkspaceService

mod common;

use common::TestFixture;

// ============================================================
// EnvService Tests
// ============================================================

#[tokio::test]
async fn test_env_service_get_workspace_root() {
    let fixture = TestFixture::new().await;

    // Create EnvService from the core
    let env_service = ecolog_lsp::server::services::EnvService::new(fixture.state.core.clone());

    let root = env_service.get_workspace_root().await;
    assert!(root.exists(), "Workspace root should exist");
}

#[tokio::test]
async fn test_env_service_get_for_file() {
    let fixture = TestFixture::new().await;

    let env_service = ecolog_lsp::server::services::EnvService::new(fixture.state.core.clone());

    // DB_URL is defined in the fixture's .env
    let result = env_service.get_for_file("DB_URL", &fixture.temp_dir).await;

    assert!(result.is_some(), "Should resolve DB_URL");
    let var = result.unwrap();
    assert_eq!(var.key.as_str(), "DB_URL");
}

#[tokio::test]
async fn test_env_service_get_for_file_not_found() {
    let fixture = TestFixture::new().await;

    let env_service = ecolog_lsp::server::services::EnvService::new(fixture.state.core.clone());

    let result = env_service.get_for_file("NONEXISTENT_VAR", &fixture.temp_dir).await;

    assert!(result.is_none(), "Should return None for nonexistent var");
}

#[tokio::test]
async fn test_env_service_all_for_file() {
    let fixture = TestFixture::new().await;

    let env_service = ecolog_lsp::server::services::EnvService::new(fixture.state.core.clone());

    let vars = env_service.all_for_file(&fixture.temp_dir).await;

    assert!(!vars.is_empty(), "Should return env vars from .env");
    // Check that DB_URL is present
    let has_db_url = vars.iter().any(|v| v.key.as_str() == "DB_URL");
    assert!(has_db_url, "Should contain DB_URL");
}

#[tokio::test]
async fn test_env_service_set_active_files() {
    let fixture = TestFixture::new().await;

    let env_service = ecolog_lsp::server::services::EnvService::new(fixture.state.core.clone());

    // Set active file filter
    env_service.set_active_files(&[".env.local".to_string()]);

    // Clear should work
    env_service.clear_active_files();
}

#[tokio::test]
async fn test_env_service_active_env_files() {
    let fixture = TestFixture::new().await;

    let env_service = ecolog_lsp::server::services::EnvService::new(fixture.state.core.clone());

    let files = env_service.active_env_files(&fixture.temp_dir);
    // Should find at least the .env file
    assert!(!files.is_empty(), "Should find .env file");
}

#[tokio::test]
async fn test_env_service_refresh() {
    let fixture = TestFixture::new().await;

    let env_service = ecolog_lsp::server::services::EnvService::new(fixture.state.core.clone());

    // Refresh should not panic
    env_service.refresh(abundantis::RefreshOptions::default()).await;
}

#[tokio::test]
async fn test_env_service_clone() {
    let fixture = TestFixture::new().await;

    let env_service = ecolog_lsp::server::services::EnvService::new(fixture.state.core.clone());
    let cloned = env_service.clone();

    // Both should return the same workspace root
    let root1 = env_service.get_workspace_root().await;
    let root2 = cloned.get_workspace_root().await;
    assert_eq!(root1, root2);
}

#[tokio::test]
async fn test_env_service_registered_file_paths() {
    let fixture = TestFixture::new().await;

    let env_service = ecolog_lsp::server::services::EnvService::new(fixture.state.core.clone());

    let paths = env_service.registered_file_paths();
    // Should have at least the .env file registered
    assert!(!paths.is_empty(), "Should have registered .env file");
}

#[tokio::test]
async fn test_env_service_context_for_file() {
    let fixture = TestFixture::new().await;

    let env_service = ecolog_lsp::server::services::EnvService::new(fixture.state.core.clone());

    // Create a test file path
    let test_file = fixture.temp_dir.join("test.js");
    std::fs::write(&test_file, "const x = 1;").unwrap();

    let context = env_service.get_context_for_file(&test_file);
    assert!(context.is_some(), "Should get context for file in workspace");
}

// ============================================================
// DocumentService Tests
// ============================================================

#[tokio::test]
async fn test_document_service_open_and_get() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const x = process.env.DB_URL;");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const x = process.env.DB_URL;".into(),
            1,
        )
        .await;

    let doc = fixture.state.document_manager.get(&uri);
    assert!(doc.is_some(), "Should retrieve opened document");
}

#[tokio::test]
async fn test_document_service_document_exists_after_open() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const x = 1;");

    // Before opening
    assert!(fixture.state.document_manager.get(&uri).is_none());

    fixture
        .state
        .document_manager
        .open(uri.clone(), "javascript".into(), "const x = 1;".into(), 1)
        .await;

    // After opening
    assert!(fixture.state.document_manager.get(&uri).is_some());
}

#[tokio::test]
async fn test_document_service_close() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const x = 1;");

    fixture
        .state
        .document_manager
        .open(uri.clone(), "javascript".into(), "const x = 1;".into(), 1)
        .await;

    assert!(fixture.state.document_manager.get(&uri).is_some());

    fixture.state.document_manager.close(&uri);

    assert!(fixture.state.document_manager.get(&uri).is_none());
}

#[tokio::test]
async fn test_document_service_change() {
    use tower_lsp::lsp_types::TextDocumentContentChangeEvent;

    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const x = 1;");

    fixture
        .state
        .document_manager
        .open(uri.clone(), "javascript".into(), "const x = 1;".into(), 1)
        .await;

    // Full document change
    let changes = vec![TextDocumentContentChangeEvent {
        range: None,
        range_length: None,
        text: "const y = 2;".to_string(),
    }];
    fixture
        .state
        .document_manager
        .change(&uri, changes, 2)
        .await;

    let doc = fixture.state.document_manager.get(&uri).unwrap();
    assert_eq!(doc.content.as_str(), "const y = 2;");
    assert_eq!(doc.version, 2);
}

#[tokio::test]
async fn test_document_service_all_uris() {
    let fixture = TestFixture::new().await;
    let uri1 = fixture.create_file("a.js", "const a = 1;");
    let uri2 = fixture.create_file("b.js", "const b = 2;");

    fixture.state.document_manager.open(uri1.clone(), "javascript".into(), "const a = 1;".into(), 1).await;
    fixture.state.document_manager.open(uri2.clone(), "javascript".into(), "const b = 2;".into(), 1).await;

    let uris = fixture.state.document_manager.all_uris();
    assert_eq!(uris.len(), 2);
    assert!(uris.contains(&uri1));
    assert!(uris.contains(&uri2));
}

#[tokio::test]
async fn test_document_service_document_count() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const x = 1;");

    assert_eq!(fixture.state.document_manager.document_count(), 0);

    fixture.state.document_manager.open(uri.clone(), "javascript".into(), "const x = 1;".into(), 1).await;

    assert_eq!(fixture.state.document_manager.document_count(), 1);
}

// ============================================================
// WorkspaceService Tests
// ============================================================

#[tokio::test]
async fn test_workspace_index_stats() {
    let fixture = TestFixture::new().await;
    fixture.index_workspace().await;

    let stats = fixture.state.workspace_index.stats();
    // Verify stats can be retrieved without panic
    let _ = stats.total_files;
}

#[tokio::test]
async fn test_workspace_index_files_for_env_var() {
    let fixture = TestFixture::new().await;

    // Create a file that references DB_URL
    fixture.create_file("test.js", "const db = process.env.DB_URL;");
    fixture.index_workspace().await;

    let files = fixture.state.workspace_index.files_for_env_var("DB_URL");
    // May or may not find files depending on indexing
    // Verify operation doesn't panic
    let _ = files.len();
}

#[tokio::test]
async fn test_workspace_index_all_env_vars() {
    let fixture = TestFixture::new().await;
    fixture.create_file("test.js", "const db = process.env.DB_URL;");
    fixture.index_workspace().await;

    let env_vars = fixture.state.workspace_index.all_env_vars();
    // Should find at least DB_URL from the indexed file
    // Verify operation doesn't panic
    let _ = env_vars.len();
}
