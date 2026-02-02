use crate::languages::LanguageSupport;
use std::sync::OnceLock;
use tree_sitter::{Language, Query};
use tracing::error;

pub struct Elixir;

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
                language = "elixir",
                query = query_name,
                error = %e,
                "Failed to compile query, using empty fallback"
            );
            Query::new(grammar, "").unwrap_or_else(|_| {
                panic!(
                    "Failed to create empty fallback query for Elixir {}",
                    query_name
                )
            })
        }
    }
}

impl LanguageSupport for Elixir {
    fn id(&self) -> &'static str {
        "elixir"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ex", "exs"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["elixir"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_elixir::LANGUAGE.into()
    }

    fn reference_query(&self) -> &Query {
        REFERENCE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/elixir/references.scm"),
                "references",
            )
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(BINDING_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/elixir/bindings.scm"),
                "bindings",
            )
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(IMPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/elixir/imports.scm"),
                "imports",
            )
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(COMPLETION_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/elixir/completion.scm"),
                "completion",
            )
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(REASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/elixir/reassignments.scm"),
                "reassignments",
            )
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(IDENTIFIER_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/elixir/identifiers.scm"),
                "identifiers",
            )
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(EXPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/elixir/exports.scm"),
                "exports",
            )
        }))
    }

    fn assignment_query(&self) -> Option<&Query> {
        Some(ASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/elixir/assignments.scm"),
                "assignments",
            )
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(DESTRUCTURE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/elixir/destructures.scm"),
                "destructures",
            )
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(SCOPE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/elixir/scopes.scm"),
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
        &["comment"]
    }

    fn is_scope_node(&self, node: tree_sitter::Node) -> bool {
        matches!(
            node.kind(),
            "do_block"
                | "anonymous_function"
                | "call"
                | "stab_clause"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_elixir() -> Elixir {
        Elixir
    }

    #[test]
    fn test_id() {
        assert_eq!(get_elixir().id(), "elixir");
    }

    #[test]
    fn test_extensions() {
        let exts = get_elixir().extensions();
        assert!(exts.contains(&"ex"));
        assert!(exts.contains(&"exs"));
    }

    #[test]
    fn test_language_ids() {
        let ids = get_elixir().language_ids();
        assert!(ids.contains(&"elixir"));
    }

    #[test]
    fn test_grammar_compiles() {
        let elixir = get_elixir();
        let _grammar = elixir.grammar();
    }

    #[test]
    fn test_reference_query_compiles() {
        let elixir = get_elixir();
        let _query = elixir.reference_query();
    }

    #[test]
    fn test_binding_query_compiles() {
        let elixir = get_elixir();
        assert!(elixir.binding_query().is_some());
    }

    #[test]
    fn test_import_query_compiles() {
        let elixir = get_elixir();
        assert!(elixir.import_query().is_some());
    }

    #[test]
    fn test_completion_query_compiles() {
        let elixir = get_elixir();
        assert!(elixir.completion_query().is_some());
    }

    #[test]
    fn test_reassignment_query_compiles() {
        let elixir = get_elixir();
        assert!(elixir.reassignment_query().is_some());
    }

    #[test]
    fn test_identifier_query_compiles() {
        let elixir = get_elixir();
        assert!(elixir.identifier_query().is_some());
    }

    #[test]
    fn test_export_query_compiles() {
        let elixir = get_elixir();
        assert!(elixir.export_query().is_some());
    }

    #[test]
    fn test_assignment_query_compiles() {
        let elixir = get_elixir();
        assert!(elixir.assignment_query().is_some());
    }

    #[test]
    fn test_scope_query_compiles() {
        let elixir = get_elixir();
        assert!(elixir.scope_query().is_some());
    }

    #[test]
    fn test_destructure_query_compiles() {
        let elixir = get_elixir();
        assert!(elixir.destructure_query().is_some());
    }
}
