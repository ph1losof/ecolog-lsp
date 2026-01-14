//! Completion tests for LSP server

use crate::harness::{LspTestClient, TempWorkspace};
use std::thread;
use std::time::Duration;

#[test]
fn test_completion_trigger_character() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    // Completion after "process.env."
    let completion = client.completion(&uri, 0, 12).expect("Completion request failed");

    assert!(!completion.is_null(), "Expected completion items");

    let items = completion.as_array().expect("Completion should be array");
    assert!(!items.is_empty(), "Should have completion items");

    // Check for expected env vars
    let labels: Vec<&str> = items
        .iter()
        .filter_map(|i| i.get("label")?.as_str())
        .collect();

    assert!(labels.contains(&"DB_URL"), "Should have DB_URL completion");
    assert!(labels.contains(&"API_KEY"), "Should have API_KEY completion");
    assert!(labels.contains(&"PORT"), "Should have PORT completion");
    assert!(labels.contains(&"DEBUG"), "Should have DEBUG completion");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_completion_on_alias() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.ts");
    let content = "const env = process.env; env.";
    workspace.create_file("test.ts", content);

    client.open_document(&uri, "typescript", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    // Completion after "env."
    let completion = client.completion(&uri, 0, 29).expect("Completion request failed");

    assert!(!completion.is_null(), "Expected completion items on alias");

    let items = completion.as_array().expect("Completion should be array");
    assert!(!items.is_empty(), "Should have completion items on alias");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_completion_item_documentation() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    let completion = client.completion(&uri, 0, 12).expect("Completion request failed");

    let items = completion.as_array().expect("Completion should be array");
    let db_url_item = items
        .iter()
        .find(|i| i.get("label").map(|l| l == "DB_URL").unwrap_or(false))
        .expect("Should have DB_URL completion");

    // Should have documentation with value
    let doc = db_url_item.get("documentation");
    assert!(doc.is_some(), "DB_URL completion should have documentation");

    let doc_value = doc
        .unwrap()
        .get("value")
        .expect("Documentation should have value")
        .as_str()
        .expect("Documentation value should be string");

    assert!(
        doc_value.contains("postgres://"),
        "Documentation should contain the value"
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_completion_disabled_via_config() {
    let workspace = TempWorkspace::new();
    workspace.create_config(
        r#"
[features]
completion = false
"#,
    );

    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    let completion = client.completion(&uri, 0, 12).expect("Completion request failed");

    // When completion is disabled, should return null or empty
    assert!(
        completion.is_null() || completion.as_array().map(|a| a.is_empty()).unwrap_or(false),
        "Completion should be null or empty when disabled"
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_completion_no_results_outside_env() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const x = 1; // not env";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    // Completion in regular code (not after process.env.)
    let completion = client.completion(&uri, 0, 5).expect("Completion request failed");

    // Should return null or empty for non-env context
    assert!(
        completion.is_null() || completion.as_array().map(|a| a.is_empty()).unwrap_or(false),
        "Should not provide completion outside env context"
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_completion_python() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.py");
    let content = "import os\nos.environ['";
    workspace.create_file("test.py", content);

    client.open_document(&uri, "python", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    // Completion inside environ['...']
    let _completion = client.completion(&uri, 1, 13).expect("Completion request failed");

    // Python completion may work differently, but shouldn't crash
    // The important thing is the request completes without error

    client.shutdown().expect("Shutdown failed");
}
