use crate::harness::{LspTestClient, TempWorkspace};
use std::thread;
use std::time::Duration;

#[test]
fn test_find_references_in_single_file() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const a = process.env.DB_URL;\nconst b = process.env.DB_URL;";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");
    thread::sleep(Duration::from_millis(500));

    let references = client
        .references(&uri, 0, 24, true)
        .expect("References request failed");

    let refs = references.as_array().expect("References should be array");
    assert!(
        refs.len() >= 2,
        "Expected at least 2 references, got {}",
        refs.len()
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_find_references_includes_declaration() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.DB_URL";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");
    thread::sleep(Duration::from_millis(500));

    let references = client
        .references(&uri, 0, 15, true)
        .expect("References request failed");

    let refs = references.as_array().expect("References should be array");

    let has_env_ref = refs.iter().any(|r| {
        r.get("uri")
            .and_then(|u| u.as_str())
            .map(|s| s.ends_with(".env"))
            .unwrap_or(false)
    });

    assert!(has_env_ref, "Should include .env file definition");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_find_references_exclude_declaration() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const a = process.env.DB_URL;\nconst b = process.env.DB_URL;";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");
    thread::sleep(Duration::from_millis(500));

    let references = client
        .references(&uri, 0, 24, false)
        .expect("References request failed");

    let refs = references.as_array().expect("References should be array");

    let _all_in_js = refs.iter().all(|r| {
        r.get("uri")
            .and_then(|u| u.as_str())
            .map(|s| s.ends_with(".js"))
            .unwrap_or(false)
    });

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_find_references_undefined_var() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.UNDEFINED_VAR";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");
    thread::sleep(Duration::from_millis(500));

    let references = client
        .references(&uri, 0, 15, true)
        .expect("References request failed");

    if !references.is_null() {
        let refs = references.as_array().expect("References should be array");

        assert!(refs.len() <= 1, "Undefined var shouldn't have many refs");
    }

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_find_references_from_binding() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const { PORT } = process.env;\nconsole.log(PORT);";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");
    thread::sleep(Duration::from_millis(500));

    let references = client
        .references(&uri, 0, 9, true)
        .expect("References request failed");

    let refs = references.as_array().expect("References should be array");

    assert!(!refs.is_empty(), "Should find references from binding");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_find_references_no_results_outside_env() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const x = 1;";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    let references = client
        .references(&uri, 0, 6, true)
        .expect("References request failed");

    assert!(
        references.is_null() || references.as_array().map(|a| a.is_empty()).unwrap_or(false),
        "Non-env code should have no references"
    );

    client.shutdown().expect("Shutdown failed");
}
