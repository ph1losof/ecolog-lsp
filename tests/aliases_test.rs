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
    registry.register(Arc::new(ecolog_lsp::languages::rust::Rust));
    registry.register(Arc::new(ecolog_lsp::languages::go::Go));
    let languages = Arc::new(registry);
    DocumentManager::new(query_engine, languages.clone())
}

#[tokio::test]
async fn test_js_object_alias() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:
    let content = r#"
        const env = process.env;
        const api = env.API_KEY; 
        const secret = env["SECRET"];
    "#;
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 1)
        .await;

    
    let ref1 = doc_manager
        .get_env_reference_cloned(&uri, Position::new(2, 24))
        .expect("Should find ref at L2");
    assert_eq!(ref1.name, "API_KEY");

    
    let ref2 = doc_manager
        .get_env_reference_cloned(&uri, Position::new(3, 30))
        .expect("Should find ref at L3");
    assert_eq!(ref2.name, "SECRET");
}

#[tokio::test]
async fn test_python_module_alias() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:
    let content = r#"
import os as o
val1 = o.environ["VAR1"]
val2 = o.getenv("VAR2")
"#;
    doc_manager
        .open(uri.clone(), "python".into(), content.to_string(), 1)
        .await;

    
    let ref1 = doc_manager
        .get_env_reference_cloned(&uri, Position::new(2, 20))
        .expect("Should find VAR1 at L2");
    assert_eq!(ref1.name, "VAR1");

    
    let ref2 = doc_manager
        .get_env_reference_cloned(&uri, Position::new(3, 19))
        .expect("Should find VAR2 at L3");
    assert_eq!(ref2.name, "VAR2");
}

#[tokio::test]
async fn test_python_object_alias() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:
    let content = r#"
from os import environ as e
val = e["VAR"]
"#;
    doc_manager
        .open(uri.clone(), "python".into(), content.to_string(), 1)
        .await;

    
    let ref1 = doc_manager
        .get_env_reference_cloned(&uri, Position::new(2, 10))
        .expect("Should find VAR at L2");
    assert_eq!(ref1.name, "VAR");
}

#[tokio::test]
async fn test_rust_module_alias() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:
    let content = r#"
use std::env as e;
fn main() {
    let v = e::var("VAR").unwrap();
}
"#;
    doc_manager
        .open(uri.clone(), "rust".into(), content.to_string(), 1)
        .await;
    
    let ref1 = doc_manager
        .get_env_reference_cloned(&uri, Position::new(3, 22))
        .expect("Should find VAR at L3");
    assert_eq!(ref1.name, "VAR");
}

#[tokio::test]
async fn test_go_module_alias() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:
    let content = r#"
package main
import (
    e "os"
    "fmt"
)
func main() {
    val := e.Getenv("VAR")
}
"#;
    doc_manager
        .open(uri.clone(), "go".into(), content.to_string(), 1)
        .await;

    
    
    let ref1 = doc_manager
        .get_env_reference_cloned(&uri, Position::new(7, 22))
        .expect("Should find VAR at L7");
    assert_eq!(ref1.name, "VAR");
}

#[tokio::test]
async fn test_scope_isolation() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:
    let content = r#"
function a() {
    const env = process.env;
    env.A;
}
function b() {
    env.B; 
}
"#;
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 1)
        .await;

    
    let ref_a = doc_manager.get_env_reference_cloned(&uri, Position::new(3, 8));
    assert!(ref_a.is_some(), "Should detect env.A in scope");

    
    let ref_b = doc_manager.get_env_reference_cloned(&uri, Position::new(6, 8));
    assert!(ref_b.is_none(), "Should NOT detect env.B out of scope");
}
