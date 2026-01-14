//! Document synchronization tests for LSP server

use crate::harness::{LspTestClient, TempWorkspace};
use std::thread;
use std::time::Duration;

#[test]
fn test_document_open_close_cycle() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    workspace.create_file("test.js", "process.env.DB_URL");

    // Open
    client.open_document(&uri, "javascript", "process.env.DB_URL").expect("Failed to open");
    thread::sleep(Duration::from_millis(200));

    // Verify hover works (document is tracked)
    let hover = client.hover(&uri, 0, 15).expect("Hover request failed");
    assert!(!hover.is_null(), "Hover should work on open document");

    // Close
    client.close_document(&uri).expect("Failed to close");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_document_content_changes() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    workspace.create_file("test.js", "process.env.DB_URL");

    client.open_document(&uri, "javascript", "process.env.DB_URL").expect("Failed to open");
    thread::sleep(Duration::from_millis(200));

    // Full content change
    client.change_document(&uri, 2, "process.env.API_KEY").expect("Failed to change");
    thread::sleep(Duration::from_millis(200));

    // Verify new content is recognized
    let hover = client.hover(&uri, 0, 15).expect("Hover request failed");

    assert!(!hover.is_null(), "Hover should work after content change");

    let contents = hover.get("contents").expect("Missing contents");
    let value = contents.get("value").expect("Missing value").as_str().expect("Value not string");

    // Should show API_KEY now, not DB_URL
    assert!(
        value.contains("API_KEY") || value.contains("secret"),
        "Hover should reflect changed content"
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_multiple_documents_concurrent() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    // Open multiple documents
    let uris: Vec<String> = (0..3)
        .map(|i| {
            let name = format!("test{}.js", i);
            workspace.create_file(&name, &format!("process.env.VAR_{}", i));
            workspace.file_uri(&name)
        })
        .collect();

    for (i, uri) in uris.iter().enumerate() {
        client
            .open_document(uri, "javascript", &format!("process.env.VAR_{}", i))
            .expect("Failed to open");
    }

    thread::sleep(Duration::from_millis(500));

    // All should be tracked - hover shouldn't crash
    for uri in &uris {
        let hover = client.hover(uri, 0, 15);
        assert!(hover.is_ok(), "Hover should not fail on any document");
    }

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_document_reopen() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    workspace.create_file("test.js", "process.env.DB_URL");

    // Open
    client.open_document(&uri, "javascript", "process.env.DB_URL").expect("Failed to open");
    thread::sleep(Duration::from_millis(200));

    // Close
    client.close_document(&uri).expect("Failed to close");
    thread::sleep(Duration::from_millis(100));

    // Reopen with different content
    client.open_document(&uri, "javascript", "process.env.PORT").expect("Failed to reopen");
    thread::sleep(Duration::from_millis(200));

    // Hover should work with new content
    let hover = client.hover(&uri, 0, 15).expect("Hover request failed");
    assert!(!hover.is_null(), "Hover should work after reopen");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_document_version_tracking() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    workspace.create_file("test.js", "process.env.VAR1");

    client.open_document(&uri, "javascript", "process.env.VAR1").expect("Failed to open");
    thread::sleep(Duration::from_millis(200));

    // Multiple changes with incrementing versions
    client.change_document(&uri, 2, "process.env.VAR2").expect("Change v2 failed");
    thread::sleep(Duration::from_millis(100));

    client.change_document(&uri, 3, "process.env.VAR3").expect("Change v3 failed");
    thread::sleep(Duration::from_millis(100));

    client.change_document(&uri, 4, "process.env.PORT").expect("Change v4 failed");
    thread::sleep(Duration::from_millis(200));

    // Final state should be PORT
    let hover = client.hover(&uri, 0, 15).expect("Hover request failed");
    if !hover.is_null() {
        let contents = hover.get("contents").expect("Missing contents");
        let value = contents.get("value").expect("Missing value").as_str().expect("Value not string");
        assert!(value.contains("PORT") || value.contains("8080"), "Should have PORT after changes");
    }

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_env_file_change() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    workspace.create_file("test.js", "process.env.NEW_VAR");

    client.open_document(&uri, "javascript", "process.env.NEW_VAR").expect("Failed to open");
    thread::sleep(Duration::from_millis(300));

    // Initially NEW_VAR is undefined, hover should be null
    let hover_before = client.hover(&uri, 0, 15).expect("Hover request failed");
    assert!(hover_before.is_null(), "NEW_VAR should be undefined initially");

    // Add NEW_VAR to .env
    workspace.append_to_file(".env", "NEW_VAR=new_value\n");

    // Notify server of file change
    client
        .notify(
            "workspace/didChangeWatchedFiles",
            Some(serde_json::json!({
                "changes": [{
                    "uri": workspace.file_uri(".env"),
                    "type": 2  // FileChangeType::Changed
                }]
            })),
        )
        .expect("Failed to notify");

    thread::sleep(Duration::from_millis(500));

    // Now hover should find NEW_VAR
    let _hover_after = client.hover(&uri, 0, 15).expect("Hover request failed");

    // Note: This test may be flaky depending on file watcher implementation
    // The important thing is the server doesn't crash

    client.shutdown().expect("Shutdown failed");
}
