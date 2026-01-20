





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
async fn test_js_hover_only_on_var_name_dot_notation() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:

    
    
    
    
    let content = "const a = process.env.DB_URL;";
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 0)
        .await;

    
    let ref_on_process = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 10));
    assert!(
        ref_on_process.is_none(),
        "Hover should NOT trigger on 'process'"
    );

    
    let ref_on_env = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 18));
    assert!(ref_on_env.is_none(), "Hover should NOT trigger on 'env'");

    
    let ref_on_dot = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 21));
    assert!(ref_on_dot.is_none(), "Hover should NOT trigger on '.'");

    
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 22));
    assert!(ref_on_var.is_some(), "Hover SHOULD trigger on 'DB_URL'");
    assert_eq!(ref_on_var.unwrap().name, "DB_URL");

    
    let ref_on_var_mid = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 25));
    assert!(
        ref_on_var_mid.is_some(),
        "Hover SHOULD trigger in middle of 'DB_URL'"
    );
}

#[tokio::test]
async fn test_js_hover_only_on_var_name_bracket_notation() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:

    
    
    
    
    let content = "const a = process.env['DB_URL'];";
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 0)
        .await;

    
    let ref_on_bracket = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 21));
    assert!(
        ref_on_bracket.is_none(),
        "Hover should NOT trigger on '['"
    );

    
    let ref_on_quote = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 22));
    assert!(
        ref_on_quote.is_none(),
        "Hover should NOT trigger on opening quote"
    );

    
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 24));
    assert!(
        ref_on_var.is_some(),
        "Hover SHOULD trigger inside 'DB_URL' string"
    );
    assert_eq!(ref_on_var.unwrap().name, "DB_URL");
}





#[tokio::test]
async fn test_ts_hover_only_on_var_name_process_env() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:

    
    
    
    
    let content = "const a = process.env.VITE_API;";
    doc_manager
        .open(uri.clone(), "typescript".into(), content.to_string(), 0)
        .await;

    
    let ref_on_process = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 12));
    assert!(
        ref_on_process.is_none(),
        "Hover should NOT trigger on 'process'"
    );

    
    let ref_on_env = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 19));
    assert!(ref_on_env.is_none(), "Hover should NOT trigger on 'env'");

    
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 24));
    assert!(ref_on_var.is_some(), "Hover SHOULD trigger on 'VITE_API'");
    assert_eq!(ref_on_var.unwrap().name, "VITE_API");
}





#[tokio::test]
async fn test_py_hover_only_on_var_name_environ_subscript() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:

    
    
    
    
    
    let content = "import os\nx = os.environ['DB_URL']";
    doc_manager
        .open(uri.clone(), "python".into(), content.to_string(), 0)
        .await;

    
    let ref_on_os = doc_manager.get_env_reference_cloned(&uri, Position::new(1, 5));
    assert!(ref_on_os.is_none(), "Hover should NOT trigger on 'os'");

    
    let ref_on_environ = doc_manager.get_env_reference_cloned(&uri, Position::new(1, 9));
    assert!(
        ref_on_environ.is_none(),
        "Hover should NOT trigger on 'environ'"
    );

    
    let ref_on_bracket = doc_manager.get_env_reference_cloned(&uri, Position::new(1, 14));
    assert!(
        ref_on_bracket.is_none(),
        "Hover should NOT trigger on '['"
    );

    
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(1, 18));
    assert!(ref_on_var.is_some(), "Hover SHOULD trigger on 'DB_URL'");
    assert_eq!(ref_on_var.unwrap().name, "DB_URL");
}

#[tokio::test]
async fn test_py_hover_only_on_var_name_getenv() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:

    
    
    
    
    
    let content = "import os\nx = os.getenv('DB_URL')";
    doc_manager
        .open(uri.clone(), "python".into(), content.to_string(), 0)
        .await;

    
    let ref_on_getenv = doc_manager.get_env_reference_cloned(&uri, Position::new(1, 8));
    assert!(
        ref_on_getenv.is_none(),
        "Hover should NOT trigger on 'getenv'"
    );

    
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(1, 16));
    assert!(ref_on_var.is_some(), "Hover SHOULD trigger on 'DB_URL'");
    assert_eq!(ref_on_var.unwrap().name, "DB_URL");
}





#[tokio::test]
async fn test_rust_hover_only_on_var_name_std_env_var() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:

    
    
    
    
    let content = r#"fn main() { std::env::var("DB_URL"); }"#;
    doc_manager
        .open(uri.clone(), "rust".into(), content.to_string(), 0)
        .await;

    
    let ref_on_std = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 13));
    assert!(ref_on_std.is_none(), "Hover should NOT trigger on 'std'");

    
    let ref_on_env = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 18));
    assert!(ref_on_env.is_none(), "Hover should NOT trigger on 'env'");

    
    let ref_on_var_fn = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 23));
    assert!(
        ref_on_var_fn.is_none(),
        "Hover should NOT trigger on 'var' function name"
    );

    
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 28));
    assert!(ref_on_var.is_some(), "Hover SHOULD trigger on 'DB_URL'");
    assert_eq!(ref_on_var.unwrap().name, "DB_URL");
}

#[tokio::test]
async fn test_rust_hover_only_on_var_name_env_macro() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:

    
    
    
    
    let content = r#"fn main() { env!("DB_URL"); }"#;
    doc_manager
        .open(uri.clone(), "rust".into(), content.to_string(), 0)
        .await;

    
    let ref_on_env = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 13));
    assert!(
        ref_on_env.is_none(),
        "Hover should NOT trigger on 'env' macro name"
    );

    
    let ref_on_bang = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 15));
    assert!(ref_on_bang.is_none(), "Hover should NOT trigger on '!'");

    
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 19));
    assert!(
        ref_on_var.is_some(),
        "Hover SHOULD trigger on 'DB_URL' in env! macro"
    );
    assert_eq!(ref_on_var.unwrap().name, "DB_URL");
}





#[tokio::test]
async fn test_go_hover_only_on_var_name_getenv() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:

    
    
    
    
    
    
    
    
    let content = "package main\nimport \"os\"\nfunc main() {\n  val := os.Getenv(\"DB_URL\")\n}";
    doc_manager
        .open(uri.clone(), "go".into(), content.to_string(), 0)
        .await;

    
    let ref_on_os = doc_manager.get_env_reference_cloned(&uri, Position::new(3, 10));
    assert!(ref_on_os.is_none(), "Hover should NOT trigger on 'os'");

    
    let ref_on_getenv = doc_manager.get_env_reference_cloned(&uri, Position::new(3, 14));
    assert!(
        ref_on_getenv.is_none(),
        "Hover should NOT trigger on 'Getenv'"
    );

    
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(3, 22));
    assert!(ref_on_var.is_some(), "Hover SHOULD trigger on 'DB_URL'");
    assert_eq!(ref_on_var.unwrap().name, "DB_URL");
}

#[tokio::test]
async fn test_go_hover_only_on_var_name_lookupenv() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:

    
    
    
    
    let content =
        "package main\nimport \"os\"\nfunc main() {\n  val, ok := os.LookupEnv(\"API_KEY\")\n}";
    doc_manager
        .open(uri.clone(), "go".into(), content.to_string(), 0)
        .await;

    
    let ref_on_lookupenv = doc_manager.get_env_reference_cloned(&uri, Position::new(3, 18));
    assert!(
        ref_on_lookupenv.is_none(),
        "Hover should NOT trigger on 'LookupEnv'"
    );

    
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(3, 28));
    assert!(ref_on_var.is_some(), "Hover SHOULD trigger on 'API_KEY'");
    assert_eq!(ref_on_var.unwrap().name, "API_KEY");
}





#[tokio::test]
async fn test_js_no_hover_on_semicolon_after_var() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:

    
    
    
    
    
    let content = "const a = process.env.DB_URL;";
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 0)
        .await;

    
    let ref_on_last_char = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 27));
    assert!(
        ref_on_last_char.is_some(),
        "Hover SHOULD trigger on last char of 'DB_URL'"
    );

    
    let ref_on_semicolon = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 28));
    assert!(
        ref_on_semicolon.is_none(),
        "Hover should NOT trigger on ';' after variable"
    );
}

#[tokio::test]
async fn test_js_no_hover_on_closing_quote_bracket_notation() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:

    
    
    
    
    
    let content = "const a = process.env['DB_URL'];";
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 0)
        .await;

    
    let ref_on_last_char = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 28));
    assert!(
        ref_on_last_char.is_some(),
        "Hover SHOULD trigger on last char of 'DB_URL'"
    );

    
    let ref_on_quote = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 29));
    assert!(
        ref_on_quote.is_none(),
        "Hover should NOT trigger on closing quote"
    );

    
    let ref_on_bracket = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 30));
    assert!(
        ref_on_bracket.is_none(),
        "Hover should NOT trigger on ']'"
    );
}





#[tokio::test]
async fn test_returned_range_is_name_range_not_full_range() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:

    
    
    
    
    
    
    let content = "const a = process.env.DB_URL;";
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 0)
        .await;

    let ref_result = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 24));
    assert!(ref_result.is_some());
    let reference = ref_result.unwrap();

    
    assert_eq!(reference.name_range.start.line, 0);
    assert_eq!(reference.name_range.start.character, 22); 
    assert_eq!(reference.name_range.end.line, 0);
    assert_eq!(reference.name_range.end.character, 28); 

    
    assert_eq!(reference.full_range.start.line, 0);
    assert_eq!(reference.full_range.start.character, 10); 
    assert_eq!(reference.full_range.end.line, 0);
    assert_eq!(reference.full_range.end.character, 28); 
}
