mod common;
use common::TestFixture;
use ecolog_lsp::server::handlers::{compute_diagnostics, handle_hover};
use tower_lsp::lsp_types::{
    HoverParams, Position, TextDocumentIdentifier, TextDocumentPositionParams,
};

#[tokio::test]
async fn test_elixir_hover_system_get_env() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("lib/app.ex", r#"System.get_env("DB_URL")"#);

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "elixir".to_string(),
            r#"System.get_env("DB_URL")"#.to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 18),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover for System.get_env");
    assert!(format!("{:?}", hover.unwrap()).contains("postgres://"));
}

#[tokio::test]
async fn test_elixir_hover_system_fetch_env() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("lib/app.ex", r#"System.fetch_env("API_KEY")"#);

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "elixir".to_string(),
            r#"System.fetch_env("API_KEY")"#.to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 20),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover for System.fetch_env");
    assert!(format!("{:?}", hover.unwrap()).contains("secret_key"));
}

#[tokio::test]
async fn test_elixir_hover_system_fetch_env_bang() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("lib/app.ex", r#"System.fetch_env!("DEBUG")"#);

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "elixir".to_string(),
            r#"System.fetch_env!("DEBUG")"#.to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 21),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover for System.fetch_env!");
    assert!(format!("{:?}", hover.unwrap()).contains("true"));
}

#[tokio::test]
async fn test_elixir_diagnostics_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("lib/app.ex", r#"System.get_env("UNDEFINED_VAR")"#);

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "elixir".to_string(),
            r#"System.get_env("UNDEFINED_VAR")"#.to_string(),
            0,
        )
        .await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;

    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("not defined")));
}

// Note: Completion tests skipped for now - completion context queries need refinement
