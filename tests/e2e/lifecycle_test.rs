

use crate::harness::{LspTestClient, TempWorkspace};
use serde_json::json;

#[test]
fn test_initialize_response_capabilities() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");

    let result = client.initialize().expect("Initialize failed");

    
    let capabilities = result.get("capabilities").expect("Missing capabilities");

    assert!(capabilities.get("hoverProvider").is_some(), "Missing hoverProvider");
    assert!(capabilities.get("completionProvider").is_some(), "Missing completionProvider");
    assert!(capabilities.get("definitionProvider").is_some(), "Missing definitionProvider");
    assert!(capabilities.get("referencesProvider").is_some(), "Missing referencesProvider");
    assert!(capabilities.get("renameProvider").is_some(), "Missing renameProvider");

    
    let rename = capabilities.get("renameProvider").unwrap();
    assert_eq!(rename.get("prepareProvider"), Some(&json!(true)));

    
    let sync = capabilities.get("textDocumentSync").unwrap();
    assert_eq!(sync.as_i64(), Some(1), "Expected TextDocumentSyncKind::FULL");

    
    let commands = capabilities
        .get("executeCommandProvider")
        .expect("Missing executeCommandProvider")
        .get("commands")
        .expect("Missing commands")
        .as_array()
        .expect("Commands should be array");

    assert!(
        commands.iter().any(|c| c == "ecolog.file.setActive"),
        "Missing ecolog.file.setActive command"
    );
    assert!(
        commands.iter().any(|c| c == "ecolog.listEnvVariables"),
        "Missing ecolog.listEnvVariables command"
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_shutdown_and_exit() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");

    client.initialize().expect("Initialize failed");

    
    let result = client.request("shutdown", None);
    assert!(result.is_ok(), "Shutdown request failed");

    
    let _ = client.notify("exit", None);

    
    std::thread::sleep(std::time::Duration::from_millis(100));
}

#[test]
fn test_initialization_trigger_characters() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");

    let result = client.initialize().expect("Initialize failed");

    let capabilities = result.get("capabilities").expect("Missing capabilities");
    let completion = capabilities.get("completionProvider").expect("Missing completionProvider");
    let triggers = completion.get("triggerCharacters");

    
    assert!(triggers.is_some(), "Should have triggerCharacters");
    let triggers = triggers.unwrap().as_array().expect("triggerCharacters should be array");

    
    assert!(
        triggers.iter().any(|t| t == "."),
        "Should include '.' as trigger character"
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_double_initialize_fails() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");

    
    client.initialize().expect("First initialize failed");

    
    let result = client.request(
        "initialize",
        Some(json!({
            "processId": std::process::id(),
            "rootUri": format!("file://{}", workspace.root.display()),
            "capabilities": {}
        })),
    );

    
    assert!(result.is_err(), "Second initialize should fail");

    client.shutdown().expect("Shutdown failed");
}
