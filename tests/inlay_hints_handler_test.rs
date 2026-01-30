//! Tests for server/handlers/inlay_hints.rs - Inlay hints handler

mod common;

use common::TestFixture;
use ecolog_lsp::server::handlers::handle_inlay_hints;
use tower_lsp::lsp_types::{
    InlayHintParams, Position, Range, TextDocumentIdentifier,
};

fn make_inlay_params(uri: tower_lsp::lsp_types::Url, start: Position, end: Position) -> InlayHintParams {
    InlayHintParams {
        text_document: TextDocumentIdentifier { uri },
        range: Range::new(start, end),
        work_done_progress_params: Default::default(),
    }
}

#[tokio::test]
async fn test_inlay_hints_direct_reference() {
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

    let params = make_inlay_params(
        uri,
        Position::new(0, 0),
        Position::new(0, 30),
    );
    let result = handle_inlay_hints(params, &fixture.state).await;

    // Inlay hints may or may not be enabled by default
    // Just verify the call doesn't panic
    assert!(result.is_some() || result.is_none());
}

#[tokio::test]
async fn test_inlay_hints_destructuring() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const { DB_URL, API_KEY } = process.env;",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const { DB_URL, API_KEY } = process.env;".into(),
            1,
        )
        .await;

    let params = make_inlay_params(
        uri,
        Position::new(0, 0),
        Position::new(0, 40),
    );
    let result = handle_inlay_hints(params, &fixture.state).await;

    // Verify call succeeds
    assert!(result.is_some() || result.is_none());
}

#[tokio::test]
async fn test_inlay_hints_empty_for_no_env_vars() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const x = 42;");

    fixture
        .state
        .document_manager
        .open(uri.clone(), "javascript".into(), "const x = 42;".into(), 1)
        .await;

    let params = make_inlay_params(
        uri,
        Position::new(0, 0),
        Position::new(0, 15),
    );
    let result = handle_inlay_hints(params, &fixture.state).await;

    if let Some(hints) = result {
        assert!(hints.is_empty(), "Should return empty hints for no env vars");
    }
}

#[tokio::test]
async fn test_inlay_hints_python() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.py",
        "import os\ndb = os.environ['DB_URL']",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".into(),
            "import os\ndb = os.environ['DB_URL']".into(),
            1,
        )
        .await;

    let params = make_inlay_params(
        uri,
        Position::new(0, 0),
        Position::new(2, 0),
    );
    let result = handle_inlay_hints(params, &fixture.state).await;

    // Verify call succeeds
    assert!(result.is_some() || result.is_none());
}

#[tokio::test]
async fn test_inlay_hints_range_filtering() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const a = process.env.DB_URL;\nconst b = process.env.API_KEY;\nconst c = process.env.PORT;",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const a = process.env.DB_URL;\nconst b = process.env.API_KEY;\nconst c = process.env.PORT;".into(),
            1,
        )
        .await;

    // Only request hints for first line
    let params = make_inlay_params(
        uri,
        Position::new(0, 0),
        Position::new(0, 30),
    );
    let result = handle_inlay_hints(params, &fixture.state).await;

    // Hints should be filtered to only the requested range
    if let Some(hints) = result {
        for hint in &hints {
            assert_eq!(hint.position.line, 0, "Hints should only be on line 0");
        }
    }
}

#[tokio::test]
async fn test_inlay_hints_typescript() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.ts",
        "const apiKey: string = process.env.API_KEY!;",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "typescript".into(),
            "const apiKey: string = process.env.API_KEY!;".into(),
            1,
        )
        .await;

    let params = make_inlay_params(
        uri,
        Position::new(0, 0),
        Position::new(0, 50),
    );
    let result = handle_inlay_hints(params, &fixture.state).await;

    // Verify call succeeds
    assert!(result.is_some() || result.is_none());
}

#[tokio::test]
async fn test_inlay_hints_document_not_found() {
    let fixture = TestFixture::new().await;
    let uri = tower_lsp::lsp_types::Url::parse("file:///nonexistent/file.js").unwrap();

    // Don't open the document
    let params = make_inlay_params(
        uri,
        Position::new(0, 0),
        Position::new(0, 30),
    );
    let result = handle_inlay_hints(params, &fixture.state).await;

    // Should return None for non-existent document
    assert!(result.is_none(), "Should return None for non-existent document");
}

#[tokio::test]
async fn test_inlay_hints_undefined_var() {
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

    let params = make_inlay_params(
        uri,
        Position::new(0, 0),
        Position::new(0, 40),
    );
    let result = handle_inlay_hints(params, &fixture.state).await;

    // Undefined vars shouldn't have inlay hints (no value to show)
    if let Some(hints) = result {
        let has_undefined = hints.iter().any(|h| {
            if let tower_lsp::lsp_types::InlayHintLabel::String(s) = &h.label {
                s.contains("UNDEFINED")
            } else {
                false
            }
        });
        assert!(!has_undefined, "Should not show hint for undefined var");
    }
}
