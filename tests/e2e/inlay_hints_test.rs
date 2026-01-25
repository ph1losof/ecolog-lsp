use crate::harness::{LspTestClient, TempWorkspace};
use std::thread;
use std::time::Duration;

#[test]
fn test_inlay_hints_direct_reference() {
    let workspace = TempWorkspace::new();
    workspace.create_config("[features]\ninlay_hints = true");
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const url = process.env.DB_URL;";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");

    thread::sleep(Duration::from_millis(300));

    let hints = client.inlay_hint(&uri, 0, 0, 0, 50).expect("Inlay hint request failed");

    // Should return an array of hints
    assert!(hints.is_array(), "Expected array of inlay hints");

    let hints_arr = hints.as_array().unwrap();
    assert!(!hints_arr.is_empty(), "Expected at least one inlay hint for DB_URL");

    // Check that the hint contains expected content
    let first_hint = &hints_arr[0];
    let label = first_hint.get("label").expect("Missing label");
    let label_str = label.as_str().expect("Label not a string");
    assert!(label_str.contains('"'), "Hint should contain quoted value");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_inlay_hints_destructuring() {
    let workspace = TempWorkspace::new();
    workspace.create_config("[features]\ninlay_hints = true");
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const { PORT, API_KEY } = process.env;";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");

    thread::sleep(Duration::from_millis(300));

    let hints = client.inlay_hint(&uri, 0, 0, 0, 50).expect("Inlay hint request failed");

    assert!(hints.is_array(), "Expected array of inlay hints");

    let hints_arr = hints.as_array().unwrap();
    // Should have hints for both PORT and API_KEY
    assert!(hints_arr.len() >= 2, "Expected at least two inlay hints for destructuring");

    client.shutdown().expect("Shutdown failed");
}

#[test]
#[ignore = "Alias property access not yet tracked in env_var_index for inlay hints"]
fn test_inlay_hints_alias_property_access() {
    let workspace = TempWorkspace::new();
    workspace.create_config("[features]\ninlay_hints = true");
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const e = process.env; const port = e.PORT;";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");

    thread::sleep(Duration::from_millis(300));

    let hints = client.inlay_hint(&uri, 0, 0, 0, 60).expect("Inlay hint request failed");

    assert!(hints.is_array(), "Expected array of inlay hints");

    let hints_arr = hints.as_array().unwrap();
    // Should have hint for PORT via property access on alias
    assert!(!hints_arr.is_empty(), "Expected inlay hint for property access via alias");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_inlay_hints_empty_for_no_env_vars() {
    let workspace = TempWorkspace::new();
    workspace.create_config("[features]\ninlay_hints = true");
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const x = 42; function foo() { return x * 2; }";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");

    thread::sleep(Duration::from_millis(300));

    let hints = client.inlay_hint(&uri, 0, 0, 0, 60).expect("Inlay hint request failed");

    assert!(hints.is_array(), "Expected array of inlay hints");

    let hints_arr = hints.as_array().unwrap();
    assert!(hints_arr.is_empty(), "Expected no inlay hints for code without env vars");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_inlay_hints_tooltip_shows_source() {
    let workspace = TempWorkspace::new();
    workspace.create_config("[features]\ninlay_hints = true");
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const port = process.env.PORT;";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");

    thread::sleep(Duration::from_millis(300));

    let hints = client.inlay_hint(&uri, 0, 0, 0, 50).expect("Inlay hint request failed");

    assert!(hints.is_array(), "Expected array of inlay hints");

    let hints_arr = hints.as_array().unwrap();
    assert!(!hints_arr.is_empty(), "Expected at least one inlay hint");

    // Check that the tooltip contains source info
    let first_hint = &hints_arr[0];
    let tooltip = first_hint.get("tooltip").expect("Missing tooltip");
    let tooltip_str = tooltip.as_str().expect("Tooltip not a string");
    assert!(
        tooltip_str.contains("Source:"),
        "Tooltip should indicate source. Got: {}",
        tooltip_str
    );

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_inlay_hints_typescript() {
    let workspace = TempWorkspace::new();
    workspace.create_config("[features]\ninlay_hints = true");
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.ts");
    let content = "const port: string = process.env.PORT!;";
    workspace.create_file("test.ts", content);

    client
        .open_document(&uri, "typescript", content)
        .expect("Failed to open document");

    thread::sleep(Duration::from_millis(300));

    let hints = client.inlay_hint(&uri, 0, 0, 0, 50).expect("Inlay hint request failed");

    assert!(hints.is_array(), "Expected array of inlay hints for TypeScript");

    let hints_arr = hints.as_array().unwrap();
    assert!(!hints_arr.is_empty(), "Expected at least one inlay hint for TypeScript");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_inlay_hints_python() {
    let workspace = TempWorkspace::new();
    workspace.create_config("[features]\ninlay_hints = true");
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.py");
    let content = "import os\ndb = os.environ['DB_URL']";
    workspace.create_file("test.py", content);

    client
        .open_document(&uri, "python", content)
        .expect("Failed to open document");

    thread::sleep(Duration::from_millis(300));

    let hints = client.inlay_hint(&uri, 0, 0, 2, 0).expect("Inlay hint request failed");

    assert!(hints.is_array(), "Expected array of inlay hints for Python");

    let hints_arr = hints.as_array().unwrap();
    assert!(!hints_arr.is_empty(), "Expected at least one inlay hint for Python");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_inlay_hints_range_filtering() {
    let workspace = TempWorkspace::new();
    workspace.create_config("[features]\ninlay_hints = true");
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    // Multiple env vars on different lines
    let content = "const a = process.env.PORT;\nconst b = process.env.DEBUG;";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");

    thread::sleep(Duration::from_millis(300));

    // Request hints only for line 0
    let hints = client.inlay_hint(&uri, 0, 0, 0, 50).expect("Inlay hint request failed");

    assert!(hints.is_array(), "Expected array of inlay hints");

    let hints_arr = hints.as_array().unwrap();
    // Should only have hint for PORT (line 0), not DEBUG (line 1)
    for hint in hints_arr {
        let position = hint.get("position").expect("Missing position");
        let line = position.get("line").expect("Missing line").as_u64().unwrap();
        assert_eq!(line, 0, "All hints should be on line 0 given the requested range");
    }

    client.shutdown().expect("Shutdown failed");
}
