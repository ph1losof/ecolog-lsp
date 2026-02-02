use crate::languages::LanguageSupport;
use std::sync::OnceLock;
use tree_sitter::{Language, Query};
use tracing::error;

pub struct Java;

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

fn compile_query(grammar: &Language, source: &str, query_name: &str) -> Query {
    match Query::new(grammar, source) {
        Ok(query) => query,
        Err(e) => {
            error!(
                language = "java",
                query = query_name,
                error = %e,
                "Failed to compile query, using empty fallback"
            );
            Query::new(grammar, "").unwrap_or_else(|_| {
                panic!(
                    "Failed to create empty fallback query for Java {}",
                    query_name
                )
            })
        }
    }
}

impl LanguageSupport for Java {
    fn id(&self) -> &'static str {
        "java"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["java"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["java"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_java::LANGUAGE.into()
    }

    fn reference_query(&self) -> &Query {
        REFERENCE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/java/references.scm"),
                "references",
            )
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(BINDING_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/java/bindings.scm"),
                "bindings",
            )
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(IMPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/java/imports.scm"),
                "imports",
            )
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(COMPLETION_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/java/completion.scm"),
                "completion",
            )
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(REASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/java/reassignments.scm"),
                "reassignments",
            )
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(IDENTIFIER_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/java/identifiers.scm"),
                "identifiers",
            )
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(EXPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/java/exports.scm"),
                "exports",
            )
        }))
    }

    fn assignment_query(&self) -> Option<&Query> {
        Some(ASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/java/assignments.scm"),
                "assignments",
            )
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(DESTRUCTURE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/java/destructures.scm"),
                "destructures",
            )
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(SCOPE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/java/scopes.scm"),
                "scopes",
            )
        }))
    }

    fn completion_trigger_characters(&self) -> &'static [&'static str] {
        &["(\"", "('"]
    }

    fn is_standard_env_object(&self, name: &str) -> bool {
        name == "System"
    }

    fn comment_node_kinds(&self) -> &'static [&'static str] {
        &["line_comment", "block_comment"]
    }

    fn is_scope_node(&self, node: tree_sitter::Node) -> bool {
        matches!(
            node.kind(),
            "method_declaration"
                | "constructor_declaration"
                | "block"
                | "for_statement"
                | "enhanced_for_statement"
                | "if_statement"
                | "while_statement"
                | "do_statement"
                | "switch_expression"
                | "try_statement"
                | "catch_clause"
                | "class_declaration"
                | "interface_declaration"
                | "lambda_expression"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_java() -> Java {
        Java
    }

    #[test]
    fn test_id() {
        assert_eq!(get_java().id(), "java");
    }

    #[test]
    fn test_extensions() {
        let exts = get_java().extensions();
        assert!(exts.contains(&"java"));
    }

    #[test]
    fn test_language_ids() {
        let ids = get_java().language_ids();
        assert!(ids.contains(&"java"));
    }

    #[test]
    fn test_grammar_compiles() {
        let java = get_java();
        let _grammar = java.grammar();
    }

    #[test]
    fn test_reference_query_compiles() {
        let java = get_java();
        let _query = java.reference_query();
    }

    #[test]
    fn test_binding_query_compiles() {
        let java = get_java();
        assert!(java.binding_query().is_some());
    }

    #[test]
    fn test_import_query_compiles() {
        let java = get_java();
        assert!(java.import_query().is_some());
    }

    #[test]
    fn test_completion_query_compiles() {
        let java = get_java();
        assert!(java.completion_query().is_some());
    }

    #[test]
    fn test_reassignment_query_compiles() {
        let java = get_java();
        assert!(java.reassignment_query().is_some());
    }

    #[test]
    fn test_identifier_query_compiles() {
        let java = get_java();
        assert!(java.identifier_query().is_some());
    }

    #[test]
    fn test_export_query_compiles() {
        let java = get_java();
        assert!(java.export_query().is_some());
    }

    #[test]
    fn test_assignment_query_compiles() {
        let java = get_java();
        assert!(java.assignment_query().is_some());
    }

    #[test]
    fn test_scope_query_compiles() {
        let java = get_java();
        assert!(java.scope_query().is_some());
    }

    #[test]
    fn test_destructure_query_compiles() {
        let java = get_java();
        assert!(java.destructure_query().is_some());
    }
}
