mod common;
use common::TestFixture;
use ecolog_lsp::server::handlers::{compute_diagnostics, handle_completion, handle_hover};
use tower_lsp::lsp_types::{
    HoverParams, Position, TextDocumentIdentifier, TextDocumentPositionParams,
};

#[tokio::test]
async fn test_go_hover_getenv() {
    let fixture = TestFixture::new().await;
    // Changed " _ =" to "val :=" just in case assignment vs short decl matters,
    // though it shouldn't. Also doubled check position calculation.
    // "  val := os.Getenv(\"DB_URL\")"
    // 01234567890123456789012345678
    // "  " (2) + "val" (3) + " " (1) + ":=" (2) + " " (1) = 9
    // "os." (3) -> 12. "Getenv" (6) -> 18. "(" (1) -> 19.
    // "\"" (1) -> 20. "D" -> 21. "B" -> 22.
    // Position 22 should be safe.
    let content = "package main\nimport \"os\"\nfunc main() {\n  val := os.Getenv(\"DB_URL\")\n}";
    let uri = fixture.create_file("test.go", content);

    fixture
        .state
        .document_manager
        .open(uri.clone(), "go".to_string(), content.to_string(), 0)
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(3, 22),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Hover failed for os.Getenv");
    assert!(format!("{:?}", hover.unwrap()).contains("postgres://"));
}

#[tokio::test]
async fn test_go_hover_lookupenv() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.go",
        "package main\nimport \"os\"\nfunc main() {\n  val, _ := os.LookupEnv(\"API_KEY\")\n}",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "go".to_string(),
            "package main\nimport \"os\"\nfunc main() {\n  val, _ := os.LookupEnv(\"API_KEY\")\n}"
                .to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(3, 27),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Hover failed for os.LookupEnv");
    assert!(format!("{:?}", hover.unwrap()).contains("secret_key"));
}

#[tokio::test]
async fn test_go_completion() {
    let fixture = TestFixture::new().await;

    // Small delay to avoid potential race conditions with tree-sitter initialization
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let content = "package main\nimport \"os\"\nfunc main() {\n  os.LookupEnv(\"\")\n}";
    let uri = fixture.create_file("test.go", content);

    fixture
        .state
        .document_manager
        .open(uri.clone(), "go".to_string(), content.to_string(), 0)
        .await;

    // Position inside quotes.
    // "  os.LookupEnv(\"\")"
    // "  " (2) + "os." (3) = 5. "LookupEnv" (9) = 14. "(" (1) = 15. "\"" (1) = 16.
    // Inside quote 17.
    let completion = handle_completion(
        tower_lsp::lsp_types::CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(3, 17),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        },
        &fixture.state,
    )
    .await;

    assert!(completion.is_some(), "Completion failed for os.Getenv");
    assert!(completion.unwrap().iter().any(|i| i.label == "PORT"));
}
