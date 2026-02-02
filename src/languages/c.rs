use crate::languages::LanguageSupport;
use std::sync::OnceLock;
use tree_sitter::{Language, Query};
use tracing::error;

pub struct C;

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
                language = "c",
                query = query_name,
                error = %e,
                "Failed to compile query, using empty fallback"
            );
            Query::new(grammar, "").unwrap_or_else(|_| {
                panic!("Failed to create empty fallback query for C {}", query_name)
            })
        }
    }
}

impl LanguageSupport for C {
    fn id(&self) -> &'static str {
        "c"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["c", "h"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["c"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_c::LANGUAGE.into()
    }

    fn reference_query(&self) -> &Query {
        REFERENCE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/c/references.scm"),
                "references",
            )
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(BINDING_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/c/bindings.scm"),
                "bindings",
            )
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(IMPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/c/imports.scm"),
                "imports",
            )
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(COMPLETION_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/c/completion.scm"),
                "completion",
            )
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(REASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/c/reassignments.scm"),
                "reassignments",
            )
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(IDENTIFIER_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/c/identifiers.scm"),
                "identifiers",
            )
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(EXPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/c/exports.scm"),
                "exports",
            )
        }))
    }

    fn assignment_query(&self) -> Option<&Query> {
        Some(ASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/c/assignments.scm"),
                "assignments",
            )
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(DESTRUCTURE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/c/destructures.scm"),
                "destructures",
            )
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(SCOPE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/c/scopes.scm"),
                "scopes",
            )
        }))
    }

    fn completion_trigger_characters(&self) -> &'static [&'static str] {
        &["(\"", "('"]
    }

    fn is_standard_env_object(&self, name: &str) -> bool {
        matches!(name, "getenv" | "secure_getenv")
    }

    fn comment_node_kinds(&self) -> &'static [&'static str] {
        &["comment"]
    }

    fn is_scope_node(&self, node: tree_sitter::Node) -> bool {
        matches!(
            node.kind(),
            "function_definition"
                | "compound_statement"
                | "for_statement"
                | "if_statement"
                | "while_statement"
                | "do_statement"
                | "switch_statement"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_c() -> C {
        C
    }

    #[test]
    fn test_id() {
        assert_eq!(get_c().id(), "c");
    }

    #[test]
    fn test_extensions() {
        let exts = get_c().extensions();
        assert!(exts.contains(&"c"));
        assert!(exts.contains(&"h"));
    }

    #[test]
    fn test_language_ids() {
        let ids = get_c().language_ids();
        assert!(ids.contains(&"c"));
    }

    #[test]
    fn test_grammar_compiles() {
        let c = get_c();
        let _grammar = c.grammar();
    }

    #[test]
    fn test_reference_query_compiles() {
        let c = get_c();
        let _query = c.reference_query();
    }

    #[test]
    fn test_binding_query_compiles() {
        let c = get_c();
        assert!(c.binding_query().is_some());
    }

    #[test]
    fn test_import_query_compiles() {
        let c = get_c();
        assert!(c.import_query().is_some());
    }

    #[test]
    fn test_completion_query_compiles() {
        let c = get_c();
        assert!(c.completion_query().is_some());
    }

    #[test]
    fn test_reassignment_query_compiles() {
        let c = get_c();
        assert!(c.reassignment_query().is_some());
    }

    #[test]
    fn test_identifier_query_compiles() {
        let c = get_c();
        assert!(c.identifier_query().is_some());
    }

    #[test]
    fn test_export_query_compiles() {
        let c = get_c();
        assert!(c.export_query().is_some());
    }

    #[test]
    fn test_assignment_query_compiles() {
        let c = get_c();
        assert!(c.assignment_query().is_some());
    }

    #[test]
    fn test_scope_query_compiles() {
        let c = get_c();
        assert!(c.scope_query().is_some());
    }

    #[test]
    fn test_destructure_query_compiles() {
        let c = get_c();
        assert!(c.destructure_query().is_some());
    }
}
