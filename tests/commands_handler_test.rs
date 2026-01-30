//! Tests for server/handlers/commands.rs - Command handler

mod common;

use common::TestFixture;
use ecolog_lsp::server::handlers::handle_execute_command;
use serde_json::json;
use tower_lsp::lsp_types::ExecuteCommandParams;

fn make_cmd(command: &str, arguments: Vec<serde_json::Value>) -> ExecuteCommandParams {
    ExecuteCommandParams {
        command: command.to_string(),
        arguments,
        work_done_progress_params: Default::default(),
    }
}

#[tokio::test]
async fn test_list_env_variables() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.listEnvVariables", vec![]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some(), "Should return result");
    let value = result.unwrap();
    assert!(value.get("variables").is_some(), "Should have variables array");
    assert!(value.get("count").is_some(), "Should have count");

    // Check that DB_URL from fixture .env is present
    let vars = value.get("variables").unwrap().as_array().unwrap();
    let has_db_url = vars.iter().any(|v| v.get("name").unwrap() == "DB_URL");
    assert!(has_db_url, "Should contain DB_URL from .env");
}

#[tokio::test]
async fn test_variable_get_found() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.variable.get", vec![json!("DB_URL")]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert_eq!(value.get("name").unwrap(), "DB_URL");
    assert!(value.get("value").is_some(), "Should have value");
    assert!(value.get("source").is_some(), "Should have source");
}

#[tokio::test]
async fn test_variable_get_not_found() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.variable.get", vec![json!("NONEXISTENT_VAR")]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert!(value.get("error").is_some(), "Should return error for not found");
}

#[tokio::test]
async fn test_variable_get_no_arg() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.variable.get", vec![]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert!(value.get("error").is_some(), "Should return error when no arg provided");
}

#[tokio::test]
async fn test_generate_env_example() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.generateEnvExample", vec![]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert!(value.get("content").is_some(), "Should have content");
    assert!(value.get("count").is_some(), "Should have count");

    let content = value.get("content").unwrap().as_str().unwrap();
    assert!(content.contains("DB_URL="), "Should contain DB_URL");
}

#[tokio::test]
async fn test_workspace_list() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.workspace.list", vec![]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert!(value.get("workspaces").is_some(), "Should have workspaces array");
    assert_eq!(value.get("count").unwrap(), 1, "Should have count of 1");
}

#[tokio::test]
async fn test_source_list() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.source.list", vec![]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert!(value.get("sources").is_some(), "Should have sources array");

    let sources = value.get("sources").unwrap().as_array().unwrap();
    assert_eq!(sources.len(), 3, "Should have 3 sources (Shell, File, Remote)");
}

#[tokio::test]
async fn test_source_set_precedence_valid() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.source.setPrecedence", vec![json!("File"), json!("Shell")]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert!(value.get("success").is_some(), "Should succeed");
    assert!(value.get("precedence").is_some(), "Should return new precedence");
}

#[tokio::test]
async fn test_source_set_precedence_invalid_source() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.source.setPrecedence", vec![json!("InvalidSource")]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert!(value.get("error").is_some(), "Should return error for invalid source");
}

#[tokio::test]
async fn test_source_set_precedence_empty_resets_to_all() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.source.setPrecedence", vec![]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert!(value.get("success").is_some(), "Should succeed");

    // Verify all sources are enabled
    let precedence = value.get("precedence").unwrap().as_array().unwrap();
    assert_eq!(precedence.len(), 3, "Empty should enable all sources");
}

#[tokio::test]
async fn test_interpolation_set_enable() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.interpolation.set", vec![json!(true)]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert_eq!(value.get("success").unwrap(), true);
    assert_eq!(value.get("enabled").unwrap(), true);
}

#[tokio::test]
async fn test_interpolation_set_disable() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.interpolation.set", vec![json!(false)]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert_eq!(value.get("success").unwrap(), true);
    assert_eq!(value.get("enabled").unwrap(), false);
}

#[tokio::test]
async fn test_interpolation_get() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.interpolation.get", vec![]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert!(value.get("enabled").is_some(), "Should return enabled status");
}

#[tokio::test]
async fn test_file_list() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.file.list", vec![]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert!(value.get("files").is_some(), "Should have files array");
    assert!(value.get("count").is_some(), "Should have count");
}

#[tokio::test]
async fn test_file_set_active_clear() {
    let fixture = TestFixture::new().await;

    // Clear active filter
    let params = make_cmd("ecolog.file.setActive", vec![]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert_eq!(value.get("success").unwrap(), true);
    assert!(value.get("message").is_some(), "Should have clear message");
}

#[tokio::test]
async fn test_file_set_active_with_patterns() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.file.setActive", vec![json!(".env.local")]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert_eq!(value.get("success").unwrap(), true);
    assert!(value.get("patterns").is_some(), "Should return patterns");
}

#[tokio::test]
async fn test_workspace_set_root_no_arg() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.workspace.setRoot", vec![]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert!(value.get("error").is_some(), "Should error without path arg");
}

#[tokio::test]
async fn test_workspace_set_root_nonexistent() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.workspace.setRoot", vec![json!("/nonexistent/path/that/does/not/exist")]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert!(value.get("error").is_some(), "Should error for nonexistent path");
}

#[tokio::test]
async fn test_workspace_set_root_valid() {
    let fixture = TestFixture::new().await;

    // Use the temp dir itself as a valid path
    let temp_path = fixture.temp_dir.to_string_lossy().to_string();

    let params = make_cmd("ecolog.workspace.setRoot", vec![json!(temp_path)]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_some());
    let value = result.unwrap();
    assert_eq!(value.get("success").unwrap(), true);
    assert!(value.get("root").is_some(), "Should return new root");
}

#[tokio::test]
async fn test_unknown_command_returns_none() {
    let fixture = TestFixture::new().await;

    let params = make_cmd("ecolog.unknownCommand", vec![]);
    let result = handle_execute_command(params, &fixture.state).await;

    assert!(result.is_none(), "Unknown command should return None");
}
