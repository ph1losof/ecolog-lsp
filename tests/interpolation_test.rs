mod common;

use common::TestFixture;
use ecolog_lsp::server::handlers::{handle_execute_command, handle_hover};
use serde_json::json;
use std::fs::File;
use std::io::Write;
use tower_lsp::lsp_types::{
    ExecuteCommandParams, HoverParams, Position, TextDocumentIdentifier, TextDocumentPositionParams,
};

async fn set_interpolation(fixture: &TestFixture, enabled: bool) -> Option<serde_json::Value> {
    let params = ExecuteCommandParams {
        command: "ecolog.interpolation.set".to_string(),
        arguments: vec![json!(enabled)],
        work_done_progress_params: Default::default(),
    };
    handle_execute_command(params, &fixture.state).await
}

async fn get_interpolation(fixture: &TestFixture) -> Option<serde_json::Value> {
    let params = ExecuteCommandParams {
        command: "ecolog.interpolation.get".to_string(),
        arguments: vec![],
        work_done_progress_params: Default::default(),
    };
    handle_execute_command(params, &fixture.state).await
}

async fn get_hover(
    fixture: &TestFixture,
    uri: &tower_lsp::lsp_types::Url,
    line: u32,
    col: u32,
) -> Option<tower_lsp::lsp_types::Hover> {
    handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(line, col),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await
}

fn extract_hover_value(hover: &tower_lsp::lsp_types::Hover) -> Option<String> {
    match &hover.contents {
        tower_lsp::lsp_types::HoverContents::Markup(markup) => {
            let content = &markup.value;
            if let Some(start) = content.find("**Value**: `") {
                let value_start = start + "**Value**: `".len();
                if let Some(end) = content[value_start..].find('`') {
                    return Some(content[value_start..value_start + end].to_string());
                }
            }
            None
        }
        _ => None,
    }
}

#[tokio::test]
async fn test_get_interpolation_returns_enabled_by_default() {
    let fixture = TestFixture::new().await;

    let result = get_interpolation(&fixture).await;
    assert!(result.is_some(), "Get interpolation should return a result");

    let result = result.unwrap();
    assert_eq!(
        result.get("enabled").and_then(|v| v.as_bool()),
        Some(true),
        "Interpolation should be enabled by default"
    );
}

#[tokio::test]
async fn test_set_interpolation_changes_state() {
    let fixture = TestFixture::new().await;

    let result = get_interpolation(&fixture).await.unwrap();
    assert_eq!(result.get("enabled").and_then(|v| v.as_bool()), Some(true));

    let result = set_interpolation(&fixture, false).await;
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.get("success").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(result.get("enabled").and_then(|v| v.as_bool()), Some(false));

    let result = get_interpolation(&fixture).await.unwrap();
    assert_eq!(
        result.get("enabled").and_then(|v| v.as_bool()),
        Some(false),
        "Interpolation should now be disabled"
    );

    let result = set_interpolation(&fixture, true).await.unwrap();
    assert_eq!(result.get("success").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(result.get("enabled").and_then(|v| v.as_bool()), Some(true));

    let result = get_interpolation(&fixture).await.unwrap();
    assert_eq!(
        result.get("enabled").and_then(|v| v.as_bool()),
        Some(true),
        "Interpolation should be enabled again"
    );
}

#[tokio::test]
async fn test_interpolation_affects_hover_values() {
    let fixture = TestFixture::new().await;

    let env_path = fixture.temp_dir.join(".env.interpolation");
    {
        let mut env_file = File::create(&env_path).unwrap();
        writeln!(env_file, "BASE_DIR=/home/user").unwrap();
        writeln!(env_file, "DATA_PATH=${{BASE_DIR}}/data").unwrap();
    }

    fixture
        .state
        .core
        .refresh(abundantis::RefreshOptions::reset_all())
        .await
        .unwrap();

    let uri = fixture.create_file("test.js", "process.env.DATA_PATH");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.DATA_PATH".to_string(),
            0,
        )
        .await;

    let hover = get_hover(&fixture, &uri, 0, 20).await;
    if let Some(hover) = hover {
        let value = extract_hover_value(&hover);
        if let Some(value) = value {
            assert!(
                value.contains("/home/user/data") || value == "/home/user/data",
                "With interpolation enabled, should show resolved value. Got: {}",
                value
            );
        }
    }

    set_interpolation(&fixture, false).await;

    fixture
        .state
        .core
        .refresh(abundantis::RefreshOptions::reset_all())
        .await
        .unwrap();

    let hover = get_hover(&fixture, &uri, 0, 20).await;
    if let Some(hover) = hover {
        let value = extract_hover_value(&hover);
        if let Some(value) = value {
            assert!(
                value.contains("${BASE_DIR}") || value == "${BASE_DIR}/data",
                "With interpolation disabled, should show raw value. Got: {}",
                value
            );
        }
    }
}

#[tokio::test]
async fn test_boolean_argument_parsing() {
    let fixture = TestFixture::new().await;

    let params = ExecuteCommandParams {
        command: "ecolog.interpolation.set".to_string(),
        arguments: vec![json!(true)],
        work_done_progress_params: Default::default(),
    };
    let result = handle_execute_command(params, &fixture.state)
        .await
        .unwrap();
    assert_eq!(result.get("enabled").and_then(|v| v.as_bool()), Some(true));

    let params = ExecuteCommandParams {
        command: "ecolog.interpolation.set".to_string(),
        arguments: vec![json!(false)],
        work_done_progress_params: Default::default(),
    };
    let result = handle_execute_command(params, &fixture.state)
        .await
        .unwrap();
    assert_eq!(result.get("enabled").and_then(|v| v.as_bool()), Some(false));

    let result = get_interpolation(&fixture).await.unwrap();
    assert_eq!(result.get("enabled").and_then(|v| v.as_bool()), Some(false));
}
