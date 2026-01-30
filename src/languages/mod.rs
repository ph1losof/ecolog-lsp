use crate::types::{EnvSourceKind, ScopeKind};
use compact_str::CompactString;
use tree_sitter::{Language, Node, Query};

pub mod go;
pub mod javascript;
pub mod lua;
pub mod python;
pub mod registry;
pub mod rust;
pub mod typescript;

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
    }
}
