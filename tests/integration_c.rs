mod common;
use common::TestFixture;
use ecolog_lsp::server::handlers::{compute_diagnostics, handle_hover};
use tower_lsp::lsp_types::{
    HoverParams, Position, TextDocumentIdentifier, TextDocumentPositionParams,
};

#[tokio::test]
async fn test_c_hover_getenv() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("main.c", r#"getenv("DB_URL")"#);

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "c".to_string(),
            r#"getenv("DB_URL")"#.to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 9),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover for getenv");
    assert!(format!("{:?}", hover.unwrap()).contains("postgres://"));
}

#[tokio::test]
async fn test_c_hover_secure_getenv() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("main.c", r#"secure_getenv("API_KEY")"#);

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "c".to_string(),
            r#"secure_getenv("API_KEY")"#.to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 16),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover for secure_getenv");
    assert!(format!("{:?}", hover.unwrap()).contains("secret_key"));
}

#[tokio::test]
async fn test_c_diagnostics_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("main.c", r#"getenv("UNDEFINED_VAR")"#);

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "c".to_string(),
            r#"getenv("UNDEFINED_VAR")"#.to_string(),
            0,
        )
        .await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;

    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("not defined")));
}

// Note: Completion tests skipped for now - completion context queries need refinement
