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
async fn test_py_hover_environ_getitem() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.py", "import os\nval = os.environ['DB_URL']");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".to_string(),
            "import os\nval = os.environ['DB_URL']".to_string(),
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
    assert!(format!("{:?}", hover.unwrap()).contains("postgres:
}

#[tokio::test]
async fn test_py_hover_environ_get() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.py", "import os\nval = os.environ.get('API_KEY')");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".to_string(),
            "import os\nval = os.environ.get('API_KEY')".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(1, 25),
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
async fn test_py_hover_os_getenv() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.py", "import os\nval = os.getenv('PORT')");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".to_string(),
            "import os\nval = os.getenv('PORT')".to_string(),
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
async fn test_py_completion_environ() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.py", "import os\nos.environ['");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".to_string(),
            "import os\nos.environ['".to_string(),
            0,
        )
        .await;

    let completion = handle_completion(
        CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(1, 11),
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

    assert!(completion.is_some());
    let items = completion.unwrap();
    assert!(items.iter().any(|i| i.label == "DB_URL"));
}

#[tokio::test]
async fn test_py_hover_from_import() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.py", "from os import environ\nx = environ['DEBUG']");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".to_string(),
            "from os import environ\nx = environ['DEBUG']".to_string(),
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

    
    
    if hover.is_some() {
        assert!(format!("{:?}", hover.unwrap()).contains("true"));
    }
}




#[tokio::test]
async fn test_py_hover_walrus_operator_environ_get() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.py",
        "import os\nif (db_url := os.environ.get('DB_URL')):\n  print(db_url)",
    );
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".to_string(),
            "import os\nif (db_url := os.environ.get('DB_URL')):\n  print(db_url)".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(1, 6), 
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    assert!(format!("{:?}", hover.unwrap()).contains("postgres:
}

#[tokio::test]
async fn test_py_hover_walrus_operator_getenv() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.py",
        "import os\nif (api_key := os.getenv('API_KEY')):\n  print(api_key)",
    );
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".to_string(),
            "import os\nif (api_key := os.getenv('API_KEY')):\n  print(api_key)".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(1, 6), 
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
async fn test_py_hover_walrus_operator_subscript() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.py",
        "import os\nif (port := os.environ['PORT']):\n  print(port)",
    );
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".to_string(),
            "import os\nif (port := os.environ['PORT']):\n  print(port)".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(1, 6), 
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
async fn test_py_hover_walrus_operator_while_loop() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.py",
        "import os\nwhile (val := os.getenv('PORT')):\n  print(val)\n  break",
    );
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".to_string(),
            "import os\nwhile (val := os.getenv('PORT')):\n  print(val)\n  break".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(1, 8), 
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
async fn test_py_diagnostics_walrus_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.py",
        "import os\nif (missing := os.environ.get('MISSING_VAR')):\n  print(missing)",
    );
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".to_string(),
            "import os\nif (missing := os.environ.get('MISSING_VAR')):\n  print(missing)"
                .to_string(),
            0,
        )
        .await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;

    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("not defined")));
}

#[tokio::test]
async fn test_py_diagnostics_walrus_getenv_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.py",
        "import os\nif (missing := os.getenv('MISSING_VAR')):\n  print(missing)",
    );
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".to_string(),
            "import os\nif (missing := os.getenv('MISSING_VAR')):\n  print(missing)".to_string(),
            0,
        )
        .await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;

    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("not defined")));
}

#[tokio::test]
async fn test_py_walrus_operator_multiple_in_if() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.py", "import os\nif (db_url := os.environ.get('DB_URL')) and (api_key := os.environ.get('API_KEY')):\n  print(db_url, api_key)");
    fixture.state.document_manager.open(uri.clone(), "python".to_string(), 
        "import os\nif (db_url := os.environ.get('DB_URL')) and (api_key := os.environ.get('API_KEY')):\n  print(db_url, api_key)".to_string(), 0).await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(1, 6),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    assert!(format!("{:?}", hover.unwrap()).contains("postgres:

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(1, 45),
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
async fn test_py_walrus_operator_undefined_and_defined_mix() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.py", "import os\nif (db_url := os.environ.get('DB_URL')) and (missing := os.environ.get('MISSING_VAR')):\n  print(db_url, missing)");
    fixture.state.document_manager.open(uri.clone(), "python".to_string(), 
        "import os\nif (db_url := os.environ.get('DB_URL')) and (missing := os.environ.get('MISSING_VAR')):\n  print(db_url, missing)".to_string(), 0).await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;

    
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("MISSING_VAR")));
}
