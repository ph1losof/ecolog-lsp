mod common;
use common::TestFixture;
use ecolog_lsp::server::handlers::{compute_diagnostics, handle_completion, handle_hover};
use tower_lsp::lsp_types::{
    HoverParams, Position, TextDocumentIdentifier, TextDocumentPositionParams,
};

#[tokio::test]
async fn test_go_hover_getenv() {
    let fixture = TestFixture::new().await;
    
    
    
    
    
    
    
    
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
    assert!(format!("{:?}", hover.unwrap()).contains("postgres:
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

    
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let content = "package main\nimport \"os\"\nfunc main() {\n  os.LookupEnv(\"\")\n}";
    let uri = fixture.create_file("test.go", content);

    fixture
        .state
        .document_manager
        .open(uri.clone(), "go".to_string(), content.to_string(), 0)
        .await;

    
    
    
    
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
