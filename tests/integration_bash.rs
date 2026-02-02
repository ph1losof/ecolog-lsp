mod common;
use common::TestFixture;
use ecolog_lsp::server::handlers::{compute_diagnostics, handle_hover};
use tower_lsp::lsp_types::{
    HoverParams, Position, TextDocumentIdentifier, TextDocumentPositionParams,
};

#[tokio::test]
async fn test_bash_hover_simple_expansion() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("script.sh", r#"echo $DB_URL"#);

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "shellscript".to_string(),
            r#"echo $DB_URL"#.to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 8),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover for $DB_URL");
    assert!(format!("{:?}", hover.unwrap()).contains("postgres://"));
}

#[tokio::test]
async fn test_bash_hover_brace_expansion() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("script.sh", r#"echo ${API_KEY}"#);

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "shellscript".to_string(),
            r#"echo ${API_KEY}"#.to_string(),
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

    assert!(hover.is_some(), "Expected hover for ${{API_KEY}}");
    assert!(format!("{:?}", hover.unwrap()).contains("secret_key"));
}

#[tokio::test]
async fn test_bash_hover_default_value() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("script.sh", r#"echo ${DEBUG:-false}"#);

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "shellscript".to_string(),
            r#"echo ${DEBUG:-false}"#.to_string(),
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

    assert!(hover.is_some(), "Expected hover for ${{DEBUG:-false}}");
    assert!(format!("{:?}", hover.unwrap()).contains("true"));
}

#[tokio::test]
async fn test_bash_diagnostics_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("script.sh", r#"echo $UNDEFINED_VAR"#);

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "shellscript".to_string(),
            r#"echo $UNDEFINED_VAR"#.to_string(),
            0,
        )
        .await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;

    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("not defined")));
}

#[tokio::test]
async fn test_bash_multiple_expansions() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("script.sh", r#"echo $DB_URL ${API_KEY}"#);

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "shellscript".to_string(),
            r#"echo $DB_URL ${API_KEY}"#.to_string(),
            0,
        )
        .await;

    // Check first expansion
    let hover1 = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(0, 8),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover1.is_some(), "Expected hover for $DB_URL");
    assert!(format!("{:?}", hover1.unwrap()).contains("postgres://"));

    // Check second expansion
    let hover2 = handle_hover(
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

    assert!(hover2.is_some(), "Expected hover for ${{API_KEY}}");
    assert!(format!("{:?}", hover2.unwrap()).contains("secret_key"));
}
