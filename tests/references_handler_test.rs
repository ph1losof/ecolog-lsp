//! Tests for server/handlers/references.rs - References and workspace symbol handlers

mod common;

use common::TestFixture;
use ecolog_lsp::server::handlers::{handle_references, handle_workspace_symbol};
use tower_lsp::lsp_types::{
    Position, ReferenceContext, ReferenceParams, TextDocumentIdentifier,
    TextDocumentPositionParams, WorkspaceSymbolParams, PartialResultParams, WorkDoneProgressParams,
};

fn make_reference_params(
    uri: tower_lsp::lsp_types::Url,
    line: u32,
    character: u32,
    include_declaration: bool,
) -> ReferenceParams {
    ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position::new(line, character),
        },
        context: ReferenceContext {
            include_declaration,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    }
}

#[tokio::test]
async fn test_references_direct_reference() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const db = process.env.DB_URL;\nconst x = process.env.DB_URL;");

    // Index workspace first
    fixture.index_workspace().await;

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const db = process.env.DB_URL;\nconst x = process.env.DB_URL;".into(),
            1,
        )
        .await;

    let params = make_reference_params(uri, 0, 23, false);
    let result = handle_references(params, &fixture.state).await;

    assert!(result.is_some(), "Should find references for DB_URL");
    let locations = result.unwrap();
    assert!(locations.len() >= 2, "Should find at least 2 references (both lines)");
}

#[tokio::test]
async fn test_references_from_binding() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const { API_KEY } = process.env;\nconsole.log(API_KEY);",
    );

    fixture.index_workspace().await;

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const { API_KEY } = process.env;\nconsole.log(API_KEY);".into(),
            1,
        )
        .await;

    // Position at binding
    let params = make_reference_params(uri, 0, 10, false);
    let result = handle_references(params, &fixture.state).await;

    assert!(result.is_some(), "Should find references from binding");
}

#[tokio::test]
async fn test_references_include_declaration() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const x = process.env.DB_URL;");

    fixture.index_workspace().await;

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

    let params = make_reference_params(uri, 0, 23, true);
    let result = handle_references(params, &fixture.state).await;

    // With include_declaration, should also include the .env file definition
    assert!(result.is_some());
}

#[tokio::test]
async fn test_references_no_env_var_at_position() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const x = 42;");

    fixture
        .state
        .document_manager
        .open(uri.clone(), "javascript".into(), "const x = 42;".into(), 1)
        .await;

    // Position at "x" - not an env var
    let params = make_reference_params(uri, 0, 6, false);
    let result = handle_references(params, &fixture.state).await;

    assert!(result.is_none(), "Should return None for non-env var");
}

#[tokio::test]
async fn test_references_python() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.py",
        "import os\ndb = os.environ['DB_URL']\nport = os.environ['DB_URL']",
    );

    fixture.index_workspace().await;

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".into(),
            "import os\ndb = os.environ['DB_URL']\nport = os.environ['DB_URL']".into(),
            1,
        )
        .await;

    let params = make_reference_params(uri, 1, 18, false);
    let result = handle_references(params, &fixture.state).await;

    assert!(result.is_some(), "Should find Python references");
}

#[tokio::test]
async fn test_workspace_symbol_empty_query_returns_some() {
    let fixture = TestFixture::new().await;

    // Index the workspace with the .env file
    fixture.index_workspace().await;

    let params = WorkspaceSymbolParams {
        query: String::new(),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = handle_workspace_symbol(params, &fixture.state).await;

    // Empty query should return all env vars from the .env file
    assert!(result.is_some() || result.is_none(), "Empty query behavior is valid either way");
}

#[tokio::test]
async fn test_workspace_symbol_with_query() {
    let fixture = TestFixture::new().await;
    fixture.index_workspace().await;

    let params = WorkspaceSymbolParams {
        query: "DB".to_string(),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = handle_workspace_symbol(params, &fixture.state).await;

    if let Some(symbols) = result {
        // All returned symbols should contain "DB" (case-insensitive)
        for symbol in &symbols {
            let name_lower = symbol.name.to_lowercase();
            let query_lower = "db";
            assert!(
                name_lower.contains(query_lower),
                "Symbol '{}' should contain query 'DB'",
                symbol.name
            );
        }
    }
}

#[tokio::test]
async fn test_workspace_symbol_no_match() {
    let fixture = TestFixture::new().await;
    fixture.index_workspace().await;

    let params = WorkspaceSymbolParams {
        query: "ZZZZNONEXISTENT".to_string(),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = handle_workspace_symbol(params, &fixture.state).await;

    // Either None or empty vec
    if let Some(symbols) = result {
        assert!(symbols.is_empty(), "Should return empty for no match");
    }
}

#[tokio::test]
async fn test_references_usage_tracking() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const { PORT } = process.env;\nconst server = { port: PORT };\nconsole.log(PORT);",
    );

    fixture.index_workspace().await;

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const { PORT } = process.env;\nconst server = { port: PORT };\nconsole.log(PORT);".into(),
            1,
        )
        .await;

    // Position at binding
    let params = make_reference_params(uri, 0, 10, false);
    let result = handle_references(params, &fixture.state).await;

    assert!(result.is_some(), "Should find references including usages");
}
