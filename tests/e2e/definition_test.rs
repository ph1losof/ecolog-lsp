//! Go-to-definition tests for LSP server

use crate::harness::{LspTestClient, TempWorkspace};
use std::thread;
use std::time::Duration;

#[test]
fn test_goto_definition_to_env_file() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.DB_URL";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    let definition = client.definition(&uri, 0, 15).expect("Definition request failed");

    assert!(!definition.is_null(), "Expected definition result");

    // Should point to .env file
    let def_uri = definition.get("uri").expect("Should have uri").as_str().unwrap();
    assert!(def_uri.ends_with(".env"), "Definition should point to .env file");

    // Should have correct range
    let range = definition.get("range").expect("Should have range");
    let start = range.get("start").expect("Should have start");
    assert_eq!(start.get("line").unwrap().as_i64(), Some(0), "DB_URL is on first line");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_definition_from_binding() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const { PORT } = process.env; console.log(PORT);";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    // Go to definition from usage of PORT at the end
    let definition = client.definition(&uri, 0, 44).expect("Definition request failed");

    assert!(!definition.is_null(), "Expected definition result from binding usage");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_definition_disabled_via_config() {
    let workspace = TempWorkspace::new();
    workspace.create_config(
        r#"
[features]
definition = false
"#,
    );

    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.DB_URL";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    let definition = client.definition(&uri, 0, 15).expect("Definition request failed");

    assert!(definition.is_null(), "Definition should be null when disabled");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_definition_undefined_var() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.UNDEFINED_VAR";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    let definition = client.definition(&uri, 0, 15).expect("Definition request failed");

    // Undefined vars should return null (no definition)
    assert!(definition.is_null(), "Undefined var should have no definition");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_definition_python() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.py");
    let content = "import os\ndb = os.environ['DB_URL']";
    workspace.create_file("test.py", content);

    client.open_document(&uri, "python", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    // Go to definition from DB_URL in Python
    let definition = client.definition(&uri, 1, 18).expect("Definition request failed");

    assert!(!definition.is_null(), "Expected definition result for Python");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_definition_outside_env_returns_null() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const x = 1;";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    // Definition on regular code
    let definition = client.definition(&uri, 0, 6).expect("Definition request failed");

    assert!(definition.is_null(), "Non-env code should have no definition");

    client.shutdown().expect("Shutdown failed");
}
