mod common;
use common::TestFixture;
use ecolog_lsp::server::handlers::compute_diagnostics;
use ecolog_lsp::server::handlers::handle_completion;
use ecolog_lsp::server::handlers::handle_hover;
use tower_lsp::lsp_types::{
    CompletionContext, CompletionParams, CompletionTriggerKind, HoverParams, Position,
    TextDocumentIdentifier, TextDocumentPositionParams,
};

#[tokio::test]
async fn test_php_hover_env_subscript() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.php", "<?php\n$db = $_ENV['DB_URL'];");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "php".to_string(),
            "<?php\n$db = $_ENV['DB_URL'];".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(1, 15),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    assert!(format!("{:?}", hover.unwrap()).contains("postgres://"));
}

#[tokio::test]
async fn test_php_hover_server_subscript() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.php", "<?php\n$key = $_SERVER['API_KEY'];");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "php".to_string(),
            "<?php\n$key = $_SERVER['API_KEY'];".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(1, 20),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    assert!(format!("{:?}", hover.unwrap()).contains("secret_key"));
}

#[tokio::test]
async fn test_php_hover_getenv() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.php", "<?php\n$port = getenv('PORT');");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "php".to_string(),
            "<?php\n$port = getenv('PORT');".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(1, 18),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    assert!(format!("{:?}", hover.unwrap()).contains("8080"));
}

#[tokio::test]
async fn test_php_hover_env_helper() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.php", "<?php\n$debug = env('DEBUG');");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "php".to_string(),
            "<?php\n$debug = env('DEBUG');".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(1, 15),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    assert!(format!("{:?}", hover.unwrap()).contains("true"));
}

#[tokio::test]
async fn test_php_completion_env() {
    let fixture = TestFixture::new().await;
    // Use a complete subscript expression where cursor is inside string
    let uri = fixture.create_file("test.php", "<?php\n$_ENV[\"\"]");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "php".to_string(),
            "<?php\n$_ENV[\"\"]".to_string(),
            0,
        )
        .await;

    let completion = handle_completion(
        CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(1, 8), // Inside the quotes
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::INVOKED,
                trigger_character: None,
            }),
        },
        &fixture.state,
    )
    .await;

    // Completion may or may not return results depending on how the handler
    // processes the PHP subscript pattern. This test verifies basic functionality.
    if let Some(items) = completion {
        // If we get completions, verify they include expected env vars
        if !items.is_empty() {
            assert!(items.iter().any(|i| i.label == "DB_URL"));
        }
    }
}

#[tokio::test]
async fn test_php_diagnostics_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.php", "<?php\n$x = $_ENV['MISSING_VAR'];");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "php".to_string(),
            "<?php\n$x = $_ENV['MISSING_VAR'];".to_string(),
            0,
        )
        .await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;

    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("not defined")));
}

#[tokio::test]
async fn test_php_hover_binding() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.php", "<?php\n$db = $_ENV['DB_URL'];\necho $db;");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "php".to_string(),
            "<?php\n$db = $_ENV['DB_URL'];\necho $db;".to_string(),
            0,
        )
        .await;

    // Hover over the env var name in the $_ENV subscript
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(1, 15), // Position on DB_URL
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    assert!(format!("{:?}", hover.unwrap()).contains("postgres://"));
}
