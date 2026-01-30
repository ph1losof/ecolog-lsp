//! Tests for server/handlers/diagnostics.rs - Diagnostics handler

mod common;

use common::TestFixture;
use ecolog_lsp::server::handlers::compute_diagnostics;
use tower_lsp::lsp_types::Url;
use std::fs;

#[tokio::test]
async fn test_diagnostics_undefined_env_var() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const x = process.env.UNDEFINED_VAR;");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const x = process.env.UNDEFINED_VAR;".into(),
            1,
        )
        .await;

    let diagnostics = compute_diagnostics(&uri, &fixture.state).await;

    assert!(!diagnostics.is_empty(), "Should have diagnostic for undefined var");
    assert!(diagnostics.iter().any(|d| d.message.contains("UNDEFINED_VAR")));
}

#[tokio::test]
async fn test_diagnostics_defined_env_var_no_warning() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.js", "const x = process.env.DB_URL;");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const x = process.env.DB_URL;".into(),
            1,
        )
        .await;

    let diagnostics = compute_diagnostics(&uri, &fixture.state).await;

    // DB_URL is defined in .env, so no diagnostics
    let undefined_warnings = diagnostics
        .iter()
        .filter(|d| d.message.contains("not defined"))
        .count();
    assert_eq!(undefined_warnings, 0, "Should not warn for defined var");
}

#[tokio::test]
async fn test_diagnostics_multiple_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const a = process.env.UNDEFINED_A;\nconst b = process.env.UNDEFINED_B;",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const a = process.env.UNDEFINED_A;\nconst b = process.env.UNDEFINED_B;".into(),
            1,
        )
        .await;

    let diagnostics = compute_diagnostics(&uri, &fixture.state).await;

    // Should have warnings for both undefined vars
    assert!(diagnostics.len() >= 2, "Should have at least 2 diagnostics");
}

#[tokio::test]
async fn test_diagnostics_python_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file("test.py", "import os\nx = os.environ['UNDEFINED_VAR']");

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "python".into(),
            "import os\nx = os.environ['UNDEFINED_VAR']".into(),
            1,
        )
        .await;

    let diagnostics = compute_diagnostics(&uri, &fixture.state).await;

    assert!(!diagnostics.is_empty(), "Should have diagnostic for Python undefined var");
}

#[tokio::test]
async fn test_diagnostics_destructuring_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const { UNDEFINED_VAR } = process.env;",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const { UNDEFINED_VAR } = process.env;".into(),
            1,
        )
        .await;

    let diagnostics = compute_diagnostics(&uri, &fixture.state).await;

    assert!(!diagnostics.is_empty(), "Should have diagnostic for destructured undefined var");
}

#[tokio::test]
async fn test_diagnostics_env_file_double_equals() {
    let fixture = TestFixture::new().await;

    // Create a .env file with error - use .env.local pattern which is in default config
    let env_path = fixture.temp_dir.join(".env.local");
    fs::write(&env_path, "KEY==value\n").unwrap();
    let uri = Url::from_file_path(&env_path).unwrap();

    fixture
        .state
        .document_manager
        .open(uri.clone(), "env".into(), "KEY==value\n".into(), 1)
        .await;

    let diagnostics = compute_diagnostics(&uri, &fixture.state).await;

    // Should detect double equals error
    assert!(!diagnostics.is_empty(), "Should detect .env syntax error");
}

#[tokio::test]
async fn test_diagnostics_env_file_valid() {
    let fixture = TestFixture::new().await;

    // The fixture's .env file should be valid
    let env_path = fixture.temp_dir.join(".env");
    let uri = Url::from_file_path(&env_path).unwrap();

    let content = fs::read_to_string(&env_path).unwrap();
    fixture
        .state
        .document_manager
        .open(uri.clone(), "env".into(), content, 1)
        .await;

    let diagnostics = compute_diagnostics(&uri, &fixture.state).await;

    // Valid .env should have no diagnostics
    assert!(diagnostics.is_empty(), "Valid .env should have no diagnostics");
}

#[tokio::test]
async fn test_diagnostics_document_not_found() {
    let fixture = TestFixture::new().await;
    let uri = Url::parse("file:///nonexistent/file.js").unwrap();

    // Don't open the document - test diagnostics for non-existent doc
    let diagnostics = compute_diagnostics(&uri, &fixture.state).await;

    assert!(diagnostics.is_empty(), "Should return empty for non-existent document");
}

#[tokio::test]
async fn test_diagnostics_object_alias_property_access() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const env = process.env;\nconst x = env.UNDEFINED_PROP;",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const env = process.env;\nconst x = env.UNDEFINED_PROP;".into(),
            1,
        )
        .await;

    let diagnostics = compute_diagnostics(&uri, &fixture.state).await;

    // Should detect undefined property access on env object alias
    assert!(!diagnostics.is_empty(), "Should detect undefined property on env alias");
}

#[tokio::test]
async fn test_diagnostics_mixed_defined_undefined() {
    let fixture = TestFixture::new().await;
    let uri = fixture.create_file(
        "test.js",
        "const db = process.env.DB_URL;\nconst x = process.env.UNDEFINED_VAR;",
    );

    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".into(),
            "const db = process.env.DB_URL;\nconst x = process.env.UNDEFINED_VAR;".into(),
            1,
        )
        .await;

    let diagnostics = compute_diagnostics(&uri, &fixture.state).await;

    // Only undefined should have warning
    let messages: Vec<_> = diagnostics.iter().map(|d| &d.message).collect();
    assert!(
        !messages.iter().any(|m| m.contains("DB_URL")),
        "Should not warn for DB_URL"
    );
    assert!(
        messages.iter().any(|m| m.contains("UNDEFINED_VAR")),
        "Should warn for UNDEFINED_VAR"
    );
}
