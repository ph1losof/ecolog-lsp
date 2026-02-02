use crate::types::{EnvSourceKind, ScopeKind};
use compact_str::CompactString;
use tree_sitter::{Language, Node, Query};

pub mod bash;
pub mod c;
pub mod cpp;
pub mod csharp;
pub mod elixir;
pub mod go;
pub mod java;
pub mod javascript;
pub mod kotlin;
pub mod lua;
pub mod php;
pub mod python;
pub mod registry;
pub mod ruby;
pub mod rust;
pub mod typescript;
pub mod zig;

pub use registry::LanguageRegistry;

pub trait LanguageSupport: Send + Sync {
    fn id(&self) -> &'static str;

    fn extensions(&self) -> &'static [&'static str];

    fn language_ids(&self) -> &'static [&'static str];

    fn grammar(&self) -> Language;

    fn reference_query(&self) -> &Query;

    fn binding_query(&self) -> Option<&Query> {
        None
    }

    fn completion_query(&self) -> Option<&Query> {
        None
    }

    fn reassignment_query(&self) -> Option<&Query> {
        None
    }

    fn import_query(&self) -> Option<&Query> {
        None
    }

    fn identifier_query(&self) -> Option<&Query> {
        None
    }

    fn assignment_query(&self) -> Option<&Query> {
        None
    }

    fn destructure_query(&self) -> Option<&Query> {
        None
    }

    fn scope_query(&self) -> Option<&Query> {
        None
    }

    fn export_query(&self) -> Option<&Query> {
        None
    }

    fn extract_var_name(&self, node: Node, source: &[u8]) -> Option<CompactString> {
        node.utf8_text(source).ok().map(|s| s.trim().into())
    }

    fn extract_identifier(&self, node: Node, source: &[u8]) -> Option<CompactString> {
        node.utf8_text(source).ok().map(|s| s.trim().into())
    }

    fn extract_destructure_key(&self, node: Node, source: &[u8]) -> Option<CompactString> {
        node.utf8_text(source).ok().map(|s| s.trim().into())
    }

    fn extract_property_access(
        &self,
        _tree: &tree_sitter::Tree,
        _content: &str,
        _byte_offset: usize,
    ) -> Option<(CompactString, CompactString)> {
        None
    }

    fn is_env_source_node(&self, _node: Node, _source: &[u8]) -> Option<EnvSourceKind> {
        None
    }

    fn strip_quotes<'a>(&self, text: &'a str) -> &'a str {
        text.trim_matches(|c| c == '"' || c == '\'')
    }

    fn known_env_modules(&self) -> &'static [&'static str] {
        &[]
    }

    fn is_standard_env_object(&self, _name: &str) -> bool {
        false
    }

    fn default_env_object_name(&self) -> Option<&'static str> {
        None
    }

    fn completion_trigger_characters(&self) -> &'static [&'static str] {
        &[]
    }

    /// Returns the node kinds that represent comments in this language.
    /// Used to filter out env var matches that appear inside comments.
    fn comment_node_kinds(&self) -> &'static [&'static str] {
        &["comment"]
    }

    /// Validates if the characters before cursor form a valid completion trigger.
    /// Returns true if completion should proceed, false to skip.
    fn is_valid_completion_trigger(&self, source: &[u8], byte_offset: usize) -> bool {
        let triggers = self.completion_trigger_characters();

        for trigger in triggers {
            let len = trigger.len();
            if len == 0 {
                continue;
            }
            if byte_offset >= len {
                let slice = &source[byte_offset - len..byte_offset];
                if slice == trigger.as_bytes() {
                    return true;
                }
            }
        }

        false
    }

    fn is_scope_node(&self, _node: Node) -> bool {
        _node.kind() == "program" || _node.kind() == "source_file" || _node.kind() == "module"
    }

    fn is_root_node(&self, node: Node) -> bool {
        matches!(node.kind(), "program" | "source_file" | "module")
    }

    fn node_to_scope_kind(&self, kind: &str) -> ScopeKind {
        match kind {
            "function_declaration"
            | "arrow_function"
            | "function"
            | "method_definition"
            | "function_definition"
            | "function_item"
            | "func_literal"
            | "closure_expression"
            | "generator_function"
            | "generator_function_declaration" => ScopeKind::Function,

            "class_declaration" | "class_definition" | "class_body" | "impl_item"
            | "trait_item" | "class" => ScopeKind::Class,

            "for_statement" | "for_expression" | "while_statement" | "while_expression"
            | "loop_expression" | "do_statement" | "for_in_statement" | "for_of_statement" => {
                ScopeKind::Loop
            }

            "if_statement" | "if_expression" | "else_clause" | "try_statement" | "catch_clause"
            | "match_expression" | "switch_statement" | "switch_case" => ScopeKind::Conditional,

            _ => ScopeKind::Block,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to validate all queries for a given language implementation.
    /// This triggers compilation of all OnceLock-cached queries, catching any
    /// tree-sitter query syntax errors at test time rather than runtime.
    fn validate_all_queries<L: LanguageSupport>(lang: &L) {
        // Required query - must exist and compile
        let _ = lang.reference_query();

        // Optional queries - trigger compilation if implemented
        let _ = lang.binding_query();
        let _ = lang.completion_query();
        let _ = lang.reassignment_query();
        let _ = lang.import_query();
        let _ = lang.identifier_query();
        let _ = lang.assignment_query();
        let _ = lang.destructure_query();
        let _ = lang.scope_query();
        let _ = lang.export_query();
    }

    #[test]
    fn test_javascript_queries_compile() {
        let js = javascript::JavaScript;
        validate_all_queries(&js);

        // Verify all expected queries are implemented (not just default None)
        assert!(js.binding_query().is_some(), "JavaScript should have binding_query");
        assert!(js.completion_query().is_some(), "JavaScript should have completion_query");
        assert!(js.reassignment_query().is_some(), "JavaScript should have reassignment_query");
        assert!(js.import_query().is_some(), "JavaScript should have import_query");
        assert!(js.identifier_query().is_some(), "JavaScript should have identifier_query");
        assert!(js.assignment_query().is_some(), "JavaScript should have assignment_query");
        assert!(js.destructure_query().is_some(), "JavaScript should have destructure_query");
        assert!(js.scope_query().is_some(), "JavaScript should have scope_query");
        assert!(js.export_query().is_some(), "JavaScript should have export_query");
    }

    #[test]
    fn test_typescript_queries_compile() {
        let ts = typescript::TypeScript;
        validate_all_queries(&ts);

        // Verify all expected queries are implemented
        assert!(ts.binding_query().is_some(), "TypeScript should have binding_query");
        assert!(ts.completion_query().is_some(), "TypeScript should have completion_query");
        assert!(ts.reassignment_query().is_some(), "TypeScript should have reassignment_query");
        assert!(ts.import_query().is_some(), "TypeScript should have import_query");
        assert!(ts.identifier_query().is_some(), "TypeScript should have identifier_query");
        assert!(ts.assignment_query().is_some(), "TypeScript should have assignment_query");
        assert!(ts.destructure_query().is_some(), "TypeScript should have destructure_query");
        assert!(ts.scope_query().is_some(), "TypeScript should have scope_query");
        assert!(ts.export_query().is_some(), "TypeScript should have export_query");
    }

    #[test]
    fn test_typescriptreact_queries_compile() {
        let tsx = typescript::TypeScriptReact;
        validate_all_queries(&tsx);

        // Verify all expected queries are implemented
        assert!(tsx.binding_query().is_some(), "TypeScriptReact should have binding_query");
        assert!(tsx.completion_query().is_some(), "TypeScriptReact should have completion_query");
        assert!(tsx.reassignment_query().is_some(), "TypeScriptReact should have reassignment_query");
        assert!(tsx.import_query().is_some(), "TypeScriptReact should have import_query");
        assert!(tsx.identifier_query().is_some(), "TypeScriptReact should have identifier_query");
        assert!(tsx.assignment_query().is_some(), "TypeScriptReact should have assignment_query");
        assert!(tsx.destructure_query().is_some(), "TypeScriptReact should have destructure_query");
        assert!(tsx.scope_query().is_some(), "TypeScriptReact should have scope_query");
        assert!(tsx.export_query().is_some(), "TypeScriptReact should have export_query");
    }

    #[test]
    fn test_python_queries_compile() {
        let py = python::Python;
        validate_all_queries(&py);

        // Verify all expected queries are implemented
        assert!(py.binding_query().is_some(), "Python should have binding_query");
        assert!(py.completion_query().is_some(), "Python should have completion_query");
        assert!(py.reassignment_query().is_some(), "Python should have reassignment_query");
        assert!(py.import_query().is_some(), "Python should have import_query");
        assert!(py.identifier_query().is_some(), "Python should have identifier_query");
        assert!(py.assignment_query().is_some(), "Python should have assignment_query");
        assert!(py.destructure_query().is_some(), "Python should have destructure_query");
        assert!(py.scope_query().is_some(), "Python should have scope_query");
        assert!(py.export_query().is_some(), "Python should have export_query");
    }

    #[test]
    fn test_rust_queries_compile() {
        let rs = rust::Rust;
        validate_all_queries(&rs);

        // Verify all expected queries are implemented
        assert!(rs.binding_query().is_some(), "Rust should have binding_query");
        assert!(rs.completion_query().is_some(), "Rust should have completion_query");
        assert!(rs.reassignment_query().is_some(), "Rust should have reassignment_query");
        assert!(rs.import_query().is_some(), "Rust should have import_query");
        assert!(rs.identifier_query().is_some(), "Rust should have identifier_query");
        assert!(rs.assignment_query().is_some(), "Rust should have assignment_query");
        assert!(rs.destructure_query().is_some(), "Rust should have destructure_query");
        assert!(rs.scope_query().is_some(), "Rust should have scope_query");
        assert!(rs.export_query().is_some(), "Rust should have export_query");
    }

    #[test]
    fn test_go_queries_compile() {
        let go = go::Go;
        validate_all_queries(&go);

        // Verify all expected queries are implemented
        assert!(go.binding_query().is_some(), "Go should have binding_query");
        assert!(go.completion_query().is_some(), "Go should have completion_query");
        assert!(go.reassignment_query().is_some(), "Go should have reassignment_query");
        assert!(go.import_query().is_some(), "Go should have import_query");
        assert!(go.identifier_query().is_some(), "Go should have identifier_query");
        assert!(go.assignment_query().is_some(), "Go should have assignment_query");
        assert!(go.destructure_query().is_some(), "Go should have destructure_query");
        assert!(go.scope_query().is_some(), "Go should have scope_query");
        assert!(go.export_query().is_some(), "Go should have export_query");
    }

    #[test]
    fn test_lua_queries_compile() {
        let lua_lang = lua::Lua;
        validate_all_queries(&lua_lang);

        // Verify all expected queries are implemented
        assert!(lua_lang.binding_query().is_some(), "Lua should have binding_query");
        assert!(lua_lang.completion_query().is_some(), "Lua should have completion_query");
        assert!(lua_lang.reassignment_query().is_some(), "Lua should have reassignment_query");
        assert!(lua_lang.import_query().is_some(), "Lua should have import_query");
        assert!(lua_lang.identifier_query().is_some(), "Lua should have identifier_query");
        assert!(lua_lang.assignment_query().is_some(), "Lua should have assignment_query");
        assert!(lua_lang.destructure_query().is_some(), "Lua should have destructure_query");
        assert!(lua_lang.scope_query().is_some(), "Lua should have scope_query");
        assert!(lua_lang.export_query().is_some(), "Lua should have export_query");
    }

    #[test]
    fn test_php_queries_compile() {
        let php_lang = php::Php;
        validate_all_queries(&php_lang);

        // Verify all expected queries are implemented
        assert!(php_lang.binding_query().is_some(), "PHP should have binding_query");
        assert!(php_lang.completion_query().is_some(), "PHP should have completion_query");
        assert!(php_lang.reassignment_query().is_some(), "PHP should have reassignment_query");
        assert!(php_lang.import_query().is_some(), "PHP should have import_query");
        assert!(php_lang.identifier_query().is_some(), "PHP should have identifier_query");
        assert!(php_lang.assignment_query().is_some(), "PHP should have assignment_query");
        assert!(php_lang.destructure_query().is_some(), "PHP should have destructure_query");
        assert!(php_lang.scope_query().is_some(), "PHP should have scope_query");
        assert!(php_lang.export_query().is_some(), "PHP should have export_query");
    }

    #[test]
    fn test_ruby_queries_compile() {
        let ruby_lang = ruby::Ruby;
        validate_all_queries(&ruby_lang);

        // Verify all expected queries are implemented
        assert!(ruby_lang.binding_query().is_some(), "Ruby should have binding_query");
        assert!(ruby_lang.completion_query().is_some(), "Ruby should have completion_query");
        assert!(ruby_lang.reassignment_query().is_some(), "Ruby should have reassignment_query");
        assert!(ruby_lang.import_query().is_some(), "Ruby should have import_query");
        assert!(ruby_lang.identifier_query().is_some(), "Ruby should have identifier_query");
        assert!(ruby_lang.assignment_query().is_some(), "Ruby should have assignment_query");
        assert!(ruby_lang.destructure_query().is_some(), "Ruby should have destructure_query");
        assert!(ruby_lang.scope_query().is_some(), "Ruby should have scope_query");
        assert!(ruby_lang.export_query().is_some(), "Ruby should have export_query");
    }

    /// Comprehensive test that validates all queries for all supported languages.
    /// This is the main test that ensures no query compilation failures at runtime.
    #[test]
    fn test_all_language_queries_compile() {
        // JavaScript
        let js = javascript::JavaScript;
        validate_all_queries(&js);

        // TypeScript
        let ts = typescript::TypeScript;
        validate_all_queries(&ts);

        // TypeScriptReact (TSX)
        let tsx = typescript::TypeScriptReact;
        validate_all_queries(&tsx);

        // Python
        let py = python::Python;
        validate_all_queries(&py);

        // Rust
        let rs = rust::Rust;
        validate_all_queries(&rs);

        // Go
        let go = go::Go;
        validate_all_queries(&go);

        // Lua
        let lua_lang = lua::Lua;
        validate_all_queries(&lua_lang);

        // PHP
        let php_lang = php::Php;
        validate_all_queries(&php_lang);

        // Ruby
        let ruby_lang = ruby::Ruby;
        validate_all_queries(&ruby_lang);

        // C
        let c_lang = c::C;
        validate_all_queries(&c_lang);

        // C++
        let cpp_lang = cpp::Cpp;
        validate_all_queries(&cpp_lang);

        // Java
        let java_lang = java::Java;
        validate_all_queries(&java_lang);

        // Kotlin
        let kotlin_lang = kotlin::Kotlin;
        validate_all_queries(&kotlin_lang);

        // C#
        let csharp_lang = csharp::CSharp;
        validate_all_queries(&csharp_lang);

        // Elixir
        let elixir_lang = elixir::Elixir;
        validate_all_queries(&elixir_lang);

        // Zig
        let zig_lang = zig::Zig;
        validate_all_queries(&zig_lang);

        // Bash
        let bash_lang = bash::Bash;
        validate_all_queries(&bash_lang);
    }

    #[test]
    fn test_c_queries_compile() {
        let c_lang = c::C;
        validate_all_queries(&c_lang);

        assert!(c_lang.binding_query().is_some(), "C should have binding_query");
        assert!(c_lang.completion_query().is_some(), "C should have completion_query");
        assert!(c_lang.reassignment_query().is_some(), "C should have reassignment_query");
        assert!(c_lang.import_query().is_some(), "C should have import_query");
        assert!(c_lang.identifier_query().is_some(), "C should have identifier_query");
        assert!(c_lang.assignment_query().is_some(), "C should have assignment_query");
        assert!(c_lang.destructure_query().is_some(), "C should have destructure_query");
        assert!(c_lang.scope_query().is_some(), "C should have scope_query");
        assert!(c_lang.export_query().is_some(), "C should have export_query");
    }

    #[test]
    fn test_cpp_queries_compile() {
        let cpp_lang = cpp::Cpp;
        validate_all_queries(&cpp_lang);

        assert!(cpp_lang.binding_query().is_some(), "C++ should have binding_query");
        assert!(cpp_lang.completion_query().is_some(), "C++ should have completion_query");
        assert!(cpp_lang.reassignment_query().is_some(), "C++ should have reassignment_query");
        assert!(cpp_lang.import_query().is_some(), "C++ should have import_query");
        assert!(cpp_lang.identifier_query().is_some(), "C++ should have identifier_query");
        assert!(cpp_lang.assignment_query().is_some(), "C++ should have assignment_query");
        assert!(cpp_lang.destructure_query().is_some(), "C++ should have destructure_query");
        assert!(cpp_lang.scope_query().is_some(), "C++ should have scope_query");
        assert!(cpp_lang.export_query().is_some(), "C++ should have export_query");
    }

    #[test]
    fn test_java_queries_compile() {
        let java_lang = java::Java;
        validate_all_queries(&java_lang);

        assert!(java_lang.binding_query().is_some(), "Java should have binding_query");
        assert!(java_lang.completion_query().is_some(), "Java should have completion_query");
        assert!(java_lang.reassignment_query().is_some(), "Java should have reassignment_query");
        assert!(java_lang.import_query().is_some(), "Java should have import_query");
        assert!(java_lang.identifier_query().is_some(), "Java should have identifier_query");
        assert!(java_lang.assignment_query().is_some(), "Java should have assignment_query");
        assert!(java_lang.destructure_query().is_some(), "Java should have destructure_query");
        assert!(java_lang.scope_query().is_some(), "Java should have scope_query");
        assert!(java_lang.export_query().is_some(), "Java should have export_query");
    }

    #[test]
    fn test_kotlin_queries_compile() {
        let kotlin_lang = kotlin::Kotlin;
        validate_all_queries(&kotlin_lang);

        assert!(kotlin_lang.binding_query().is_some(), "Kotlin should have binding_query");
        assert!(kotlin_lang.completion_query().is_some(), "Kotlin should have completion_query");
        assert!(kotlin_lang.reassignment_query().is_some(), "Kotlin should have reassignment_query");
        assert!(kotlin_lang.import_query().is_some(), "Kotlin should have import_query");
        assert!(kotlin_lang.identifier_query().is_some(), "Kotlin should have identifier_query");
        assert!(kotlin_lang.assignment_query().is_some(), "Kotlin should have assignment_query");
        assert!(kotlin_lang.destructure_query().is_some(), "Kotlin should have destructure_query");
        assert!(kotlin_lang.scope_query().is_some(), "Kotlin should have scope_query");
        assert!(kotlin_lang.export_query().is_some(), "Kotlin should have export_query");
    }

    #[test]
    fn test_csharp_queries_compile() {
        let csharp_lang = csharp::CSharp;
        validate_all_queries(&csharp_lang);

        assert!(csharp_lang.binding_query().is_some(), "C# should have binding_query");
        assert!(csharp_lang.completion_query().is_some(), "C# should have completion_query");
        assert!(csharp_lang.reassignment_query().is_some(), "C# should have reassignment_query");
        assert!(csharp_lang.import_query().is_some(), "C# should have import_query");
        assert!(csharp_lang.identifier_query().is_some(), "C# should have identifier_query");
        assert!(csharp_lang.assignment_query().is_some(), "C# should have assignment_query");
        assert!(csharp_lang.destructure_query().is_some(), "C# should have destructure_query");
        assert!(csharp_lang.scope_query().is_some(), "C# should have scope_query");
        assert!(csharp_lang.export_query().is_some(), "C# should have export_query");
    }

    #[test]
    fn test_elixir_queries_compile() {
        let elixir_lang = elixir::Elixir;
        validate_all_queries(&elixir_lang);

        assert!(elixir_lang.binding_query().is_some(), "Elixir should have binding_query");
        assert!(elixir_lang.completion_query().is_some(), "Elixir should have completion_query");
        assert!(elixir_lang.reassignment_query().is_some(), "Elixir should have reassignment_query");
        assert!(elixir_lang.import_query().is_some(), "Elixir should have import_query");
        assert!(elixir_lang.identifier_query().is_some(), "Elixir should have identifier_query");
        assert!(elixir_lang.assignment_query().is_some(), "Elixir should have assignment_query");
        assert!(elixir_lang.destructure_query().is_some(), "Elixir should have destructure_query");
        assert!(elixir_lang.scope_query().is_some(), "Elixir should have scope_query");
        assert!(elixir_lang.export_query().is_some(), "Elixir should have export_query");
    }

    #[test]
    fn test_zig_queries_compile() {
        let zig_lang = zig::Zig;
        validate_all_queries(&zig_lang);

        assert!(zig_lang.binding_query().is_some(), "Zig should have binding_query");
        assert!(zig_lang.completion_query().is_some(), "Zig should have completion_query");
        assert!(zig_lang.reassignment_query().is_some(), "Zig should have reassignment_query");
        assert!(zig_lang.import_query().is_some(), "Zig should have import_query");
        assert!(zig_lang.identifier_query().is_some(), "Zig should have identifier_query");
        assert!(zig_lang.assignment_query().is_some(), "Zig should have assignment_query");
        assert!(zig_lang.destructure_query().is_some(), "Zig should have destructure_query");
        assert!(zig_lang.scope_query().is_some(), "Zig should have scope_query");
        assert!(zig_lang.export_query().is_some(), "Zig should have export_query");
    }

    #[test]
    fn test_bash_queries_compile() {
        let bash_lang = bash::Bash;
        validate_all_queries(&bash_lang);

        assert!(bash_lang.binding_query().is_some(), "Bash should have binding_query");
        assert!(bash_lang.completion_query().is_some(), "Bash should have completion_query");
        assert!(bash_lang.reassignment_query().is_some(), "Bash should have reassignment_query");
        assert!(bash_lang.import_query().is_some(), "Bash should have import_query");
        assert!(bash_lang.identifier_query().is_some(), "Bash should have identifier_query");
        assert!(bash_lang.assignment_query().is_some(), "Bash should have assignment_query");
        assert!(bash_lang.destructure_query().is_some(), "Bash should have destructure_query");
        assert!(bash_lang.scope_query().is_some(), "Bash should have scope_query");
        assert!(bash_lang.export_query().is_some(), "Bash should have export_query");
    }
}
