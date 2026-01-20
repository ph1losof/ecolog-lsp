use crate::types::{EnvSourceKind, ScopeKind};
use compact_str::CompactString;
use tree_sitter::{Language, Node, Query};

pub mod go;
pub mod javascript;
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
