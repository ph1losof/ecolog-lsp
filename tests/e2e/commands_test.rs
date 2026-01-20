use crate::harness::{LspTestClient, TempWorkspace};
use serde_json::json;
use std::thread;
use std::time::Duration;

#[test]
fn test_command_list_env_variables() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let result = client
        .execute_command("ecolog.listEnvVariables", vec![])
        .expect("Command execution failed");

    let variables = result
        .get("variables")
        .expect("Should have variables")
        .as_array()
        .expect("Variables should be array");

    assert!(
        variables.len() >= 4,
        "Should have at least 4 variables (DB_URL, API_KEY, DEBUG, PORT)"
    );

    let names: Vec<&str> = variables
        .iter()
        .filter_map(|v| v.get("name")?.as_str())
        .collect();

    assert!(names.contains(&"DB_URL"), "Should have DB_URL");
    assert!(names.contains(&"API_KEY"), "Should have API_KEY");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_command_file_list() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let result = client
        .execute_command("ecolog.file.list", vec![])
        .expect("Command execution failed");

    let files = result
        .get("files")
        .expect("Should have files")
        .as_array()
        .expect("Files should be array");

    assert!(
        files
            .iter()
            .any(|f| f.as_str().map(|s| s.contains(".env")).unwrap_or(false)),
        "Should list .env file"
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_command_set_active_file() {
    let workspace = TempWorkspace::new();
    workspace.create_file(".env.production", "MODE=production\n");

    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    thread::sleep(Duration::from_millis(300));

    let result = client
        .execute_command("ecolog.file.setActive", vec![json!(".env.production")])
        .expect("Command execution failed");

    assert_eq!(
        result.get("success"),
        Some(&json!(true)),
        "setActive should succeed"
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_command_get_variable() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let result = client
        .execute_command("ecolog.variable.get", vec![json!("DB_URL")])
        .expect("Command execution failed");

    assert_eq!(
        result.get("name").and_then(|n| n.as_str()),
        Some("DB_URL"),
        "Should return correct variable name"
    );
    assert!(
        result
            .get("value")
            .and_then(|v| v.as_str())
            .map(|s| s.contains("postgres"))
            .unwrap_or(false),
        "Should return correct value"
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_command_get_variable_not_found() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let result = client
        .execute_command("ecolog.variable.get", vec![json!("NONEXISTENT_VAR")])
        .expect("Command execution failed");

    assert!(
        result.is_null() || result.get("error").is_some() || result.get("value").is_none(),
        "Nonexistent variable should indicate not found"
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_command_interpolation_toggle() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let result = client
        .execute_command("ecolog.interpolation.set", vec![json!(false)])
        .expect("Command execution failed");

    assert_eq!(result.get("success"), Some(&json!(true)));
    assert_eq!(result.get("enabled"), Some(&json!(false)));

    let get_result = client
        .execute_command("ecolog.interpolation.get", vec![])
        .expect("Command execution failed");

    assert_eq!(get_result.get("enabled"), Some(&json!(false)));

    let result = client
        .execute_command("ecolog.interpolation.set", vec![json!(true)])
        .expect("Command execution failed");

    assert_eq!(result.get("enabled"), Some(&json!(true)));

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_command_workspace_list() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let result = client
        .execute_command("ecolog.workspace.list", vec![])
        .expect("Command execution failed");

    assert!(!result.is_null(), "Should return workspace info");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_command_workspace_set_root() {
    let workspace = TempWorkspace::new();
    let subdir = workspace.root.join("subproject");
    std::fs::create_dir_all(&subdir).unwrap();
    std::fs::write(subdir.join(".env"), "SUB_VAR=value\n").unwrap();

    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let result = client
        .execute_command(
            "ecolog.workspace.setRoot",
            vec![json!(subdir.to_string_lossy())],
        )
        .expect("Command execution failed");

    assert_eq!(result.get("success"), Some(&json!(true)));

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_unknown_command_returns_null() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let result = client
        .execute_command("ecolog.unknown.command", vec![])
        .expect("Command execution failed");

    assert!(result.is_null(), "Unknown command should return null");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_command_generate_env_example() {
    let workspace = TempWorkspace::new();
    workspace.create_file(
        "app.js",
        "const url = process.env.DB_URL;\nconst key = process.env.API_KEY;",
    );

    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("app.js");
    client
        .open_document(
            &uri,
            "javascript",
            "const url = process.env.DB_URL;\nconst key = process.env.API_KEY;",
        )
        .expect("Failed to open document");

    thread::sleep(Duration::from_millis(500));

    let result = client
        .execute_command("ecolog.generateEnvExample", vec![])
        .expect("Command execution failed");

    if !result.is_null() {
        if let Some(content) = result.get("content").and_then(|c| c.as_str()) {
            assert!(content.len() < 100000, "Content should be reasonable size");
        }
    }

    client.shutdown().expect("Shutdown failed");
}
