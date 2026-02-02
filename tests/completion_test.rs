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
    registry.register(Arc::new(ecolog_lsp::languages::c::C));
    registry.register(Arc::new(ecolog_lsp::languages::java::Java));
    let languages = Arc::new(registry);
    DocumentManager::new(query_engine, languages.clone())
}

#[tokio::test]
async fn test_completion_context_js() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///test.js").unwrap();

    let content = r#"
        const env = process.env;
        env.
    "#;
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 1)
        .await;

    
    let pos = Position::new(2, 12);

    let ctx = doc_manager.check_completion_context(&uri, pos).await;
    assert!(ctx.is_some(), "Should detect completion context for 'env.'");
    assert_eq!(ctx.unwrap(), "env");
}

#[tokio::test]
async fn test_completion_context_python() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///test.py").unwrap();

    let content = r#"
        env.
    "#;
    doc_manager
        .open(uri.clone(), "python".into(), content.to_string(), 1)
        .await;

    let pos = Position::new(1, 12);

    let ctx = doc_manager.check_completion_context(&uri, pos).await;
    assert!(ctx.is_some(), "Should detect completion context for 'env.'");
    assert_eq!(ctx.unwrap(), "env");
}

#[tokio::test]
async fn test_completion_context_c_getenv() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///test.c").unwrap();

    let content = r#"char* val = getenv("");"#;
    doc_manager
        .open(uri.clone(), "c".into(), content.to_string(), 1)
        .await;

    // Position inside the quotes: getenv("") - character 20
    let pos = Position::new(0, 20);

    let ctx = doc_manager.check_completion_context(&uri, pos).await;
    assert!(ctx.is_some(), "Should detect completion context for C getenv(\"\")");
    assert_eq!(ctx.unwrap(), "getenv");
}

#[tokio::test]
async fn test_completion_context_c_secure_getenv() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///test.c").unwrap();

    let content = r#"char* val = secure_getenv("");"#;
    doc_manager
        .open(uri.clone(), "c".into(), content.to_string(), 1)
        .await;

    // Position inside the quotes: secure_getenv("") - character 27
    let pos = Position::new(0, 27);

    let ctx = doc_manager.check_completion_context(&uri, pos).await;
    assert!(ctx.is_some(), "Should detect completion context for C secure_getenv(\"\")");
    assert_eq!(ctx.unwrap(), "secure_getenv");
}

#[tokio::test]
async fn test_completion_context_java_system_getenv() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///Test.java").unwrap();

    // String val = System.getenv("");
    // 0      6 7  12  13    19 20    26 27 28
    // Position 28 is inside the empty quotes (after opening quote, before closing quote)
    let content = r#"String val = System.getenv("");"#;
    doc_manager
        .open(uri.clone(), "java".into(), content.to_string(), 1)
        .await;

    // Position inside the quotes: System.getenv("") - character 28
    let pos = Position::new(0, 28);

    let ctx = doc_manager.check_completion_context(&uri, pos).await;
    assert!(ctx.is_some(), "Should detect completion context for Java System.getenv(\"\")");
    assert_eq!(ctx.unwrap(), "System");
}
