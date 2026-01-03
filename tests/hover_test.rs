use ecolog_lsp::analysis::document::DocumentManager;
use ecolog_lsp::analysis::query::QueryEngine;
use ecolog_lsp::languages::LanguageRegistry;
use std::sync::Arc;
use tower_lsp::lsp_types::{Position, Url};

async fn setup_manager() -> DocumentManager {
    let query_engine = Arc::new(QueryEngine::new());
    let mut registry = LanguageRegistry::new();
    registry.register(Arc::new(ecolog_lsp::languages::javascript::JavaScript));
    registry.register(Arc::new(ecolog_lsp::languages::typescript::TypeScript));
    registry.register(Arc::new(ecolog_lsp::languages::python::Python));
    let languages = Arc::new(registry);
    DocumentManager::new(query_engine, languages.clone())
}

#[tokio::test]
async fn test_hover_direct_access() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///test.js").unwrap();
    let content = r#"
const api = process.env.API_KEY;
"#;
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 1)
        .await;

    // Hover on "API_KEY" (line 1, around char 24-31)
    let ref1 = doc_manager.get_env_reference_cloned(&uri, Position::new(1, 26));
    assert!(ref1.is_some(), "Should find reference for direct access");
    assert_eq!(ref1.unwrap().name, "API_KEY");
}

#[tokio::test]
async fn test_hover_object_alias() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///test.js").unwrap();
    let content = r#"
const env = process.env;
const secret = env.SECRET;
"#;
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 1)
        .await;

    // Hover on "SECRET" via alias (line 2)
    let ref1 = doc_manager.get_env_reference_cloned(&uri, Position::new(2, 22));
    assert!(ref1.is_some(), "Should find reference via object alias");
    assert_eq!(ref1.unwrap().name, "SECRET");
}

#[tokio::test]
async fn test_hover_subscript_access() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///test.js").unwrap();
    let content = r#"
const env = process.env;
const db = env["DATABASE_URL"];
"#;
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 1)
        .await;

    // Hover on "DATABASE_URL" (line 2, inside the string)
    let ref1 = doc_manager.get_env_reference_cloned(&uri, Position::new(2, 20));
    assert!(ref1.is_some(), "Should find reference via subscript access");
    assert_eq!(ref1.unwrap().name, "DATABASE_URL");
}

#[tokio::test]
async fn test_repro_integration_js_single_line() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///test.js").unwrap();
    let content = "const a = process.env.DB_URL;";
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 0)
        .await;

    // Hover on "DB_URL" (line 0, char 22)
    let ref1 = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 22));
    assert!(
        ref1.is_some(),
        "Should find reference for single line direct access"
    );
    assert_eq!(ref1.unwrap().name, "DB_URL");
}
