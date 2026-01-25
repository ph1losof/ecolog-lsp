

use crate::harness::{LspTestClient, TempWorkspace};
use std::thread;
use std::time::Duration;

#[test]
fn test_hover_direct_env_reference_js() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const url = process.env.DB_URL;";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");

    
    thread::sleep(Duration::from_millis(300));

    
    let hover = client.hover(&uri, 0, 24).expect("Hover request failed");

    assert!(!hover.is_null(), "Expected hover result for DB_URL");

    let contents = hover.get("contents").expect("Missing contents");
    let value = contents.get("value").expect("Missing value").as_str().expect("Value not string");

    assert!(value.contains("DB_URL"), "Hover should contain variable name");
    assert!(value.contains("postgres://"));

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_hover_bracket_notation_js() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const key = process.env['API_KEY'];";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    
    let hover = client.hover(&uri, 0, 26).expect("Hover request failed");

    assert!(!hover.is_null(), "Expected hover result for API_KEY");

    let contents = hover.get("contents").expect("Missing contents");
    let value = contents.get("value").expect("Missing value").as_str().expect("Value not string");

    assert!(value.contains("secret_key"), "Hover should contain API_KEY value");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_hover_destructuring() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const { PORT } = process.env;";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    
    let hover = client.hover(&uri, 0, 8).expect("Hover request failed");

    assert!(!hover.is_null(), "Expected hover result for PORT");

    let contents = hover.get("contents").expect("Missing contents");
    let value = contents.get("value").expect("Missing value").as_str().expect("Value not string");

    assert!(value.contains("8080"), "Hover should contain PORT value");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_hover_object_alias_chain() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const e = process.env; const port = e.PORT;";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    
    let hover = client.hover(&uri, 0, 40).expect("Hover request failed");

    assert!(!hover.is_null(), "Expected hover result for PORT via alias");

    let contents = hover.get("contents").expect("Missing contents");
    let value = contents.get("value").expect("Missing value").as_str().expect("Value not string");

    assert!(value.contains("8080"), "Hover should contain PORT value");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_hover_undefined_returns_null() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.UNDEFINED_VAR";
    workspace.create_file("test.js", content);

    client.open_document(&uri, "javascript", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    let hover = client.hover(&uri, 0, 15).expect("Hover request failed");

    
    assert!(hover.is_null(), "Undefined vars should return null hover");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_hover_python() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.py");
    let content = "import os\ndb = os.environ['DB_URL']";
    workspace.create_file("test.py", content);

    client.open_document(&uri, "python", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    
    let hover = client.hover(&uri, 1, 18).expect("Hover request failed");

    assert!(!hover.is_null(), "Expected hover result for Python");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_hover_rust() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.rs");
    let content = r#"fn main() { std::env::var("PORT"); }"#;
    workspace.create_file("test.rs", content);

    client.open_document(&uri, "rust", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    
    let hover = client.hover(&uri, 0, 28).expect("Hover request failed");

    assert!(!hover.is_null(), "Expected hover result for Rust");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_hover_go() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.go");
    let content = r#"package main
import "os"
func main() { os.Getenv("DEBUG") }"#;
    workspace.create_file("test.go", content);

    client.open_document(&uri, "go", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    
    let hover = client.hover(&uri, 2, 26).expect("Hover request failed");

    assert!(!hover.is_null(), "Expected hover result for Go");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_hover_typescript() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.ts");
    let content = "const apiKey: string = process.env.API_KEY!;";
    workspace.create_file("test.ts", content);

    client.open_document(&uri, "typescript", content).expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    
    let hover = client.hover(&uri, 0, 38).expect("Hover request failed");

    assert!(!hover.is_null(), "Expected hover result for TypeScript");

    client.shutdown().expect("Shutdown failed");
}
