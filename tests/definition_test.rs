use ecolog_lsp::analysis::document::DocumentManager;
use ecolog_lsp::analysis::query::QueryEngine;
use ecolog_lsp::languages::LanguageRegistry;
use std::sync::Arc;
use tower_lsp::lsp_types::{Position, Url};

async fn setup_manager() -> DocumentManager {
    let query_engine = Arc::new(QueryEngine::new());
    let mut registry = LanguageRegistry::new();
    registry.register(Arc::new(ecolog_lsp::languages::javascript::JavaScript));
    registry.register(Arc::new(ecolog_lsp::languages::python::Python));
    let languages = Arc::new(registry);
    DocumentManager::new(query_engine, languages.clone())
}

#[tokio::test]
async fn test_definition_reference_exists() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:
    let content = r#"
const api = process.env.API_KEY;
"#;
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 1)
        .await;

    
    let ref1 = doc_manager.get_env_reference_cloned(&uri, Position::new(1, 26));
    assert!(
        ref1.is_some(),
        "Reference should exist for definition lookup"
    );
    assert_eq!(ref1.unwrap().name, "API_KEY");
}

#[tokio::test]
async fn test_definition_python_environ() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:
    let content = r#"
import os
val = os.environ["DB_HOST"]
"#;
    doc_manager
        .open(uri.clone(), "python".into(), content.to_string(), 1)
        .await;

    
    let ref1 = doc_manager.get_env_reference_cloned(&uri, Position::new(2, 21));
    assert!(ref1.is_some(), "Reference should exist for Python environ");
    assert_eq!(ref1.unwrap().name, "DB_HOST");
}
