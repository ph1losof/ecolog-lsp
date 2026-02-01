use crate::languages::LanguageSupport;
use crate::types::EnvSourceKind;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};
use tracing::error;

pub struct Php;

static REFERENCE_QUERY: OnceLock<Query> = OnceLock::new();
static BINDING_QUERY: OnceLock<Query> = OnceLock::new();
static IMPORT_QUERY: OnceLock<Query> = OnceLock::new();
static COMPLETION_QUERY: OnceLock<Query> = OnceLock::new();
static REASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static IDENTIFIER_QUERY: OnceLock<Query> = OnceLock::new();
static EXPORT_QUERY: OnceLock<Query> = OnceLock::new();

static ASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static DESTRUCTURE_QUERY: OnceLock<Query> = OnceLock::new();
static SCOPE_QUERY: OnceLock<Query> = OnceLock::new();

/// Compiles a tree-sitter query, logging an error and returning an empty fallback on failure.
/// This prevents the LSP from crashing due to query compilation errors.
fn compile_query(grammar: &Language, source: &str, query_name: &str) -> Query {
    match Query::new(grammar, source) {
        Ok(query) => query,
        Err(e) => {
            error!(
                language = "php",
                query = query_name,
                error = %e,
                "Failed to compile query, using empty fallback"
            );
            // Return an empty query that matches nothing, allowing the LSP to continue
            Query::new(grammar, "").unwrap_or_else(|_| {
                panic!(
                    "Failed to create empty fallback query for PHP {}",
                    query_name
                )
            })
        }
    }
}

impl LanguageSupport for Php {
    fn id(&self) -> &'static str {
        "php"
    }

    fn is_standard_env_object(&self, name: &str) -> bool {
        // PHP uses global superglobals $_ENV and $_SERVER
        matches!(name, "$_ENV" | "$_SERVER" | "getenv" | "env")
    }

    fn default_env_object_name(&self) -> Option<&'static str> {
        Some("$_ENV")
    }

    fn is_scope_node(&self, node: Node) -> bool {
        matches!(
            node.kind(),
            "program"
                | "function_definition"
                | "method_declaration"
                | "class_declaration"
                | "anonymous_function"
                | "arrow_function"
                | "for_statement"
                | "foreach_statement"
                | "if_statement"
                | "try_statement"
                | "while_statement"
                | "do_statement"
                | "switch_statement"
        )
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["php", "phtml", "php3", "php4", "php5", "phps"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["php"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_php::LANGUAGE_PHP.into()
    }

    fn reference_query(&self) -> &Query {
        REFERENCE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/php/references.scm"),
                "references",
            )
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(BINDING_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/php/bindings.scm"),
                "bindings",
            )
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(IMPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/php/imports.scm"),
                "imports",
            )
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(COMPLETION_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/php/completion.scm"),
                "completion",
            )
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(REASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/php/reassignments.scm"),
                "reassignments",
            )
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(IDENTIFIER_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/php/identifiers.scm"),
                "identifiers",
            )
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(EXPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/php/exports.scm"),
                "exports",
            )
        }))
    }

    fn assignment_query(&self) -> Option<&Query> {
        Some(ASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/php/assignments.scm"),
                "assignments",
            )
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(DESTRUCTURE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/php/destructures.scm"),
                "destructures",
            )
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(SCOPE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/php/scopes.scm"),
                "scopes",
            )
        }))
    }

    fn is_env_source_node(&self, node: Node, source: &[u8]) -> Option<EnvSourceKind> {
        // Detect $_ENV and $_SERVER superglobals
        if node.kind() == "variable_name" {
            let text = node.utf8_text(source).ok()?;
            if text == "$_ENV" || text == "_ENV" {
                return Some(EnvSourceKind::Object {
                    canonical_name: "$_ENV".into(),
                });
            }
            if text == "$_SERVER" || text == "_SERVER" {
                return Some(EnvSourceKind::Object {
                    canonical_name: "$_SERVER".into(),
                });
            }
        }

        None
    }

    fn known_env_modules(&self) -> &'static [&'static str] {
        // PHP doesn't have modules in the same way, but these are common env-related patterns
        &[]
    }

    fn completion_trigger_characters(&self) -> &'static [&'static str] {
        // Trigger on opening quote after array subscript or function call
        &["[\"", "['", "(\"", "('"]
    }

    fn strip_quotes<'a>(&self, text: &'a str) -> &'a str {
        text.trim_matches(|c| c == '"' || c == '\'')
    }

    fn extract_var_name(&self, node: Node, source: &[u8]) -> Option<compact_str::CompactString> {
        node.utf8_text(source)
            .ok()
            .map(|s| compact_str::CompactString::from(self.strip_quotes(s)))
    }

    fn extract_property_access(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        byte_offset: usize,
    ) -> Option<(compact_str::CompactString, compact_str::CompactString)> {
        let node = tree
            .root_node()
            .descendant_for_byte_range(byte_offset, byte_offset)?;

        // In PHP, property access is through member_access_expression
        let member_access = if node.kind() == "member_access_expression" {
            node
        } else if let Some(parent) = node.parent() {
            if parent.kind() == "member_access_expression" {
                parent
            } else {
                return None;
            }
        } else {
            return None;
        };

        let object_node = member_access.child_by_field_name("object")?;
        let name_node = member_access.child_by_field_name("name")?;

        let object_name = object_node.utf8_text(content.as_bytes()).ok()?;
        let property_name = name_node.utf8_text(content.as_bytes()).ok()?;

        Some((object_name.into(), property_name.into()))
    }

    fn comment_node_kinds(&self) -> &'static [&'static str] {
        &["comment"]
    }

    fn is_root_node(&self, node: Node) -> bool {
        node.kind() == "program"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_php() -> Php {
        Php
    }

    #[test]
    fn test_id() {
        assert_eq!(get_php().id(), "php");
    }

    #[test]
    fn test_extensions() {
        let exts = get_php().extensions();
        assert!(exts.contains(&"php"));
        assert!(exts.contains(&"phtml"));
    }

    #[test]
    fn test_language_ids() {
        let ids = get_php().language_ids();
        assert!(ids.contains(&"php"));
    }

    #[test]
    fn test_is_standard_env_object() {
        let php = get_php();
        assert!(php.is_standard_env_object("$_ENV"));
        assert!(php.is_standard_env_object("$_SERVER"));
        assert!(php.is_standard_env_object("getenv"));
        assert!(php.is_standard_env_object("env"));
        assert!(!php.is_standard_env_object("process"));
    }

    #[test]
    fn test_default_env_object_name() {
        assert_eq!(get_php().default_env_object_name(), Some("$_ENV"));
    }

    #[test]
    fn test_grammar_compiles() {
        let php = get_php();
        let _grammar = php.grammar();
    }

    #[test]
    fn test_strip_quotes() {
        let php = get_php();
        assert_eq!(php.strip_quotes("\"hello\""), "hello");
        assert_eq!(php.strip_quotes("'world'"), "world");
        assert_eq!(php.strip_quotes("noquotes"), "noquotes");
    }

    #[test]
    fn test_is_env_source_node_env() {
        let php = get_php();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&php.grammar()).unwrap();

        let code = "<?php\n$x = $_ENV['VAR'];";
        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        fn walk_tree(cursor: &mut tree_sitter::TreeCursor, php: &Php, code: &str) -> bool {
            loop {
                let node = cursor.node();
                if node.kind() == "variable_name" {
                    if let Some(kind) = php.is_env_source_node(node, code.as_bytes()) {
                        if let EnvSourceKind::Object { canonical_name } = kind {
                            if canonical_name == "$_ENV" {
                                return true;
                            }
                        }
                    }
                }

                if cursor.goto_first_child() {
                    if walk_tree(cursor, php, code) {
                        return true;
                    }
                    cursor.goto_parent();
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            false
        }

        let mut cursor = root.walk();
        let found = walk_tree(&mut cursor, &php, code);
        assert!(found, "Should detect $_ENV as env source");
    }

    #[test]
    fn test_is_scope_node() {
        let php = get_php();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&php.grammar()).unwrap();

        let code = "<?php\nfunction test() { }";
        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        fn find_node_of_kind<'a>(
            node: tree_sitter::Node<'a>,
            kind: &str,
        ) -> Option<tree_sitter::Node<'a>> {
            if node.kind() == kind {
                return Some(node);
            }
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if let Some(found) = find_node_of_kind(child, kind) {
                        return Some(found);
                    }
                }
            }
            None
        }

        if let Some(func) = find_node_of_kind(root, "function_definition") {
            assert!(php.is_scope_node(func));
        }
    }
}
