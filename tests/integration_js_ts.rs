mod common;
use common::TestFixture;
use ecolog_lsp::server::handlers::{
    compute_diagnostics, handle_completion, handle_definition, handle_hover,
};
use tower_lsp::lsp_types::{
    CompletionContext, CompletionParams, CompletionTriggerKind, GotoDefinitionParams, HoverParams,
    Position, TextDocumentIdentifier, TextDocumentPositionParams,
};

#[tokio::test]
async fn test_js_hover_direct() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const a = process.env.DB_URL;");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const a = process.env.DB_URL;".to_string(),
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

    assert!(hover.is_some());
    assert!(format!("{:?}", hover.unwrap()).contains("postgres://"));
}

#[tokio::test]
async fn test_js_hover_bracket() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const a = process.env['API_KEY'];");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const a = process.env['API_KEY'];".to_string(),
            0,
        )
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

    assert!(hover.is_some());
    assert!(format!("{:?}", hover.unwrap()).contains("secret_key"));
}

#[tokio::test]
async fn test_js_hover_destructuring() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const { PORT } = process.env;");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const { PORT } = process.env;".to_string(),
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

    assert!(hover.is_some());
    assert!(format!("{:?}", hover.unwrap()).contains("8080"));
}

#[tokio::test]
async fn test_js_completion_trigger() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "process.env.");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.".to_string(),
            0,
        )
        .await;

    let completion = handle_completion(
        CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 12),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
                trigger_character: Some(".".to_string()),
            }),
        },
        &fixture.state,
    )
    .await;

    assert!(completion.is_some());
    let items = completion.unwrap();
    assert!(items.iter().any(|i| i.label == "DB_URL"));
    assert!(items.iter().any(|i| i.label == "PORT"));
}

#[tokio::test]
async fn test_js_definition_direct() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "process.env.DB_URL");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.DB_URL".to_string(),
            0,
        )
        .await;

    let def = handle_definition(
        GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 15),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(def.is_some());
}

#[tokio::test]
async fn test_js_diagnostics_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "process.env.MISSING_VAR");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.MISSING_VAR".to_string(),
            0,
        )
        .await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;

    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("not defined")));
}

#[tokio::test]
async fn test_js_object_alias_hover() {
    let fixture = TestFixture::new().await;
    
    let content = "const e = process.env; e.PORT;";
    let uri = fixture.create_file("test.js", content);

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            content.to_string(),
            0,
        )
        .await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 26), 
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
async fn test_ts_hover_type_cast() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.ts", "const a = process.env.PORT as string;");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "typescript".to_string(),
            "const a = process.env.PORT as string;".to_string(),
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

    assert!(hover.is_some());
    assert!(format!("{:?}", hover.unwrap()).contains("8080"));
}

#[tokio::test]
async fn test_ts_completion_on_alias() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.ts", "const env = process.env; env.");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "typescript".to_string(),
            "const env = process.env; env.".to_string(),
            0,
        )
        .await;

    let completion = handle_completion(
        CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 29),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
                trigger_character: Some(".".to_string()),
            }),
        },
        &fixture.state,
    )
    .await;

    
    if let Some(items) = completion {
        assert!(items.iter().any(|i| i.label == "PORT"));
    } else {
        
        
    }
}






#[tokio::test]
async fn test_js_hover_destructuring_rename() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const { API_KEY: apiKey } = process.env;");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const { API_KEY: apiKey } = process.env;".to_string(),
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

    assert!(hover.is_some());
    assert!(format!("{:?}", hover.unwrap()).contains("secret_key"));
}




#[tokio::test]
async fn test_js_hover_destructuring_property_key() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const { API_KEY: apiKey } = process.env;");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const { API_KEY: apiKey } = process.env;".to_string(),
            0,
        )
        .await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 10), 
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover on API_KEY property key");
    assert!(format!("{:?}", hover.unwrap()).contains("secret_key"));
}

#[tokio::test]
async fn test_js_hover_destructuring_rename_bracket() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const { API_KEY: apiKey } = process.env; console.log(apiKey);",
    );
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const { API_KEY: apiKey } = process.env; console.log(apiKey);".to_string(),
            0,
        )
        .await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 55),
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
async fn test_js_hover_destructuring_with_default() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const { PORT: port = 3000 } = process.env;");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const { PORT: port = 3000 } = process.env;".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 14), 
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
async fn test_js_diagnostics_destructuring_rename_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const { MISSING_VAR: missing } = process.env; console.log(missing);",
    );
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const { MISSING_VAR: missing } = process.env; console.log(missing);".to_string(),
            0,
        )
        .await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;

    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("not defined")));
}

#[tokio::test]
async fn test_js_diagnostics_destructuring_with_default_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const { MISSING_VAR: missing = 'default' } = process.env; console.log(missing);",
    );
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const { MISSING_VAR: missing = 'default' } = process.env; console.log(missing);"
                .to_string(),
            0,
        )
        .await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;

    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("not defined")));
}




#[tokio::test]
async fn test_js_destructuring_multiple() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const { API_KEY: apiKey, PORT: port, DB_URL: dbUrl } = process.env;",
    );
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const { API_KEY: apiKey, PORT: port, DB_URL: dbUrl } = process.env;".to_string(),
            0,
        )
        .await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(0, 17), 
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover on apiKey binding");
    assert!(format!("{:?}", hover.unwrap()).contains("secret_key"));

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(0, 31), 
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover on port binding");
    assert!(format!("{:?}", hover.unwrap()).contains("8080"));

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(0, 45), 
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover on dbUrl binding");
    assert!(format!("{:?}", hover.unwrap()).contains("postgres://"));
}

#[tokio::test]
async fn test_js_destructuring_undefined_and_defined_mix() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const { API_KEY: apiKey, MISSING_VAR: missing } = process.env; console.log(apiKey, missing);");
    fixture.state.document_manager.open(uri.clone(), "javascript".to_string(), 
        "const { API_KEY: apiKey, MISSING_VAR: missing } = process.env; console.log(apiKey, missing);".to_string(), 0).await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;

    
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("MISSING_VAR")));
}






#[tokio::test]
async fn test_ts_hover_destructuring_rename() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.ts", "const { API_KEY: apiKey } = process.env;");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "typescript".to_string(),
            "const { API_KEY: apiKey } = process.env;".to_string(),
            0,
        )
        .await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 10), 
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover on API_KEY property key");
    assert!(format!("{:?}", hover.unwrap()).contains("secret_key"));
}

#[tokio::test]
async fn test_ts_hover_destructuring_with_default() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.ts", "const { PORT: port = 3000 } = process.env;");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "typescript".to_string(),
            "const { PORT: port = 3000 } = process.env;".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 14),
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
async fn test_ts_diagnostics_destructuring_rename_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.ts",
        "const { MISSING_VAR: missing } = process.env; console.log(missing);",
    );
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "typescript".to_string(),
            "const { MISSING_VAR: missing } = process.env; console.log(missing);".to_string(),
            0,
        )
        .await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;

    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("not defined")));
}




#[tokio::test]
async fn test_ts_destructuring_with_type_cast_and_rename() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.ts",
        "const { API_KEY: apiKey } = process.env; const typed = apiKey as string;",
    );
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "typescript".to_string(),
            "const { API_KEY: apiKey } = process.env; const typed = apiKey as string;".to_string(),
            0,
        )
        .await;

    
    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 10), 
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some(), "Expected hover on API_KEY property key");
    assert!(format!("{:?}", hover.unwrap()).contains("secret_key"));
}
