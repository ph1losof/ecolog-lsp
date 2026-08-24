//! Behavioural coverage for language queries.
//!
//! The `*_queries_compile` unit tests prove a query parses. They cannot prove it
//! matches anything, and an empty query is a perfectly valid query -- so these
//! tests pin the captures that each previously-broken query is supposed to
//! produce.

use ecolog_lsp::analysis::QueryEngine;
use ecolog_lsp::languages::{LanguageRegistry, LanguageSupport};
use std::sync::Arc;

fn language(id: &str) -> Arc<dyn LanguageSupport> {
    LanguageRegistry::with_all_languages()
        .get_by_language_id(id)
        .unwrap_or_else(|| panic!("no language registered for {}", id))
}

fn parse(lang: &dyn LanguageSupport, source: &str) -> tree_sitter::Tree {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&lang.grammar())
        .expect("failed to set grammar");
    parser.parse(source, None).expect("failed to parse")
}

// ───────────────────────────────────────────────────────────────────────────
// C# -- `variable_declarator` holds its initializer directly, and string bodies
// are `string_literal_content`.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_csharp_binding_from_local_declaration() {
    let lang = language("csharp");
    let source = r#"
class C {
    void M() {
        var apiKey = Environment.GetEnvironmentVariable("API_KEY");
    }
}
"#;
    let tree = parse(lang.as_ref(), source);
    let bindings = QueryEngine::new()
        .extract_bindings(lang.as_ref(), &tree, source.as_bytes())
        .await;

    assert_eq!(bindings.len(), 1, "expected one binding, got {:?}", bindings);
    assert_eq!(bindings[0].binding_name, "apiKey");
    assert_eq!(bindings[0].env_var_name, "API_KEY");
}

#[tokio::test]
async fn test_csharp_binding_from_field_declaration() {
    let lang = language("csharp");
    let source = r#"
class C {
    private string _dbUrl = Environment.GetEnvironmentVariable("DB_URL");
}
"#;
    let tree = parse(lang.as_ref(), source);
    let bindings = QueryEngine::new()
        .extract_bindings(lang.as_ref(), &tree, source.as_bytes())
        .await;

    assert_eq!(bindings.len(), 1, "expected one binding, got {:?}", bindings);
    assert_eq!(bindings[0].binding_name, "_dbUrl");
    assert_eq!(bindings[0].env_var_name, "DB_URL");
}

#[tokio::test]
async fn test_csharp_assignments_cover_declaration_and_reassignment() {
    let lang = language("csharp");
    let source = r#"
class C {
    void M() {
        var a = Environment.GetEnvironmentVariable("API_KEY");
        var b = a;
        string c;
        c = b;
    }
}
"#;
    let tree = parse(lang.as_ref(), source);
    let assignments = QueryEngine::new()
        .extract_assignments(lang.as_ref(), &tree, source.as_bytes())
        .await;

    let pairs: Vec<(String, String)> = assignments
        .iter()
        .map(|(target, _, src)| (target.to_string(), src.to_string()))
        .collect();

    assert!(
        pairs.contains(&("b".to_string(), "a".to_string())),
        "missing `var b = a` chain, got {:?}",
        pairs
    );
    assert!(
        pairs.contains(&("c".to_string(), "b".to_string())),
        "missing `c = b` chain, got {:?}",
        pairs
    );
}

#[tokio::test]
async fn test_csharp_has_no_destructure_patterns() {
    // C# exposes the environment through calls and dictionary indexing, not
    // through an object that can be destructured, so this query is empty by
    // design rather than by breakage.
    let lang = language("csharp");
    let query = lang.destructure_query().expect("destructure query exists");
    assert_eq!(query.pattern_count(), 0);
}

// ───────────────────────────────────────────────────────────────────────────
// Kotlin -- the node is `import` holding a `qualified_identifier`, with an
// optional trailing alias.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_kotlin_imports_plain_and_aliased() {
    let lang = language("kotlin");
    let source = r#"
import java.lang.System
import com.example.EnvConfig as Config

fun main() {}
"#;
    let tree = parse(lang.as_ref(), source);
    let imports = QueryEngine::new()
        .extract_imports(lang.as_ref(), &tree, source.as_bytes())
        .await;

    assert_eq!(imports.len(), 2, "expected two imports, got {:?}", imports);

    let plain = imports
        .iter()
        .find(|i| i.module_path == "java.lang.System")
        .expect("plain import missing");
    assert_eq!(plain.alias, None);

    let aliased = imports
        .iter()
        .find(|i| i.module_path == "com.example.EnvConfig")
        .expect("aliased import missing");
    assert_eq!(aliased.alias.as_deref(), Some("Config"));
}

// ───────────────────────────────────────────────────────────────────────────
// Zig -- `pub` is an anonymous token, so only `pub` items are exported.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_zig_exports_only_public_items() {
    let lang = language("zig");
    let source = r#"
pub fn publicFn() void {}
fn privateFn() void {}
pub const publicConst = 1;
const privateConst = 2;
"#;
    let tree = parse(lang.as_ref(), source);
    let exports = QueryEngine::new()
        .extract_exports(lang.as_ref(), &tree, source.as_bytes())
        .await;

    let names: Vec<&str> = exports.named_exports.keys().map(|k| k.as_str()).collect();

    assert!(names.contains(&"publicFn"), "got {:?}", names);
    assert!(names.contains(&"publicConst"), "got {:?}", names);
    assert!(!names.contains(&"privateFn"), "got {:?}", names);
    assert!(!names.contains(&"privateConst"), "got {:?}", names);
}

// ───────────────────────────────────────────────────────────────────────────
// C++ -- a namespace name is a `namespace_identifier`, not an `identifier`.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_cpp_exports_include_namespace_class_and_function() {
    let lang = language("cpp");
    let source = r#"
namespace config {
class Settings { };
void load() {}
}
"#;
    let tree = parse(lang.as_ref(), source);
    let exports = QueryEngine::new()
        .extract_exports(lang.as_ref(), &tree, source.as_bytes())
        .await;

    let names: Vec<&str> = exports.named_exports.keys().map(|k| k.as_str()).collect();

    assert!(names.contains(&"config"), "namespace missing, got {:?}", names);
    assert!(names.contains(&"Settings"), "class missing, got {:?}", names);
    assert!(names.contains(&"load"), "function missing, got {:?}", names);
}
