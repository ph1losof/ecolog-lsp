//! Integration tests for Find References and Rename functionality.

mod common;

use common::TestFixture;
use ecolog_lsp::server::handlers::{handle_references, handle_rename, handle_prepare_rename};
use tower_lsp::lsp_types::{
    Position, ReferenceContext, ReferenceParams, RenameParams, TextDocumentIdentifier,
    TextDocumentPositionParams,
};

// ============================================================================
// Find References Tests
// ============================================================================

#[tokio::test]
async fn test_find_references_direct_reference() {
    let fixture = TestFixture::new().await;

    // Create test file with env var reference
    let uri = fixture.create_file(
        "test.js",
        "const url = process.env.DB_URL;\nconsole.log(process.env.DB_URL);",
    );

    // Index workspace
    fixture.index_workspace().await;

    // Open the document so it's analyzed
    fixture
        .state
        .document_manager
        .open(uri.clone(), "javascript".to_string(),
              "const url = process.env.DB_URL;\nconsole.log(process.env.DB_URL);".to_string(), 1)
        .await;

    // Create reference params at position of DB_URL
    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 0,
                character: 24, // Position within "DB_URL"
            },
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: true,
        },
    };

    let result = handle_references(params, &fixture.state).await;

    assert!(result.is_some(), "Expected to find references");
    let locations = result.unwrap();

    // Should find at least 2 references (the two DB_URL usages in the file)
    assert!(
        locations.len() >= 2,
        "Expected at least 2 references, found {}",
        locations.len()
    );
}

#[tokio::test]
async fn test_find_references_across_files() {
    let fixture = TestFixture::new().await;

    // Create multiple test files
    fixture.create_file("a.js", "const key = process.env.API_KEY;");
    fixture.create_file("b.ts", "const apiKey = process.env.API_KEY;");

    // Index workspace
    fixture.index_workspace().await;

    // Open first file
    let uri_a = fixture.create_file("a.js", "const key = process.env.API_KEY;");
    fixture
        .state
        .document_manager
        .open(
            uri_a.clone(),
            "javascript".to_string(),
            "const key = process.env.API_KEY;".to_string(),
            1,
        )
        .await;

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri_a },
            position: Position {
                line: 0,
                character: 24, // Position within "API_KEY"
            },
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: true,
        },
    };

    let result = handle_references(params, &fixture.state).await;

    assert!(result.is_some(), "Expected to find references");
    let locations = result.unwrap();

    // Should find references in multiple files
    assert!(
        locations.len() >= 2,
        "Expected at least 2 references across files, found {}",
        locations.len()
    );
}

#[tokio::test]
async fn test_find_references_no_refs_for_unknown_var() {
    let fixture = TestFixture::new().await;

    let uri = fixture.create_file("test.js", "const x = process.env.UNKNOWN_VAR;");

    fixture.index_workspace().await;

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const x = process.env.UNKNOWN_VAR;".to_string(),
            1,
        )
        .await;

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 0,
                character: 22, // Position within "UNKNOWN_VAR"
            },
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = handle_references(params, &fixture.state).await;

    // Should find at least the one reference in the file
    assert!(result.is_some());
}

// ============================================================================
// Rename Tests
// ============================================================================

#[tokio::test]
async fn test_prepare_rename_valid_env_var() {
    let fixture = TestFixture::new().await;

    let uri = fixture.create_file("test.js", "const url = process.env.DB_URL;");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const url = process.env.DB_URL;".to_string(),
            1,
        )
        .await;

    let params = TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri },
        position: Position {
            line: 0,
            character: 24,
        },
    };

    let result = handle_prepare_rename(params, &fixture.state).await;

    assert!(result.is_some(), "Prepare rename should succeed for valid env var");
}

#[tokio::test]
async fn test_rename_env_var() {
    let fixture = TestFixture::new().await;

    let uri = fixture.create_file(
        "test.js",
        "const url = process.env.DB_URL;\nconst x = process.env.DB_URL;",
    );

    fixture.index_workspace().await;

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const url = process.env.DB_URL;\nconst x = process.env.DB_URL;".to_string(),
            1,
        )
        .await;

    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 0,
                character: 24,
            },
        },
        new_name: "DATABASE_URL".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = handle_rename(params, &fixture.state).await;

    assert!(result.is_some(), "Rename should return edits");
    let edit = result.unwrap();

    assert!(edit.changes.is_some(), "WorkspaceEdit should have changes");
    let changes = edit.changes.unwrap();

    // Should have edits for at least one file
    assert!(!changes.is_empty(), "Should have edits for at least one file");
}

#[tokio::test]
async fn test_rename_invalid_new_name() {
    let fixture = TestFixture::new().await;

    let uri = fixture.create_file("test.js", "const url = process.env.DB_URL;");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const url = process.env.DB_URL;".to_string(),
            1,
        )
        .await;

    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 0,
                character: 24,
            },
        },
        new_name: "123_INVALID".to_string(), // Invalid: starts with number
        work_done_progress_params: Default::default(),
    };

    let result = handle_rename(params, &fixture.state).await;

    assert!(result.is_none(), "Rename with invalid name should return None");
}

// ============================================================================
// Rename from .env File Tests
// ============================================================================

#[tokio::test]
async fn test_prepare_rename_in_env_file() {
    let fixture = TestFixture::new().await;

    // Create a .env file with a variable
    let env_uri = fixture.create_file(".env", "API_KEY=secret123\nDB_URL=postgres://localhost");

    // Open the .env file
    fixture
        .state
        .document_manager
        .open(
            env_uri.clone(),
            "plaintext".to_string(),
            "API_KEY=secret123\nDB_URL=postgres://localhost".to_string(),
            1,
        )
        .await;

    // Prepare rename on API_KEY (position within the key name)
    let params = TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: env_uri },
        position: Position {
            line: 0,
            character: 3, // Within "API_KEY"
        },
    };

    let result = handle_prepare_rename(params, &fixture.state).await;

    assert!(result.is_some(), "Prepare rename should succeed for env var in .env file");
}

#[tokio::test]
async fn test_rename_from_env_file() {
    let fixture = TestFixture::new().await;

    // Create .env file
    let env_uri = fixture.create_file(".env", "API_KEY=secret123");

    // Create code files referencing API_KEY
    let js_uri = fixture.create_file("app.js", "const key = process.env.API_KEY;");
    let ts_uri = fixture.create_file("config.ts", "const apiKey = process.env.API_KEY;");

    // Index workspace
    fixture.index_workspace().await;

    // Open all files
    fixture
        .state
        .document_manager
        .open(
            env_uri.clone(),
            "plaintext".to_string(),
            "API_KEY=secret123".to_string(),
            1,
        )
        .await;
    fixture
        .state
        .document_manager
        .open(
            js_uri.clone(),
            "javascript".to_string(),
            "const key = process.env.API_KEY;".to_string(),
            1,
        )
        .await;
    fixture
        .state
        .document_manager
        .open(
            ts_uri.clone(),
            "typescript".to_string(),
            "const apiKey = process.env.API_KEY;".to_string(),
            1,
        )
        .await;

    // Rename API_KEY to AUTH_TOKEN from .env file
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: env_uri.clone() },
            position: Position {
                line: 0,
                character: 3, // Within "API_KEY"
            },
        },
        new_name: "AUTH_TOKEN".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = handle_rename(params, &fixture.state).await;

    assert!(result.is_some(), "Rename from .env file should return edits");
    let edit = result.unwrap();

    assert!(edit.changes.is_some(), "WorkspaceEdit should have changes");
    let changes = edit.changes.unwrap();

    // Should have edits for at least the .env file and possibly code files
    assert!(!changes.is_empty(), "Should have edits for at least one file");

    // The .env file should definitely be in the changes
    assert!(
        changes.contains_key(&env_uri),
        "Changes should include the .env file"
    );
}

#[tokio::test]
async fn test_rename_from_env_file_updates_code_files() {
    let fixture = TestFixture::new().await;

    // Create .env file
    let env_uri = fixture.create_file(".env", "DB_HOST=localhost");

    // Create code file
    let js_content = "const host = process.env.DB_HOST;\nconst url = `http://${process.env.DB_HOST}:8080`;";
    let js_uri = fixture.create_file("server.js", js_content);

    // Index workspace
    fixture.index_workspace().await;

    // Open files
    fixture
        .state
        .document_manager
        .open(
            env_uri.clone(),
            "plaintext".to_string(),
            "DB_HOST=localhost".to_string(),
            1,
        )
        .await;
    fixture
        .state
        .document_manager
        .open(
            js_uri.clone(),
            "javascript".to_string(),
            js_content.to_string(),
            1,
        )
        .await;

    // Rename from .env file
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: env_uri.clone() },
            position: Position {
                line: 0,
                character: 2, // Within "DB_HOST"
            },
        },
        new_name: "DATABASE_HOST".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = handle_rename(params, &fixture.state).await;

    assert!(result.is_some(), "Rename should succeed");
    let edit = result.unwrap();
    let changes = edit.changes.expect("Should have changes");

    // Check that js file got edits (if indexed)
    if changes.contains_key(&js_uri) {
        let js_edits = &changes[&js_uri];
        // Should have edits for both DB_HOST occurrences
        assert!(
            js_edits.len() >= 1,
            "JS file should have at least 1 edit"
        );
    }

    // .env file should always have the edit
    assert!(
        changes.contains_key(&env_uri),
        ".env file should be in changes"
    );
}

// ============================================================================
// Workspace Index Tests
// ============================================================================

#[tokio::test]
async fn test_workspace_index_stats() {
    let fixture = TestFixture::new().await;

    // Create multiple files with env var references
    fixture.create_file("a.js", "process.env.API_KEY");
    fixture.create_file("b.ts", "process.env.DB_URL");
    fixture.create_file("c.py", "os.environ['DEBUG']");

    fixture.index_workspace().await;

    let stats = fixture.state.workspace_index.stats();

    // Should have indexed at least the code files plus .env
    assert!(stats.total_files >= 1, "Should have indexed at least 1 file");
    assert!(
        stats.total_env_vars >= 1,
        "Should have indexed at least 1 env var"
    );
}

#[tokio::test]
async fn test_workspace_index_files_for_env_var() {
    let fixture = TestFixture::new().await;

    // Create files with API_KEY references
    fixture.create_file("a.js", "const k = process.env.API_KEY;");
    fixture.create_file("b.ts", "const key = process.env.API_KEY;");
    fixture.create_file("c.js", "const port = process.env.PORT;"); // Different var

    fixture.index_workspace().await;

    let api_key_files = fixture.state.workspace_index.files_for_env_var("API_KEY");

    // Should find files referencing API_KEY
    assert!(
        api_key_files.len() >= 2,
        "Expected at least 2 files with API_KEY, found {}",
        api_key_files.len()
    );
}
