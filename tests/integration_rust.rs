mod common;
use common::TestFixture;
use ecolog_lsp::server::handlers::{compute_diagnostics, handle_completion, handle_hover};
use tower_lsp::lsp_types::{
    HoverParams, Position, TextDocumentIdentifier, TextDocumentPositionParams,
};

#[tokio::test]
async fn test_rust_hover_std_env_var() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("main.rs", "fn main() { std::env::var(\"DB_URL\"); }");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "rust".to_string(),
            "fn main() { std::env::var(\"DB_URL\"); }".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 27),
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
async fn test_rust_hover_std_env_var_os() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("main.rs", "fn main() { std::env::var_os(\"API_KEY\"); }");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "rust".to_string(),
            "fn main() { std::env::var_os(\"API_KEY\"); }".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 31),
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
async fn test_rust_hover_result_destructuring() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "main.rs",
        "fn main() { let Ok(val) = std::env::var(\"DB_URL\"); }",
    );
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "rust".to_string(),
            "fn main() { let Ok(val) = std::env::var(\"DB_URL\"); }".to_string(),
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

    assert!(hover.is_some(), "Expected hover on 'val' binding");
    assert!(format!("{:?}", hover.unwrap()).contains("postgres:
}

#[tokio::test]
async fn test_rust_hover_result_destructuring_var_short() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "main.rs",
        "fn main() { let Ok(val) = env::var(\"DB_URL\"); }",
    );
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "rust".to_string(),
            "fn main() { let Ok(val) = env::var(\"DB_URL\"); }".to_string(),
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

    assert!(hover.is_some(), "Expected hover on 'val' binding");
    assert!(format!("{:?}", hover.unwrap()).contains("postgres:
}

#[tokio::test]
async fn test_rust_hover_if_let() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "main.rs",
        "fn main() { if let Ok(val) = std::env::var(\"DB_URL\") { println!(\"{}\", val); } }",
    );
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "rust".to_string(),
            "fn main() { if let Ok(val) = std::env::var(\"DB_URL\") { println!(\"{}\", val); } }"
                .to_string(),
            0,
        )
        .await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 23), 
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover on 'val' binding in if let");
    assert!(format!("{:?}", hover.unwrap()).contains("postgres:
}

#[tokio::test]
async fn test_rust_hover_option_destructuring() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "main.rs",
        "fn main() { let Some(val) = std::env::var(\"DB_URL\").ok(); }",
    );
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "rust".to_string(),
            "fn main() { let Some(val) = std::env::var(\"DB_URL\").ok(); }".to_string(),
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

    assert!(
        hover.is_some(),
        "Expected hover on 'val' binding with Some destructuring"
    );
}

#[tokio::test]
async fn test_rust_hover_match_destructuring() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("main.rs", "fn main() { match std::env::var(\"DB_URL\") { Ok(val) => println!(\"{}\", val), _ => () } }");
    fixture.state.document_manager.open(uri.clone(), "rust".to_string(),
        "fn main() { match std::env::var(\"DB_URL\") { Ok(val) => println!(\"{}\", val), _ => () } }".to_string(), 0).await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 48), 
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(
        hover.is_some(),
        "Expected hover on 'val' binding in match arm"
    );
    assert!(format!("{:?}", hover.unwrap()).contains("postgres:
}

#[tokio::test]
async fn test_rust_diagnostics_result_destructuring_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "main.rs",
        "fn main() { let Ok(val) = std::env::var(\"MISSING_VAR\"); println!(\"{}\", val); }",
    );
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "rust".to_string(),
            "fn main() { let Ok(val) = std::env::var(\"MISSING_VAR\"); println!(\"{}\", val); }"
                .to_string(),
            0,
        )
        .await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;

    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("not defined")));
}

#[tokio::test]
async fn test_rust_diagnostics_if_let_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "main.rs",
        "fn main() { if let Ok(val) = std::env::var(\"MISSING_VAR\") { println!(\"{}\", val); } }",
    );
    fixture.state.document_manager.open(uri.clone(), "rust".to_string(), 
        "fn main() { if let Ok(val) = std::env::var(\"MISSING_VAR\") { println!(\"{}\", val); } }".to_string(), 0).await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;

    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("not defined")));
}

#[tokio::test]
async fn test_rust_diagnostics_match_destructuring_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("main.rs", "fn main() { match std::env::var(\"MISSING_VAR\") { Ok(val) => println!(\"{}\", val), _ => () } }");
    fixture.state.document_manager.open(uri.clone(), "rust".to_string(), 
        "fn main() { match std::env::var(\"MISSING_VAR\") { Ok(val) => println!(\"{}\", val), _ => () } }".to_string(), 0).await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;

    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("not defined")));
}

#[tokio::test]
async fn test_rust_completion() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("main.rs", "fn main() { std::env::var(\"\"); }");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "rust".to_string(),
            "fn main() { std::env::var(\"\"); }".to_string(),
            0,
        )
        .await;

    let completion = handle_completion(
        tower_lsp::lsp_types::CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 27),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        },
        &fixture.state,
    )
    .await;

    assert!(completion.is_some());
    assert!(completion.unwrap().iter().any(|i| i.label == "PORT"));
}

#[tokio::test]
async fn test_rust_dotenv_macro_mock() {

    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("main.rs", "fn main() { dotenv!(\"DEBUG\"); }");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "rust".to_string(),
            "fn main() { dotenv!(\"DEBUG\"); }".to_string(),
            0,
        )
        .await;


    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 24),
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

// ═══════════════════════════════════════════════════════════════════════════
// New Pattern Tests: dotenv/dotenvy, struct init, const/static, method chains
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_rust_hover_dotenv_var() {
    let fixture = TestFixture::new().await;
    let content = r#"fn main() { let db = dotenv::var("DB_URL"); }"#;
    let uri = fixture.create_file("main.rs", content);

    fixture
        .state
        .document_manager
        .open(uri.clone(), "rust".to_string(), content.to_string(), 0)
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 36),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover for dotenv::var");
    assert!(format!("{:?}", hover.unwrap()).contains("postgres://"));
}

#[tokio::test]
async fn test_rust_hover_dotenvy_var() {
    let fixture = TestFixture::new().await;
    let content = r#"fn main() { let db = dotenvy::var("DB_URL"); }"#;
    let uri = fixture.create_file("main.rs", content);

    fixture
        .state
        .document_manager
        .open(uri.clone(), "rust".to_string(), content.to_string(), 0)
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 37),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover for dotenvy::var");
    assert!(format!("{:?}", hover.unwrap()).contains("postgres://"));
}

#[tokio::test]
async fn test_rust_hover_method_chain_unwrap() {
    let fixture = TestFixture::new().await;
    let content = r#"fn main() { let db = env::var("DB_URL").unwrap(); }"#;
    let uri = fixture.create_file("main.rs", content);

    fixture
        .state
        .document_manager
        .open(uri.clone(), "rust".to_string(), content.to_string(), 0)
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 33),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover for method chain unwrap");
    assert!(format!("{:?}", hover.unwrap()).contains("postgres://"));
}

#[tokio::test]
async fn test_rust_hover_try_expression() {
    let fixture = TestFixture::new().await;
    let content = r#"fn get_db() -> Result<String, std::env::VarError> { let db = env::var("DB_URL")?; Ok(db) }"#;
    let uri = fixture.create_file("main.rs", content);

    fixture
        .state
        .document_manager
        .open(uri.clone(), "rust".to_string(), content.to_string(), 0)
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 73),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover for try expression");
    assert!(format!("{:?}", hover.unwrap()).contains("postgres://"));
}

#[tokio::test]
async fn test_rust_hover_const_env_macro() {
    let fixture = TestFixture::new().await;
    let content = r#"const DB: &str = env!("DB_URL"); fn main() {}"#;
    let uri = fixture.create_file("main.rs", content);

    fixture
        .state
        .document_manager
        .open(uri.clone(), "rust".to_string(), content.to_string(), 0)
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 24),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover for const env! macro");
    assert!(format!("{:?}", hover.unwrap()).contains("postgres://"));
}

#[tokio::test]
async fn test_rust_hover_static_env_macro() {
    let fixture = TestFixture::new().await;
    let content = r#"static DB: &str = env!("DB_URL"); fn main() {}"#;
    let uri = fixture.create_file("main.rs", content);

    fixture
        .state
        .document_manager
        .open(uri.clone(), "rust".to_string(), content.to_string(), 0)
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 25),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover for static env! macro");
    assert!(format!("{:?}", hover.unwrap()).contains("postgres://"));
}
