//! Tests for server/env_resolution.rs - Environment variable resolution at cursor position

mod common;

use common::TestFixture;
use ecolog_lsp::server::env_resolution::{resolve_env_var_at_position, EnvVarSource};
use tower_lsp::lsp_types::Position;

#[tokio::test]
async fn test_resolve_direct_reference() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const db = process.env.DB_URL;",
    );

    fixture
        .state
        .document_manager
        .open(uri.clone(), "javascript".into(), "const db = process.env.DB_URL;".into(), 1)
        .await;

    // Position at "DB_URL" (character 23)
    let result = resolve_env_var_at_position(&uri, Position::new(0, 23), &fixture.state, false).await;

    assert!(result.is_some(), "Should resolve direct reference");
    let resolved = result.unwrap();
    assert_eq!(resolved.env_var_name.as_str(), "DB_URL");
    assert!(matches!(resolved.source, EnvVarSource::DirectReference));
}

#[tokio::test]
async fn test_resolve_local_binding() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const { DB_URL } = process.env;\nconsole.log(DB_URL);",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const { DB_URL } = process.env;\nconsole.log(DB_URL);".into(),
            1,
        )
        .await;

    // Position at the destructured binding "DB_URL" (line 0, character 8)
    let result = resolve_env_var_at_position(&uri, Position::new(0, 8), &fixture.state, false).await;

    assert!(result.is_some(), "Should resolve local binding");
    let resolved = result.unwrap();
    assert_eq!(resolved.env_var_name.as_str(), "DB_URL");
    assert!(matches!(resolved.source, EnvVarSource::LocalBinding { .. }));
}

#[tokio::test]
async fn test_resolve_local_usage() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const { DB_URL } = process.env;\nconsole.log(DB_URL);",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const { DB_URL } = process.env;\nconsole.log(DB_URL);".into(),
            1,
        )
        .await;

    // Position at the usage "DB_URL" (line 1, character 12)
    let result = resolve_env_var_at_position(&uri, Position::new(1, 12), &fixture.state, false).await;

    assert!(result.is_some(), "Should resolve local usage");
    let resolved = result.unwrap();
    assert_eq!(resolved.env_var_name.as_str(), "DB_URL");
    assert!(matches!(resolved.source, EnvVarSource::LocalUsage { .. }));
}

#[tokio::test]
async fn test_resolve_no_env_var_at_position() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const x = 42;");

    fixture
        .state
        .document_manager
        .open(uri.clone(), "javascript".into(), "const x = 42;".into(), 1)
        .await;

    // Position at "x" - not an env var
    let result = resolve_env_var_at_position(&uri, Position::new(0, 6), &fixture.state, false).await;

    assert!(result.is_none(), "Should not resolve non-env var");
}

#[tokio::test]
async fn test_resolve_typescript_direct_reference() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.ts",
        "const apiKey: string = process.env.API_KEY!;",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "typescript".into(),
            "const apiKey: string = process.env.API_KEY!;".into(),
            1,
        )
        .await;

    // Position at "API_KEY" (character 35)
    let result = resolve_env_var_at_position(&uri, Position::new(0, 35), &fixture.state, false).await;

    assert!(result.is_some(), "Should resolve TypeScript direct reference");
    let resolved = result.unwrap();
    assert_eq!(resolved.env_var_name.as_str(), "API_KEY");
}

#[tokio::test]
async fn test_resolve_python_environ_subscript() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.py",
        "import os\ndb_url = os.environ['DB_URL']",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".into(),
            "import os\ndb_url = os.environ['DB_URL']".into(),
            1,
        )
        .await;

    // Position at "DB_URL" (line 1, character 21)
    let result = resolve_env_var_at_position(&uri, Position::new(1, 21), &fixture.state, false).await;

    assert!(result.is_some(), "Should resolve Python environ subscript");
    let resolved = result.unwrap();
    assert_eq!(resolved.env_var_name.as_str(), "DB_URL");
}

#[tokio::test]
async fn test_resolve_python_getenv() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.py",
        "import os\napi_key = os.getenv('API_KEY')",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".into(),
            "import os\napi_key = os.getenv('API_KEY')".into(),
            1,
        )
        .await;

    // Position at "API_KEY" (line 1, character 21)
    let result = resolve_env_var_at_position(&uri, Position::new(1, 21), &fixture.state, false).await;

    assert!(result.is_some(), "Should resolve Python getenv");
    let resolved = result.unwrap();
    assert_eq!(resolved.env_var_name.as_str(), "API_KEY");
}

#[tokio::test]
async fn test_resolve_rust_std_env_var() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.rs",
        "fn main() { let db = std::env::var(\"DB_URL\"); }",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "rust".into(),
            "fn main() { let db = std::env::var(\"DB_URL\"); }".into(),
            1,
        )
        .await;

    // Position at "DB_URL" (character 36)
    let result = resolve_env_var_at_position(&uri, Position::new(0, 36), &fixture.state, false).await;

    assert!(result.is_some(), "Should resolve Rust std::env::var");
    let resolved = result.unwrap();
    assert_eq!(resolved.env_var_name.as_str(), "DB_URL");
}

#[tokio::test]
async fn test_resolve_go_os_getenv() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.go",
        "package main\nimport \"os\"\nfunc main() { db := os.Getenv(\"DB_URL\") }",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "go".into(),
            "package main\nimport \"os\"\nfunc main() { db := os.Getenv(\"DB_URL\") }".into(),
            1,
        )
        .await;

    // Position at "DB_URL" (line 2, character 32)
    let result = resolve_env_var_at_position(&uri, Position::new(2, 32), &fixture.state, false).await;

    assert!(result.is_some(), "Should resolve Go os.Getenv");
    let resolved = result.unwrap();
    assert_eq!(resolved.env_var_name.as_str(), "DB_URL");
}

#[tokio::test]
async fn test_resolve_with_cross_module_disabled() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "import env from './config';\nconst db = env.DB_URL;",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "import env from './config';\nconst db = env.DB_URL;".into(),
            1,
        )
        .await;

    // Position at "env" in "env.DB_URL" - cross-module disabled
    let result = resolve_env_var_at_position(&uri, Position::new(1, 11), &fixture.state, false).await;

    // Should be None since cross-module is disabled and this isn't a local env reference
    assert!(result.is_none(), "Cross-module should not resolve when disabled");
}
