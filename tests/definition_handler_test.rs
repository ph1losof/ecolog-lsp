//! Tests for server/handlers/definition.rs - Go-to-definition handler

mod common;

use common::TestFixture;
use ecolog_lsp::server::handlers::handle_definition;
use tower_lsp::lsp_types::{
    GotoDefinitionParams, GotoDefinitionResponse, Position, TextDocumentIdentifier,
    TextDocumentPositionParams,
};

fn make_params(uri: tower_lsp::lsp_types::Url, line: u32, character: u32) -> GotoDefinitionParams {
    GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position::new(line, character),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    }
}

#[tokio::test]
async fn test_definition_direct_reference_to_env_file() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const db = process.env.DB_URL;");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const db = process.env.DB_URL;".into(),
            1,
        )
        .await;

    let params = make_params(uri, 0, 23);
    let result = handle_definition(params, &fixture.state).await;

    assert!(result.is_some(), "Should find definition for DB_URL");
    if let Some(GotoDefinitionResponse::Scalar(location)) = result {
        assert!(
            location.uri.path().ends_with(".env"),
            "Definition should point to .env file"
        );
    }
}

#[tokio::test]
async fn test_definition_from_binding() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const { API_KEY } = process.env;\nconsole.log(API_KEY);",
    );

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

    // Position at the destructured binding
    let params = make_params(uri, 0, 10);
    let result = handle_definition(params, &fixture.state).await;

    assert!(result.is_some(), "Should find definition from binding");
    if let Some(GotoDefinitionResponse::Scalar(location)) = result {
        assert!(
            location.uri.path().ends_with(".env"),
            "Definition should point to .env file"
        );
    }
}

#[tokio::test]
async fn test_definition_from_usage() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const { DEBUG } = process.env;\nconsole.log(DEBUG);",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const { DEBUG } = process.env;\nconsole.log(DEBUG);".into(),
            1,
        )
        .await;

    // Position at the usage (line 1, character 12)
    let params = make_params(uri, 1, 12);
    let result = handle_definition(params, &fixture.state).await;

    assert!(result.is_some(), "Should find definition from usage");
}

#[tokio::test]
async fn test_definition_undefined_var_returns_none() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const x = process.env.UNDEFINED_VAR;");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const x = process.env.UNDEFINED_VAR;".into(),
            1,
        )
        .await;

    let params = make_params(uri, 0, 22);
    let result = handle_definition(params, &fixture.state).await;

    assert!(
        result.is_none(),
        "Should return None for undefined variable"
    );
}

#[tokio::test]
async fn test_definition_outside_env_returns_none() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const x = 42;");

    fixture
        .state
        .document_manager
        .open(uri.clone(), "javascript".into(), "const x = 42;".into(), 1)
        .await;

    // Position at "x" - not an env var
    let params = make_params(uri, 0, 6);
    let result = handle_definition(params, &fixture.state).await;

    assert!(
        result.is_none(),
        "Should return None for non-env var position"
    );
}

#[tokio::test]
async fn test_definition_python_environ() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.py",
        "import os\nport = os.environ['PORT']",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".into(),
            "import os\nport = os.environ['PORT']".into(),
            1,
        )
        .await;

    // Position at "PORT"
    let params = make_params(uri, 1, 20);
    let result = handle_definition(params, &fixture.state).await;

    assert!(result.is_some(), "Should find definition for Python environ");
}

#[tokio::test]
async fn test_definition_rust_std_env() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.rs",
        "fn main() { let port = std::env::var(\"PORT\"); }",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "rust".into(),
            "fn main() { let port = std::env::var(\"PORT\"); }".into(),
            1,
        )
        .await;

    // Position at "PORT"
    let params = make_params(uri, 0, 38);
    let result = handle_definition(params, &fixture.state).await;

    assert!(result.is_some(), "Should find definition for Rust std::env::var");
}

#[tokio::test]
async fn test_definition_go_getenv() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.go",
        "package main\nimport \"os\"\nfunc main() { port := os.Getenv(\"PORT\") }",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "go".into(),
            "package main\nimport \"os\"\nfunc main() { port := os.Getenv(\"PORT\") }".into(),
            1,
        )
        .await;

    // Position at "PORT"
    let params = make_params(uri, 2, 34);
    let result = handle_definition(params, &fixture.state).await;

    assert!(result.is_some(), "Should find definition for Go os.Getenv");
}

#[tokio::test]
async fn test_definition_range_points_to_key() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const db = process.env.DB_URL;");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const db = process.env.DB_URL;".into(),
            1,
        )
        .await;

    let params = make_params(uri, 0, 23);
    let result = handle_definition(params, &fixture.state).await;

    assert!(result.is_some());
    if let Some(GotoDefinitionResponse::Scalar(location)) = result {
        // The range should start at column 0 (where DB_URL key starts in .env file)
        assert_eq!(location.range.start.line, 0, "DB_URL is on first line of .env");
    }
}

#[tokio::test]
async fn test_definition_bracket_notation() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const env = process.env;\nconst db = env['DB_URL'];",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const env = process.env;\nconst db = env['DB_URL'];".into(),
            1,
        )
        .await;

    // Position at "DB_URL" in bracket notation
    let params = make_params(uri, 1, 17);
    let result = handle_definition(params, &fixture.state).await;

    assert!(result.is_some(), "Should find definition via bracket notation");
}
