mod common;
use common::TestFixture;
use ecolog_lsp::server::handlers::{compute_diagnostics, handle_hover};
use tower_lsp::lsp_types::{
    HoverParams, Position, TextDocumentIdentifier, TextDocumentPositionParams,
};

#[tokio::test]
async fn test_java_hover_system_getenv() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("Main.java", r#"System.getenv("DB_URL")"#);

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "java".to_string(),
            r#"System.getenv("DB_URL")"#.to_string(),
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

    assert!(hover.is_some(), "Expected hover for System.getenv");
    assert!(format!("{:?}", hover.unwrap()).contains("postgres://"));
}

#[tokio::test]
async fn test_java_hover_system_getproperty() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("Main.java", r#"System.getProperty("API_KEY")"#);

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "java".to_string(),
            r#"System.getProperty("API_KEY")"#.to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 22),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover for System.getProperty");
    assert!(format!("{:?}", hover.unwrap()).contains("secret_key"));
}

#[tokio::test]
async fn test_java_diagnostics_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("Main.java", r#"System.getenv("UNDEFINED_VAR")"#);

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "java".to_string(),
            r#"System.getenv("UNDEFINED_VAR")"#.to_string(),
            0,
        )
        .await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;

    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("not defined")));
}

// Note: Completion tests skipped for now - completion context queries need refinement
