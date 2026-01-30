mod common;
use common::TestFixture;
use ecolog_lsp::server::handlers::*;
use std::fs::OpenOptions;
use std::io::Write;
use tower_lsp::lsp_types::*;

#[tokio::test]
async fn test_scenario_multiple_env_files() {
    let fixture = TestFixture::new().await;

    let local_path = fixture.temp_dir.join(".env.local");
    let mut f = std::fs::File::create(&local_path).unwrap();
    writeln!(f, "PORT=9090").unwrap();
    writeln!(f, "NEW_VAR=custom").unwrap();

    fixture.state.core.clear_active_files();

    fixture
        .state
        .core
        .refresh(abundantis::RefreshOptions::reset_all())
        .await
        .expect("Refresh failed");

    let uri = fixture.create_file("test.js", "process.env.PORT");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.PORT".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(0, 12),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    let h_str = format!("{:?}", hover.unwrap());
    assert!(
        h_str.contains("9090"),
        "Expected override value 9090, got: {}",
        h_str
    );

    fixture
        .state
        .document_manager
        .change(
            &uri,
            vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "process.env.NEW_VAR".to_string(),
            }],
            1,
        )
        .await;

    let hover_new = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 12),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover_new.is_some());
    assert!(format!("{:?}", hover_new.unwrap()).contains("custom"));
}

#[tokio::test]
async fn test_scenario_env_syntax_diagnostics() {
    let fixture = TestFixture::new().await;

    let env_path = fixture.temp_dir.join(".env.bad");
    let mut f = std::fs::File::create(&env_path).unwrap();
    writeln!(f, "BAD LINE").unwrap();
    let uri = Url::from_file_path(&env_path).unwrap();

    {
        let manager = fixture.state.config.clone();
        let config_arc = manager.get_config();
        let mut config = config_arc.write().await;
        config.workspace.env_files.push(".env.bad".into());
    }

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "plaintext".to_string(),
            "BAD LINE".to_string(),
            0,
        )
        .await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;
    assert!(!diags.is_empty(), "Expected syntax diagnostics");
    assert!(diags[0].message.contains("Syntax"));
}

#[tokio::test]
async fn test_scenario_quoted_values() {
    let fixture = TestFixture::new().await;

    let env_path = fixture.temp_dir.join(".env");
    {
        let mut f = OpenOptions::new().append(true).open(env_path).unwrap();
        writeln!(f, "QUOTED=\"some value\"").unwrap();
    }

    fixture
        .state
        .core
        .refresh(abundantis::RefreshOptions::reset_all())
        .await
        .unwrap();

    let uri = fixture.create_file("test.py", "os.environ['QUOTED']");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".to_string(),
            "os.environ['QUOTED']".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 15),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    assert!(format!("{:?}", hover.unwrap()).contains("some value"));
}

#[tokio::test]
async fn test_scenario_commented_env() {
    let fixture = TestFixture::new().await;
    let env_path = fixture.temp_dir.join(".env");
    {
        let mut f = OpenOptions::new().append(true).open(env_path).unwrap();
        writeln!(f, "# IGNORE_ME=true").unwrap();
    }

    fixture
        .state
        .core
        .refresh(abundantis::RefreshOptions::reset_all())
        .await
        .unwrap();

    let uri = fixture.create_file("test.js", "process.env.IGNORE_ME");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.IGNORE_ME".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(0, 15),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(
        hover.is_none(),
        "Hover should return None for undefined variables"
    );

    let diags = compute_diagnostics(&uri, &fixture.state).await;
    assert!(
        diags.iter().any(|d| d.message.contains("not defined")),
        "Should have diagnostic for undefined var"
    );
}

#[tokio::test]
async fn test_scenario_empty_value() {
    let fixture = TestFixture::new().await;
    let env_path = fixture.temp_dir.join(".env");
    {
        let mut f = OpenOptions::new().append(true).open(env_path).unwrap();
        writeln!(f, "EMPTY=").unwrap();
    }

    fixture
        .state
        .core
        .refresh(abundantis::RefreshOptions::reset_all())
        .await
        .unwrap();

    let uri = fixture.create_file("test.js", "process.env.EMPTY");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.EMPTY".to_string(),
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

    assert!(
        hover.is_some(),
        "Hover should find EMPTY variable with empty value"
    );

    let h_str = format!("{:?}", hover.unwrap());
    assert!(
        h_str.contains("Value**: ``") || h_str.contains("Value**: \"\""),
        "Expected empty value, got: {}",
        h_str
    );
}

#[tokio::test]
async fn test_scenario_multiline_value() {
    let fixture = TestFixture::new().await;
    let env_path = fixture.temp_dir.join(".env");
    {
        let mut f = OpenOptions::new().append(true).open(env_path).unwrap();
        writeln!(f, "MULTI=\"line1\\nline2\"").unwrap();
    }

    fixture
        .state
        .core
        .refresh(abundantis::RefreshOptions::reset_all())
        .await
        .unwrap();

    let uri = fixture.create_file("test.rs", "std::env::var(\"MULTI\")");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "rust".to_string(),
            "std::env::var(\"MULTI\")".to_string(),
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

    assert!(hover.is_some());
    assert!(format!("{:?}", hover.unwrap()).contains("line1"));
}

#[tokio::test]
async fn test_scenario_forbidden_whitespace() {
    let fixture = TestFixture::new().await;
    let env_path = fixture.temp_dir.join(".env.bad2");
    let mut f = std::fs::File::create(&env_path).unwrap();
    writeln!(f, "KEY =VALUE").unwrap();
    let uri = Url::from_file_path(&env_path).unwrap();

    {
        let manager = fixture.state.config.clone();
        let config_arc = manager.get_config();
        let mut config = config_arc.write().await;
        config.workspace.env_files.push(".env.bad".into());
    }

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "plaintext".to_string(),
            "KEY =VALUE".to_string(),
            0,
        )
        .await;

    let diags = compute_diagnostics(&uri, &fixture.state).await;
    if !diags.is_empty() {
        assert!(
            diags[0].message.contains("whitespace")
                || diags[0].message.contains("Unexpected")
                || diags[0].message.contains("Invalid")
        );
    }
}

#[tokio::test]
async fn test_feature_hover_disabled() {
    let fixture = TestFixture::new().await;
    {
        let config_arc = fixture.state.config.get_config();
        let mut config = config_arc.write().await.clone();
        config.features.hover = false;
        fixture.state.config.update(config).await;
    }

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

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 15),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_none());
}

#[tokio::test]
async fn test_feature_completion_disabled() {
    let fixture = TestFixture::new().await;
    {
        let config_arc = fixture.state.config.get_config();
        let mut config = config_arc.write().await.clone();
        config.features.completion = false;
        fixture.state.config.update(config).await;
    }

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
            context: None,
        },
        &fixture.state,
    )
    .await;

    assert!(completion.is_none());
}

#[tokio::test]
async fn test_case_sensitivity() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "process.env.db_url");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.db_url".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 15),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(
        hover.is_none(),
        "db_url (lowercase) should not match DB_URL"
    );
}

#[tokio::test]
async fn test_active_file_selection() {
    let fixture = TestFixture::new().await;
    let prod_path = fixture.temp_dir.join(".env.production");
    let mut f = std::fs::File::create(&prod_path).unwrap();
    writeln!(f, "MODE=PROD").unwrap();

    fixture
        .state
        .core
        .refresh(abundantis::RefreshOptions::reset_all())
        .await
        .unwrap();

    fixture.state.core.set_active_files(&[".env.production"]);

    let uri = fixture.create_file("test.js", "process.env.MODE");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.MODE".to_string(),
            0,
        )
        .await;

    let hover = handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 13),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await;

    assert!(hover.is_some());
    assert!(format!("{:?}", hover.unwrap()).contains("PROD"));
}

#[tokio::test]
async fn test_list_variables_command() {
    let fixture = TestFixture::new().await;

    let test_file = fixture.temp_dir.join("test.txt");
    std::fs::write(&test_file, "test").ok();

    let params = ExecuteCommandParams {
        command: "ecolog.listEnvVariables".to_string(),
        arguments: vec![],
        work_done_progress_params: Default::default(),
    };

    let res = handle_execute_command(params, &fixture.state).await;
    assert!(res.is_some());
    let json = res.unwrap();
    let vars = json.get("variables").unwrap().as_array().unwrap();
    assert!(vars.len() >= 4);
}

#[tokio::test]
async fn test_list_env_files_command() {
    let fixture = TestFixture::new().await;

    {
        let config_arc = fixture.state.config.get_config();
        let mut config = config_arc.write().await;
        config.resolution.precedence = vec![abundantis::config::SourcePrecedence::File];
    }

    let params = ExecuteCommandParams {
        command: "ecolog.file.list".to_string(),
        arguments: vec![],
        work_done_progress_params: Default::default(),
    };

    let res = handle_execute_command(params, &fixture.state).await;
    assert!(res.is_some());
    let json = res.unwrap();
    let files = json.get("files").unwrap().as_array().unwrap();
    assert!(files.iter().any(|v| v.as_str() == Some(".env")));
}
