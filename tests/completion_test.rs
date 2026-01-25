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
