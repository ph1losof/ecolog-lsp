use crate::harness::{LspTestClient, TempWorkspace};
use std::thread;
use std::time::Duration;

#[test]
fn test_prepare_rename_valid() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "const url = process.env.DB_URL;";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    let prepare = client
        .prepare_rename(&uri, 0, 26)
        .expect("PrepareRename request failed");

    assert!(!prepare.is_null(), "prepareRename should succeed");

    let start = prepare.get("start").expect("Should have start");
    let end = prepare.get("end").expect("Should have end");

    assert_eq!(start.get("line").unwrap().as_i64(), Some(0));
    assert!(start.get("character").unwrap().as_i64().unwrap() >= 24);
    assert!(end.get("character").unwrap().as_i64().unwrap() >= 30);

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_rename_env_var() {
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

    let edit = client
        .rename(&uri, 0, 24, "DATABASE_URL")
        .expect("Rename request failed");

    assert!(!edit.is_null(), "Rename should return edit");

    let changes = edit.get("changes").expect("Should have changes");
    assert!(
        !changes.as_object().unwrap().is_empty(),
        "Changes should not be empty"
    );

    for (file_uri, file_edits) in changes.as_object().unwrap() {
        let edits = file_edits.as_array().expect("Edits should be array");
        if file_uri.ends_with(".js") {
            assert!(
                edits.len() >= 2,
                "Should have edits for both occurrences in JS file"
            );
        }
    }

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_prepare_rename_undefined_var() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.UNDEFINED_VAR";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");
    thread::sleep(Duration::from_millis(300));

    let _prepare = client
        .prepare_rename(&uri, 0, 15)
        .expect("PrepareRename request failed");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_prepare_rename_outside_env() {
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

    let prepare = client
        .prepare_rename(&uri, 0, 6)
        .expect("PrepareRename request failed");

    assert!(prepare.is_null(), "Non-env code should not be renameable");

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_rename_updates_env_file() {
    let workspace = TempWorkspace::new();
    let client = LspTestClient::spawn(workspace.root.clone()).expect("Failed to spawn LSP");
    client.initialize().expect("Initialize failed");

    let uri = workspace.file_uri("test.js");
    let content = "process.env.API_KEY";
    workspace.create_file("test.js", content);

    client
        .open_document(&uri, "javascript", content)
        .expect("Failed to open document");
    thread::sleep(Duration::from_millis(500));

    let edit = client
        .rename(&uri, 0, 15, "AUTH_TOKEN")
        .expect("Rename request failed");

    if !edit.is_null() {
        let changes = edit.get("changes");
        if let Some(changes) = changes {
            let has_env_edits = changes
                .as_object()
                .unwrap()
                .keys()
                .any(|k| k.ends_with(".env"));
            assert!(has_env_edits, "Rename should update .env file");
        }
    }

    client.shutdown().expect("Shutdown failed");
}

#[test]
fn test_rename_from_binding() {
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

    let edit = client
        .rename(&uri, 0, 9, "HTTP_PORT")
        .expect("Rename request failed");

    assert!(!edit.is_null(), "Rename from binding should work");

    client.shutdown().expect("Shutdown failed");
}
