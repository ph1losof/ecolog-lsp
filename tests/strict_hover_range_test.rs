//! Tests for strict hover range detection across all languages.
//!
//! Verifies that hover/go-to-definition only triggers when cursor is
//! on the variable name (e.g., DB_URL), not on other parts of the env
//! access expression (e.g., "process", "env", quotes, brackets).

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

// ============================================================================
// JavaScript Tests
// ============================================================================

#[tokio::test]
async fn test_js_hover_only_on_var_name_dot_notation() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///test.js").unwrap();

    // "const a = process.env.DB_URL;"
    //  0123456789012345678901234567890
    //            ^         ^  ^
    //            10        21 22-27 (DB_URL)
    let content = "const a = process.env.DB_URL;";
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 0)
        .await;

    // Position on "process" (char 10) - should NOT trigger
    let ref_on_process = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 10));
    assert!(
        ref_on_process.is_none(),
        "Hover should NOT trigger on 'process'"
    );

    // Position on "env" (char 18) - should NOT trigger
    let ref_on_env = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 18));
    assert!(ref_on_env.is_none(), "Hover should NOT trigger on 'env'");

    // Position on the dot after "env" (char 21) - should NOT trigger
    let ref_on_dot = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 21));
    assert!(ref_on_dot.is_none(), "Hover should NOT trigger on '.'");

    // Position on "DB_URL" (char 22) - SHOULD trigger
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 22));
    assert!(ref_on_var.is_some(), "Hover SHOULD trigger on 'DB_URL'");
    assert_eq!(ref_on_var.unwrap().name, "DB_URL");

    // Position in middle of "DB_URL" (char 25) - SHOULD trigger
    let ref_on_var_mid = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 25));
    assert!(
        ref_on_var_mid.is_some(),
        "Hover SHOULD trigger in middle of 'DB_URL'"
    );
}

#[tokio::test]
async fn test_js_hover_only_on_var_name_bracket_notation() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///test.js").unwrap();

    // "const a = process.env['DB_URL'];"
    //  01234567890123456789012345678901
    //            ^         ^ ^^      ^
    //            10        21 22      30
    let content = "const a = process.env['DB_URL'];";
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 0)
        .await;

    // Position on bracket (char 21) - should NOT trigger
    let ref_on_bracket = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 21));
    assert!(
        ref_on_bracket.is_none(),
        "Hover should NOT trigger on '['"
    );

    // Position on opening quote (char 22) - should NOT trigger
    let ref_on_quote = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 22));
    assert!(
        ref_on_quote.is_none(),
        "Hover should NOT trigger on opening quote"
    );

    // Position inside "DB_URL" string (char 24) - SHOULD trigger
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 24));
    assert!(
        ref_on_var.is_some(),
        "Hover SHOULD trigger inside 'DB_URL' string"
    );
    assert_eq!(ref_on_var.unwrap().name, "DB_URL");
}

// ============================================================================
// TypeScript Tests (import.meta.env)
// ============================================================================

#[tokio::test]
async fn test_ts_hover_only_on_var_name_process_env() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///test.ts").unwrap();

    // "const a = process.env.VITE_API;"
    //  0123456789012345678901234567890
    //            ^         ^  ^
    //            10        21 22-29 (VITE_API)
    let content = "const a = process.env.VITE_API;";
    doc_manager
        .open(uri.clone(), "typescript".into(), content.to_string(), 0)
        .await;

    // Position on "process" (char 12) - should NOT trigger
    let ref_on_process = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 12));
    assert!(
        ref_on_process.is_none(),
        "Hover should NOT trigger on 'process'"
    );

    // Position on "env" (char 19) - should NOT trigger
    let ref_on_env = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 19));
    assert!(ref_on_env.is_none(), "Hover should NOT trigger on 'env'");

    // Position on "VITE_API" (char 24) - SHOULD trigger
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 24));
    assert!(ref_on_var.is_some(), "Hover SHOULD trigger on 'VITE_API'");
    assert_eq!(ref_on_var.unwrap().name, "VITE_API");
}

// ============================================================================
// Python Tests
// ============================================================================

#[tokio::test]
async fn test_py_hover_only_on_var_name_environ_subscript() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///test.py").unwrap();

    // Line 0: "import os"
    // Line 1: "x = os.environ['DB_URL']"
    //          01234567890123456789012345
    //              ^  ^       ^^
    //              4  7       16-22
    let content = "import os\nx = os.environ['DB_URL']";
    doc_manager
        .open(uri.clone(), "python".into(), content.to_string(), 0)
        .await;

    // Position on "os" (line 1, char 5) - should NOT trigger
    let ref_on_os = doc_manager.get_env_reference_cloned(&uri, Position::new(1, 5));
    assert!(ref_on_os.is_none(), "Hover should NOT trigger on 'os'");

    // Position on "environ" (line 1, char 9) - should NOT trigger
    let ref_on_environ = doc_manager.get_env_reference_cloned(&uri, Position::new(1, 9));
    assert!(
        ref_on_environ.is_none(),
        "Hover should NOT trigger on 'environ'"
    );

    // Position on bracket (line 1, char 14) - should NOT trigger
    let ref_on_bracket = doc_manager.get_env_reference_cloned(&uri, Position::new(1, 14));
    assert!(
        ref_on_bracket.is_none(),
        "Hover should NOT trigger on '['"
    );

    // Position inside "DB_URL" (line 1, char 18) - SHOULD trigger
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(1, 18));
    assert!(ref_on_var.is_some(), "Hover SHOULD trigger on 'DB_URL'");
    assert_eq!(ref_on_var.unwrap().name, "DB_URL");
}

#[tokio::test]
async fn test_py_hover_only_on_var_name_getenv() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///test.py").unwrap();

    // Line 0: "import os"
    // Line 1: "x = os.getenv('DB_URL')"
    //          01234567890123456789012345
    //              ^  ^      ^^
    //              4  7      14-20
    let content = "import os\nx = os.getenv('DB_URL')";
    doc_manager
        .open(uri.clone(), "python".into(), content.to_string(), 0)
        .await;

    // Position on "getenv" (line 1, char 8) - should NOT trigger
    let ref_on_getenv = doc_manager.get_env_reference_cloned(&uri, Position::new(1, 8));
    assert!(
        ref_on_getenv.is_none(),
        "Hover should NOT trigger on 'getenv'"
    );

    // Position inside "DB_URL" (line 1, char 16) - SHOULD trigger
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(1, 16));
    assert!(ref_on_var.is_some(), "Hover SHOULD trigger on 'DB_URL'");
    assert_eq!(ref_on_var.unwrap().name, "DB_URL");
}

// ============================================================================
// Rust Tests
// ============================================================================

#[tokio::test]
async fn test_rust_hover_only_on_var_name_std_env_var() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///main.rs").unwrap();

    // "fn main() { std::env::var(\"DB_URL\"); }"
    //  0123456789012345678901234567890123456789
    //              ^   ^    ^   ^^
    //              12  17   22  27-33
    let content = r#"fn main() { std::env::var("DB_URL"); }"#;
    doc_manager
        .open(uri.clone(), "rust".into(), content.to_string(), 0)
        .await;

    // Position on "std" (char 12) - should NOT trigger
    let ref_on_std = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 13));
    assert!(ref_on_std.is_none(), "Hover should NOT trigger on 'std'");

    // Position on "env" (char 17) - should NOT trigger
    let ref_on_env = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 18));
    assert!(ref_on_env.is_none(), "Hover should NOT trigger on 'env'");

    // Position on "var" function (char 22) - should NOT trigger
    let ref_on_var_fn = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 23));
    assert!(
        ref_on_var_fn.is_none(),
        "Hover should NOT trigger on 'var' function name"
    );

    // Position inside "DB_URL" (char 28) - SHOULD trigger
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 28));
    assert!(ref_on_var.is_some(), "Hover SHOULD trigger on 'DB_URL'");
    assert_eq!(ref_on_var.unwrap().name, "DB_URL");
}

#[tokio::test]
async fn test_rust_hover_only_on_var_name_env_macro() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///main.rs").unwrap();

    // "fn main() { env!(\"DB_URL\"); }"
    //  012345678901234567890123456789
    //              ^   ^^
    //              12  18-24
    let content = r#"fn main() { env!("DB_URL"); }"#;
    doc_manager
        .open(uri.clone(), "rust".into(), content.to_string(), 0)
        .await;

    // Position on "env" macro (char 12) - should NOT trigger
    let ref_on_env = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 13));
    assert!(
        ref_on_env.is_none(),
        "Hover should NOT trigger on 'env' macro name"
    );

    // Position on "!" (char 15) - should NOT trigger
    let ref_on_bang = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 15));
    assert!(ref_on_bang.is_none(), "Hover should NOT trigger on '!'");

    // Position inside "DB_URL" (char 19) - SHOULD trigger
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 19));
    assert!(
        ref_on_var.is_some(),
        "Hover SHOULD trigger on 'DB_URL' in env! macro"
    );
    assert_eq!(ref_on_var.unwrap().name, "DB_URL");
}

// ============================================================================
// Go Tests
// ============================================================================

#[tokio::test]
async fn test_go_hover_only_on_var_name_getenv() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///main.go").unwrap();

    // Line 0: "package main"
    // Line 1: "import \"os\""
    // Line 2: "func main() {"
    // Line 3: "  val := os.Getenv(\"DB_URL\")"
    //          012345678901234567890123456789
    //                   ^  ^      ^^
    //                   9  12     20-26
    // Line 4: "}"
    let content = "package main\nimport \"os\"\nfunc main() {\n  val := os.Getenv(\"DB_URL\")\n}";
    doc_manager
        .open(uri.clone(), "go".into(), content.to_string(), 0)
        .await;

    // Position on "os" (line 3, char 10) - should NOT trigger
    let ref_on_os = doc_manager.get_env_reference_cloned(&uri, Position::new(3, 10));
    assert!(ref_on_os.is_none(), "Hover should NOT trigger on 'os'");

    // Position on "Getenv" (line 3, char 14) - should NOT trigger
    let ref_on_getenv = doc_manager.get_env_reference_cloned(&uri, Position::new(3, 14));
    assert!(
        ref_on_getenv.is_none(),
        "Hover should NOT trigger on 'Getenv'"
    );

    // Position inside "DB_URL" (line 3, char 22) - SHOULD trigger
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(3, 22));
    assert!(ref_on_var.is_some(), "Hover SHOULD trigger on 'DB_URL'");
    assert_eq!(ref_on_var.unwrap().name, "DB_URL");
}

#[tokio::test]
async fn test_go_hover_only_on_var_name_lookupenv() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///main.go").unwrap();

    // Line 3: "  val, ok := os.LookupEnv(\"API_KEY\")"
    //          01234567890123456789012345678901234567
    //                       ^  ^         ^^
    //                       13 16        27-34
    let content =
        "package main\nimport \"os\"\nfunc main() {\n  val, ok := os.LookupEnv(\"API_KEY\")\n}";
    doc_manager
        .open(uri.clone(), "go".into(), content.to_string(), 0)
        .await;

    // Position on "LookupEnv" (line 3, char 18) - should NOT trigger
    let ref_on_lookupenv = doc_manager.get_env_reference_cloned(&uri, Position::new(3, 18));
    assert!(
        ref_on_lookupenv.is_none(),
        "Hover should NOT trigger on 'LookupEnv'"
    );

    // Position inside "API_KEY" (line 3, char 28) - SHOULD trigger
    let ref_on_var = doc_manager.get_env_reference_cloned(&uri, Position::new(3, 28));
    assert!(ref_on_var.is_some(), "Hover SHOULD trigger on 'API_KEY'");
    assert_eq!(ref_on_var.unwrap().name, "API_KEY");
}

// ============================================================================
// Boundary Tests - Character immediately after variable name
// ============================================================================

#[tokio::test]
async fn test_js_no_hover_on_semicolon_after_var() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///test.js").unwrap();

    // "const a = process.env.DB_URL;"
    //  0123456789012345678901234567890
    //                        ^     ^
    //                        22    28 (semicolon)
    // DB_URL is at positions 22-27 (inclusive), position 28 is the semicolon
    let content = "const a = process.env.DB_URL;";
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 0)
        .await;

    // Position at end of DB_URL (char 27) - SHOULD trigger
    let ref_on_last_char = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 27));
    assert!(
        ref_on_last_char.is_some(),
        "Hover SHOULD trigger on last char of 'DB_URL'"
    );

    // Position on semicolon (char 28) - should NOT trigger
    let ref_on_semicolon = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 28));
    assert!(
        ref_on_semicolon.is_none(),
        "Hover should NOT trigger on ';' after variable"
    );
}

#[tokio::test]
async fn test_js_no_hover_on_closing_quote_bracket_notation() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///test.js").unwrap();

    // "const a = process.env['DB_URL'];"
    //  01234567890123456789012345678901234
    //                        ^      ^^
    //                        23     29 30 (closing quote, bracket)
    // DB_URL string content is at positions 23-28, position 29 is closing quote
    let content = "const a = process.env['DB_URL'];";
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 0)
        .await;

    // Position at end of DB_URL (char 28) - SHOULD trigger
    let ref_on_last_char = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 28));
    assert!(
        ref_on_last_char.is_some(),
        "Hover SHOULD trigger on last char of 'DB_URL'"
    );

    // Position on closing quote (char 29) - should NOT trigger
    let ref_on_quote = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 29));
    assert!(
        ref_on_quote.is_none(),
        "Hover should NOT trigger on closing quote"
    );

    // Position on closing bracket (char 30) - should NOT trigger
    let ref_on_bracket = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 30));
    assert!(
        ref_on_bracket.is_none(),
        "Hover should NOT trigger on ']'"
    );
}

// ============================================================================
// Range Verification Tests
// ============================================================================

#[tokio::test]
async fn test_returned_range_is_name_range_not_full_range() {
    let doc_manager = setup_manager().await;
    let uri = Url::parse("file:///test.js").unwrap();

    // "const a = process.env.DB_URL;"
    //  0123456789012345678901234567890
    //            ^         ^  ^     ^
    //            10        21 22    28
    // full_range would be (0, 10) to (0, 28) - the entire process.env.DB_URL
    // name_range should be (0, 22) to (0, 28) - just DB_URL
    let content = "const a = process.env.DB_URL;";
    doc_manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 0)
        .await;

    let ref_result = doc_manager.get_env_reference_cloned(&uri, Position::new(0, 24));
    assert!(ref_result.is_some());
    let reference = ref_result.unwrap();

    // Verify name_range is just the variable name
    assert_eq!(reference.name_range.start.line, 0);
    assert_eq!(reference.name_range.start.character, 22); // Start of DB_URL
    assert_eq!(reference.name_range.end.line, 0);
    assert_eq!(reference.name_range.end.character, 28); // End of DB_URL

    // Verify full_range is the entire expression (for other uses)
    assert_eq!(reference.full_range.start.line, 0);
    assert_eq!(reference.full_range.start.character, 10); // Start of process
    assert_eq!(reference.full_range.end.line, 0);
    assert_eq!(reference.full_range.end.character, 28); // End of DB_URL
}
