

mod common;

use common::TestFixture;
use ecolog_lsp::server::handlers::{handle_references, handle_rename, handle_prepare_rename};
use tower_lsp::lsp_types::{
    Position, ReferenceContext, ReferenceParams, RenameParams, TextDocumentIdentifier,
    TextDocumentPositionParams,
};





#[tokio::test]
async fn test_find_references_direct_reference() {
    let fixture = TestFixture::new().await;

    
    let uri = fixture.create_file(
        "test.js",
        "const url = process.env.DB_URL;\nconsole.log(process.env.DB_URL);",
    );

    
    fixture.index_workspace().await;

    
    fixture
        .state
        .document_manager
        .open(uri.clone(), "javascript".to_string(),
              "const url = process.env.DB_URL;\nconsole.log(process.env.DB_URL);".to_string(), 1)
        .await;

    
    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 0,
                character: 24, 
            },
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: true,
        },
    };

    let result = handle_references(params, &fixture.state).await;

    assert!(result.is_some(), "Expected to find references");
    let locations = result.unwrap();

    
    assert!(
        locations.len() >= 2,
        "Expected at least 2 references, found {}",
        locations.len()
    );
}

#[tokio::test]
async fn test_find_references_across_files() {
    let fixture = TestFixture::new().await;

    
    fixture.create_file("a.js", "const key = process.env.API_KEY;");
    fixture.create_file("b.ts", "const apiKey = process.env.API_KEY;");

    
    fixture.index_workspace().await;

    
    let uri_a = fixture.create_file("a.js", "const key = process.env.API_KEY;");
    fixture
        .state
        .document_manager
        .open(
            uri_a.clone(),
            "javascript".to_string(),
            "const key = process.env.API_KEY;".to_string(),
            1,
        )
        .await;

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri_a },
            position: Position {
                line: 0,
                character: 24, 
            },
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: true,
        },
    };

    let result = handle_references(params, &fixture.state).await;

    assert!(result.is_some(), "Expected to find references");
    let locations = result.unwrap();

    
    assert!(
        locations.len() >= 2,
        "Expected at least 2 references across files, found {}",
        locations.len()
    );
}

#[tokio::test]
async fn test_find_references_no_refs_for_unknown_var() {
    let fixture = TestFixture::new().await;

    let uri = fixture.create_file("test.js", "const x = process.env.UNKNOWN_VAR;");

    fixture.index_workspace().await;

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const x = process.env.UNKNOWN_VAR;".to_string(),
            1,
        )
        .await;

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 0,
                character: 22, 
            },
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    };

    let result = handle_references(params, &fixture.state).await;

    
    assert!(result.is_some());
}





#[tokio::test]
async fn test_prepare_rename_valid_env_var() {
    let fixture = TestFixture::new().await;

    let uri = fixture.create_file("test.js", "const url = process.env.DB_URL;");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const url = process.env.DB_URL;".to_string(),
            1,
        )
        .await;

    let params = TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri },
        position: Position {
            line: 0,
            character: 24,
        },
    };

    let result = handle_prepare_rename(params, &fixture.state).await;

    assert!(result.is_some(), "Prepare rename should succeed for valid env var");
}

#[tokio::test]
async fn test_rename_env_var() {
    let fixture = TestFixture::new().await;

    let uri = fixture.create_file(
        "test.js",
        "const url = process.env.DB_URL;\nconst x = process.env.DB_URL;",
    );

    fixture.index_workspace().await;

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const url = process.env.DB_URL;\nconst x = process.env.DB_URL;".to_string(),
            1,
        )
        .await;

    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 0,
                character: 24,
            },
        },
        new_name: "DATABASE_URL".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = handle_rename(params, &fixture.state).await;

    assert!(result.is_some(), "Rename should return edits");
    let edit = result.unwrap();

    assert!(edit.changes.is_some(), "WorkspaceEdit should have changes");
    let changes = edit.changes.unwrap();

    
    assert!(!changes.is_empty(), "Should have edits for at least one file");
}

#[tokio::test]
async fn test_rename_invalid_new_name() {
    let fixture = TestFixture::new().await;

    let uri = fixture.create_file("test.js", "const url = process.env.DB_URL;");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const url = process.env.DB_URL;".to_string(),
            1,
        )
        .await;

    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 0,
                character: 24,
            },
        },
        new_name: "123_INVALID".to_string(), 
        work_done_progress_params: Default::default(),
    };

    let result = handle_rename(params, &fixture.state).await;

    assert!(result.is_none(), "Rename with invalid name should return None");
}





#[tokio::test]
async fn test_prepare_rename_in_env_file() {
    let fixture = TestFixture::new().await;

    
    let env_uri = fixture.create_file(".env", "API_KEY=secret123\nDB_URL=postgres://localhost");


    fixture
        .state
        .document_manager
        .open(
            env_uri.clone(),
            "plaintext".to_string(),
            "API_KEY=secret123\nDB_URL=postgres://localhost".to_string(),
            1,
        )
        .await;

    
    let params = TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: env_uri },
        position: Position {
            line: 0,
            character: 3, 
        },
    };

    let result = handle_prepare_rename(params, &fixture.state).await;

    assert!(result.is_some(), "Prepare rename should succeed for env var in .env file");
}

#[tokio::test]
async fn test_rename_from_env_file() {
    let fixture = TestFixture::new().await;

    
    let env_uri = fixture.create_file(".env", "API_KEY=secret123");

    
    let js_uri = fixture.create_file("app.js", "const key = process.env.API_KEY;");
    let ts_uri = fixture.create_file("config.ts", "const apiKey = process.env.API_KEY;");

    
    fixture.index_workspace().await;

    
    fixture
        .state
        .document_manager
        .open(
            env_uri.clone(),
            "plaintext".to_string(),
            "API_KEY=secret123".to_string(),
            1,
        )
        .await;
    fixture
        .state
        .document_manager
        .open(
            js_uri.clone(),
            "javascript".to_string(),
            "const key = process.env.API_KEY;".to_string(),
            1,
        )
        .await;
    fixture
        .state
        .document_manager
        .open(
            ts_uri.clone(),
            "typescript".to_string(),
            "const apiKey = process.env.API_KEY;".to_string(),
            1,
        )
        .await;

    
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: env_uri.clone() },
            position: Position {
                line: 0,
                character: 3, 
            },
        },
        new_name: "AUTH_TOKEN".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = handle_rename(params, &fixture.state).await;

    assert!(result.is_some(), "Rename from .env file should return edits");
    let edit = result.unwrap();

    assert!(edit.changes.is_some(), "WorkspaceEdit should have changes");
    let changes = edit.changes.unwrap();

    
    assert!(!changes.is_empty(), "Should have edits for at least one file");

    
    assert!(
        changes.contains_key(&env_uri),
        "Changes should include the .env file"
    );
}

#[tokio::test]
async fn test_rename_from_env_file_updates_code_files() {
    let fixture = TestFixture::new().await;

    
    let env_uri = fixture.create_file(".env", "DB_HOST=localhost");

    
    let js_content = "const host = process.env.DB_HOST;\nconst url = `http://${host}`;";
    let js_uri = fixture.create_file("server.js", js_content);

    
    fixture.index_workspace().await;

    
    fixture
        .state
        .document_manager
        .open(
            env_uri.clone(),
            "plaintext".to_string(),
            "DB_HOST=localhost".to_string(),
            1,
        )
        .await;
    fixture
        .state
        .document_manager
        .open(
            js_uri.clone(),
            "javascript".to_string(),
            js_content.to_string(),
            1,
        )
        .await;

    
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: env_uri.clone() },
            position: Position {
                line: 0,
                character: 2, 
            },
        },
        new_name: "DATABASE_HOST".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = handle_rename(params, &fixture.state).await;

    assert!(result.is_some(), "Rename should succeed");
    let edit = result.unwrap();
    let changes = edit.changes.expect("Should have changes");

    
    if changes.contains_key(&js_uri) {
        let js_edits = &changes[&js_uri];
        
        assert!(
            js_edits.len() >= 1,
            "JS file should have at least 1 edit"
        );
    }

    
    assert!(
        changes.contains_key(&env_uri),
        ".env file should be in changes"
    );
}





#[tokio::test]
async fn test_workspace_index_stats() {
    let fixture = TestFixture::new().await;

    
    fixture.create_file("a.js", "process.env.API_KEY");
    fixture.create_file("b.ts", "process.env.DB_URL");
    fixture.create_file("c.py", "os.environ['DEBUG']");

    fixture.index_workspace().await;

    let stats = fixture.state.workspace_index.stats();

    
    assert!(stats.total_files >= 1, "Should have indexed at least 1 file");
    assert!(
        stats.total_env_vars >= 1,
        "Should have indexed at least 1 env var"
    );
}

#[tokio::test]
async fn test_workspace_index_files_for_env_var() {
    let fixture = TestFixture::new().await;

    
    fixture.create_file("a.js", "const k = process.env.API_KEY;");
    fixture.create_file("b.ts", "const key = process.env.API_KEY;");
    fixture.create_file("c.js", "const port = process.env.PORT;"); 

    fixture.index_workspace().await;

    let api_key_files = fixture.state.workspace_index.files_for_env_var("API_KEY");

    
    assert!(
        api_key_files.len() >= 2,
        "Expected at least 2 files with API_KEY, found {}",
        api_key_files.len()
    );
}



#[tokio::test]
async fn test_rename_does_not_affect_similar_names() {
    let fixture = TestFixture::new().await;

    
    let env_uri = fixture.create_file(".env", "DEBUG=on\nDEBUGHAT=value");

    
    let js_content = "const d = process.env.DEBUG;\nconst dh = process.env.DEBUGHAT;";
    let js_uri = fixture.create_file("app.js", js_content);

    
    fixture.index_workspace().await;

    
    fixture
        .state
        .document_manager
        .open(
            env_uri.clone(),
            "plaintext".to_string(),
            "DEBUG=on\nDEBUGHAT=value".to_string(),
            1,
        )
        .await;
    fixture
        .state
        .document_manager
        .open(
            js_uri.clone(),
            "javascript".to_string(),
            js_content.to_string(),
            1,
        )
        .await;

    
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: env_uri.clone() },
            position: Position {
                line: 0,
                character: 0, 
            },
        },
        new_name: "SOMEWHAT".to_string(),
        work_done_progress_params: Default::default(),
    };

    let result = handle_rename(params, &fixture.state).await;

    assert!(result.is_some(), "Rename should succeed");
    let edit = result.unwrap();
    let changes = edit.changes.expect("Should have changes");

    
    let env_edits = changes.get(&env_uri).expect(".env should have edits");
    assert_eq!(env_edits.len(), 1, "Should only rename DEBUG, not DEBUGHAT");

    
    let first_edit = &env_edits[0];
    assert_eq!(first_edit.new_text, "SOMEWHAT");
    assert_eq!(first_edit.range.start.line, 0);
    assert_eq!(first_edit.range.start.character, 0);
    assert_eq!(first_edit.range.end.line, 0);
    assert_eq!(first_edit.range.end.character, 5); 

    
    if let Some(js_edits) = changes.get(&js_uri) {
        assert_eq!(js_edits.len(), 1, "Should only rename DEBUG reference, not DEBUGHAT");
        let js_edit = &js_edits[0];
        assert_eq!(js_edit.new_text, "SOMEWHAT");
        
    }
}
