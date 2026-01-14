//! Diagnostics tests for LSP server

use crate::harness::{LspTestClient, TempWorkspace};
use std::thread;
use std::time::Duration;

#[test]
fn test_diagnostics_undefined_env_var() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.UNDEFINED_VAR";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");

    // Wait for diagnostics notification
    let notification = client
        .wait_for_notification("textDocument/publishDiagnostics", Duration::from_secs(5))
        .expect("Should receive diagnostics");

    let params = notification.params.expect("Should have params");
    let diagnostics = params
        .get("diagnostics")
        .expect("Should have diagnostics")
        .as_array()
        .expect("Diagnostics should be array");

    assert!(!diagnostics.is_empty(), "Should have diagnostics for undefined var");

    let diag = &diagnostics[0];
    let message = diag.get("message").expect("Should have message").as_str().unwrap();
    assert!(
        message.contains("not defined") || message.contains("undefined"),
        "Message should indicate undefined: {}", message
    );

    // Severity should be warning (2) or error (1)
    let severity = diag.get("severity").expect("Should have severity").as_i64().unwrap();
    assert!(severity == 1 || severity == 2, "Severity should be error or warning");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_diagnostics_defined_var_no_warning() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.DB_URL";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");

    // Wait for diagnostics
    let notification = client
        .wait_for_notification("textDocument/publishDiagnostics", Duration::from_secs(5))
        .expect("Should receive diagnostics");

    let params = notification.params.expect("Should have params");
    let diagnostics = params
        .get("diagnostics")
        .expect("Should have diagnostics")
        .as_array()
        .expect("Diagnostics should be array");

    // Should have no diagnostics for defined var
    assert!(
        diagnostics.is_empty(),
        "Defined var should have no diagnostics"
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_diagnostics_update_on_document_change() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    workspace.create_file("test.js", "process.env.DB_URL");

    // Open with defined var - should have no diagnostics
    client.open_document(&uri, "javascript", "process.env.DB_URL").expect("Failed to open");
    thread::sleep(Duration::from_millis(500));
    client.clear_notifications();

    // Change to undefined var
    client.change_document(&uri, 2, "process.env.UNDEFINED_VAR").expect("Failed to change");

    // Should receive new diagnostics
    let notification = client
        .wait_for_notification("textDocument/publishDiagnostics", Duration::from_secs(5))
        .expect("Should receive diagnostics after change");

    let params = notification.params.expect("Should have params");
    let diagnostics = params
        .get("diagnostics")
        .expect("Should have diagnostics")
        .as_array()
        .expect("Diagnostics should be array");

    assert!(
        !diagnostics.is_empty(),
        "Should have diagnostics for undefined var after change"
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_diagnostics_disabled_via_config() {
    let workspace = TempWorkspace::new();
    workspace.create_config(
        r#"
[features]
diagnostics = false
"#,
    );

    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.UNDEFINED_VAR";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");

    let notification = client.wait_for_notification("textDocument/publishDiagnostics", Duration::from_secs(2));

    if let Some(n) = notification {
        let params = n.params.expect("Should have params");
        let diagnostics = params
            .get("diagnostics")
            .expect("Should have diagnostics")
            .as_array()
            .expect("Diagnostics should be array");

        assert!(
            diagnostics.is_empty(),
            "Diagnostics should be empty when disabled"
        );
    }

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_diagnostics_multiple_undefined() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.UNDEFINED_A;\nprocess.env.UNDEFINED_B;";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");

    let notification = client
        .wait_for_notification("textDocument/publishDiagnostics", Duration::from_secs(5))
        .expect("Should receive diagnostics");

    let params = notification.params.expect("Should have params");
    let diagnostics = params
        .get("diagnostics")
        .expect("Should have diagnostics")
        .as_array()
        .expect("Diagnostics should be array");

    // Should have 2 diagnostics for 2 undefined vars
    assert!(
        diagnostics.len() >= 2,
        "Should have diagnostics for each undefined var, got {}",
        diagnostics.len()
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_diagnostics_correct_range() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const x = process.env.MISSING;";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");

    let notification = client
        .wait_for_notification("textDocument/publishDiagnostics", Duration::from_secs(5))
        .expect("Should receive diagnostics");

    let params = notification.params.expect("Should have params");
    let diagnostics = params
        .get("diagnostics")
        .expect("Should have diagnostics")
        .as_array()
        .expect("Diagnostics should be array");

    assert!(!diagnostics.is_empty(), "Should have diagnostic");

    let diag = &diagnostics[0];
    let range = diag.get("range").expect("Should have range");
    let start = range.get("start").expect("Should have start");

    // MISSING starts at character 22
    assert_eq!(start.get("line").unwrap().as_i64(), Some(0));
    assert!(start.get("character").unwrap().as_i64().unwrap() >= 22);

    client.shutdown().expect("Shutdown failed");
}
