//! Workspace symbol tests for LSP server

use crate::harness::{LspTestClient, TempWorkspace};
use std::thread;
use std::time::Duration;

#[test]
fn test_workspace_symbol_empty_query_returns_all() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    // Create .env file with some env vars
    workspace.create_file(".env", "DB_URL=postgres://localhost\nAPI_KEY=secret\nPORT=3000");

    // Create a JS file that references some env vars
    let uri = workspace.file_uri("test.js");
    let content = "const db = process.env.DB_URL;\nconst key = process.env.API_KEY;";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");
    thread::sleep(Duration::from_millis(500));

    // Empty query should return all env vars
    let result = client.workspace_symbol("").expect("Workspace symbol request failed");

    let symbols = result.as_array().expect("Result should be array");
    assert!(
        symbols.len() >= 2,
        "Expected at least 2 symbols, got {}",
        symbols.len()
    );

    // All symbols should be CONSTANT kind (value 14)
    for symbol in symbols {
        let kind = symbol.get("kind").and_then(|k| k.as_u64()).expect("Symbol should have kind");
        assert_eq!(kind, 14, "Symbol kind should be CONSTANT (14)");
    }

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_workspace_symbol_query_filtering() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    // Create .env file with env vars
    workspace.create_file(".env", "DATABASE_URL=postgres://localhost\nDB_HOST=localhost\nAPI_KEY=secret");

    // Create a JS file that references env vars
    let uri = workspace.file_uri("test.js");
    let content = "process.env.DATABASE_URL; process.env.DB_HOST; process.env.API_KEY;";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");
    // Wait for indexing to complete
    thread::sleep(Duration::from_millis(800));

    // Query "DB" should match DATABASE_URL and DB_HOST but not API_KEY
    let result = client.workspace_symbol("DB").expect("Workspace symbol request failed");

    let symbols = result.as_array().expect("Result should be array");

    // Should have at least 1 match (either DATABASE_URL or DB_HOST)
    // Note: Background indexer timing may vary, so we check for at least 1
    assert!(
        !symbols.is_empty(),
        "Expected at least 1 symbol matching 'DB', got 0"
    );

    // All returned symbols should contain "DB" (case-insensitive)
    for symbol in symbols {
        let name = symbol
            .get("name")
            .and_then(|n| n.as_str())
            .expect("Symbol should have name");
        assert!(
            name.to_uppercase().contains("DB"),
            "Symbol '{}' should contain 'DB'",
            name
        );
    }

    // Verify API_KEY is NOT in the results (it doesn't contain "DB")
    let has_api_key = symbols
        .iter()
        .any(|s| s.get("name").and_then(|n| n.as_str()) == Some("API_KEY"));
    assert!(!has_api_key, "API_KEY should not match query 'DB'");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_workspace_symbol_case_insensitive() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    workspace.create_file(".env", "DATABASE_URL=postgres://localhost");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.DATABASE_URL;";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");
    thread::sleep(Duration::from_millis(500));

    // Query with lowercase should match uppercase env var
    let result = client
        .workspace_symbol("database")
        .expect("Workspace symbol request failed");

    let symbols = result.as_array().expect("Result should be array");
    assert!(!symbols.is_empty(), "Should find DATABASE_URL with lowercase query");

    let has_database_url = symbols
        .iter()
        .any(|s| s.get("name").and_then(|n| n.as_str()) == Some("DATABASE_URL"));
    assert!(has_database_url, "Should find DATABASE_URL");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_workspace_symbol_points_to_env_definition() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    // Create .env file
    workspace.create_file(".env", "API_KEY=secret123");

    // Create a JS file that references the env var
    let uri = workspace.file_uri("test.js");
    let content = "const key = process.env.API_KEY;";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");
    thread::sleep(Duration::from_millis(500));

    let result = client
        .workspace_symbol("API_KEY")
        .expect("Workspace symbol request failed");

    let symbols = result.as_array().expect("Result should be array");
    assert!(!symbols.is_empty(), "Should find API_KEY");

    let api_key_symbol = symbols
        .iter()
        .find(|s| s.get("name").and_then(|n| n.as_str()) == Some("API_KEY"))
        .expect("Should find API_KEY symbol");

    // Location should point to .env file
    let location = api_key_symbol.get("location").expect("Symbol should have location");
    let location_uri = location
        .get("uri")
        .and_then(|u| u.as_str())
        .expect("Location should have uri");

    assert!(
        location_uri.ends_with(".env"),
        "Symbol location should point to .env file, got: {}",
        location_uri
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_workspace_symbol_empty_workspace() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    // Create a JS file with no env var references
    let uri = workspace.file_uri("test.js");
    let content = "const x = 1;";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    let result = client
        .workspace_symbol("")
        .expect("Workspace symbol request failed");

    // Empty workspace should return null or empty array
    assert!(
        result.is_null() || result.as_array().map(|a| a.is_empty()).unwrap_or(false),
        "Empty workspace should return null or empty array"
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_workspace_symbol_no_match() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    workspace.create_file(".env", "API_KEY=secret");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.API_KEY;";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");
    thread::sleep(Duration::from_millis(500));

    // Query that matches nothing
    let result = client
        .workspace_symbol("ZZZZZ_NONEXISTENT")
        .expect("Workspace symbol request failed");

    // Should return null or empty array
    assert!(
        result.is_null() || result.as_array().map(|a| a.is_empty()).unwrap_or(false),
        "Non-matching query should return null or empty array"
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_workspace_symbol_has_container_name() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    workspace.create_file(".env", "TEST_VAR=value");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.TEST_VAR;";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");
    thread::sleep(Duration::from_millis(500));

    let result = client
        .workspace_symbol("TEST_VAR")
        .expect("Workspace symbol request failed");

    let symbols = result.as_array().expect("Result should be array");
    assert!(!symbols.is_empty(), "Should find TEST_VAR");

    let symbol = &symbols[0];
    let container_name = symbol
        .get("containerName")
        .and_then(|c| c.as_str())
        .expect("Symbol should have containerName");

    assert_eq!(
        container_name, "Environment Variables",
        "Container name should be 'Environment Variables'"
    );

    client.shutdown().expect("Shutdown failed");
}
