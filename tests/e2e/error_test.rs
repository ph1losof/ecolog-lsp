

use crate::harness::{LspTestClient, TempWorkspace};
use serde_json::json;
use std::thread;
use std::time::Duration;

#[test]
fn test_invalid_params() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    
    let result = client.request(
        "textDocument/hover",
        Some(json!({
            "invalid": "params"
        })),
    );

    
    assert!(
        result.is_err() || result.as_ref().map(|v| v.is_null()).unwrap_or(false),
        "Invalid params should fail gracefully"
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_document_not_open() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    
    let hover = client.hover("file:

    assert!(hover.is_null(), "Hover on unopened document should return null");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_position_out_of_bounds() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    workspace.create_file("test.js", "a"); 

    client.open_document(&uri, "javascript", "a").expect("Failed to open document");
    thread::sleep(Duration::from_millis(200));

    
    let hover = client.hover(&uri, 100, 100).expect("Request should not fail");

    
    assert!(hover.is_null(), "Out of bounds position should return null");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_malformed_uri() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    
    let result = client.hover("not-a-valid-uri", 0, 0);

    
    match result {
        Ok(hover) => assert!(hover.is_null(), "Malformed URI should return null"),
        Err(e) => {
            
            let err_msg = e.to_string();
            assert!(err_msg.contains("-32602") || err_msg.contains("invalid") || err_msg.contains("URL"),
                   "Should be an invalid params error: {}", err_msg);
        }
    }

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_empty_document() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("empty.js");
    workspace.create_file("empty.js", "");

    client.open_document(&uri, "javascript", "").expect("Failed to open document");
    thread::sleep(Duration::from_millis(200));

    
    let hover = client.hover(&uri, 0, 0).expect("Request should not fail");
    assert!(hover.is_null());

    let completion = client.completion(&uri, 0, 0).expect("Request should not fail");
    assert!(completion.is_null() || completion.as_array().map(|a| a.is_empty()).unwrap_or(false));

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_binary_file_content() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("binary.js");
    
    workspace.create_file("binary.js", "\0\0\0");

    client.open_document(&uri, "javascript", "\0\0\0").expect("Failed to open document");
    thread::sleep(Duration::from_millis(200));

    
    let _hover = client.hover(&uri, 0, 0).expect("Request should not fail");
    

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_very_long_line() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("long.js");
    let long_line = "a".repeat(100_000);
    workspace.create_file("long.js", &long_line);

    client.open_document(&uri, "javascript", &long_line).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    
    let _hover = client.hover(&uri, 0, 99999).expect("Request should not fail");
    

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_rapid_document_changes() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("rapid.js");
    workspace.create_file("rapid.js", "");

    client.open_document(&uri, "javascript", "").expect("Failed to open document");

    
    for i in 1..=20 {
        let content = format!("process.env.VAR_{}", i);
        client.change_document(&uri, i, &content).expect("Change should not fail");
    }

    thread::sleep(Duration::from_millis(500));

    
    let _hover = client.hover(&uri, 0, 15).expect("Hover should work after rapid changes");
    

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_unicode_in_document() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("unicode.js");
    let content = "
    workspace.create_file("unicode.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    
    let _hover = client.hover(&uri, 1, 15).expect("Request should not fail");
    

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_deeply_nested_code() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("nested.js");
    
    let mut content = String::new();
    for _ in 0..50 {
        content.push_str("if (true) { ");
    }
    content.push_str("process.env.DB_URL");
    for _ in 0..50 {
        content.push_str(" }");
    }

    workspace.create_file("nested.js", &content);
    client.open_document(&uri, "javascript", &content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(500));

    
    let _hover = client.hover(&uri, 0, 600).expect("Request should not fail");
    

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_command_with_wrong_arg_types() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    
    let result = client.execute_command("ecolog.file.setActive", vec![json!(12345)]);

    
    assert!(result.is_ok(), "Command should not crash with wrong arg types");

    client.shutdown().expect("Shutdown failed");
}
