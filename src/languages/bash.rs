use crate::languages::LanguageSupport;
use std::sync::OnceLock;
use tree_sitter::{Language, Query};
use tracing::error;

pub struct Bash;

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
                language = "bash",
                query = query_name,
                error = %e,
                "Failed to compile query, using empty fallback"
            );
            Query::new(grammar, "").unwrap_or_else(|_| {
                panic!(
                    "Failed to create empty fallback query for Bash {}",
                    query_name
                )
            })
        }
    }
}

impl LanguageSupport for Bash {
    fn id(&self) -> &'static str {
        "bash"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["sh", "bash", "zsh", "zshrc", "bashrc", "bash_profile", "zprofile"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["shellscript", "bash", "sh", "zsh"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_bash::LANGUAGE.into()
    }

    fn reference_query(&self) -> &Query {
        REFERENCE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/bash/references.scm"),
                "references",
            )
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(BINDING_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/bash/bindings.scm"),
                "bindings",
            )
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(IMPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/bash/imports.scm"),
                "imports",
            )
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(COMPLETION_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/bash/completion.scm"),
                "completion",
            )
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(REASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/bash/reassignments.scm"),
                "reassignments",
            )
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(IDENTIFIER_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/bash/identifiers.scm"),
                "identifiers",
            )
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(EXPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/bash/exports.scm"),
                "exports",
            )
        }))
    }

    fn assignment_query(&self) -> Option<&Query> {
        Some(ASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/bash/assignments.scm"),
                "assignments",
            )
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(DESTRUCTURE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/bash/destructures.scm"),
                "destructures",
            )
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(SCOPE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/bash/scopes.scm"),
                "scopes",
            )
        }))
    }

    fn completion_trigger_characters(&self) -> &'static [&'static str] {
        &["$", "${"]
    }

    fn is_standard_env_object(&self, _name: &str) -> bool {
        // In bash, all variable expansions ($VAR, ${VAR}) are env var access
        true
    }

    fn comment_node_kinds(&self) -> &'static [&'static str] {
        &["comment"]
    }

    fn is_scope_node(&self, node: tree_sitter::Node) -> bool {
        matches!(
            node.kind(),
            "function_definition"
                | "compound_statement"
                | "subshell"
                | "for_statement"
                | "while_statement"
                | "if_statement"
                | "case_statement"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_bash() -> Bash {
        Bash
    }

    #[test]
    fn test_id() {
        assert_eq!(get_bash().id(), "bash");
    }

    #[test]
    fn test_extensions() {
        let exts = get_bash().extensions();
        assert!(exts.contains(&"sh"));
        assert!(exts.contains(&"bash"));
        assert!(exts.contains(&"zsh"));
    }

    #[test]
    fn test_language_ids() {
        let ids = get_bash().language_ids();
        assert!(ids.contains(&"bash"));
        assert!(ids.contains(&"shellscript"));
    }

    #[test]
    fn test_grammar_compiles() {
        let bash = get_bash();
        let _grammar = bash.grammar();
    }

    #[test]
    fn test_reference_query_compiles() {
        let bash = get_bash();
        let _query = bash.reference_query();
    }

    #[test]
    fn test_binding_query_compiles() {
        let bash = get_bash();
        assert!(bash.binding_query().is_some());
    }

    #[test]
    fn test_import_query_compiles() {
        let bash = get_bash();
        assert!(bash.import_query().is_some());
    }

    #[test]
    fn test_completion_query_compiles() {
        let bash = get_bash();
        assert!(bash.completion_query().is_some());
    }

    #[test]
    fn test_reassignment_query_compiles() {
        let bash = get_bash();
        assert!(bash.reassignment_query().is_some());
    }

    #[test]
    fn test_identifier_query_compiles() {
        let bash = get_bash();
        assert!(bash.identifier_query().is_some());
    }

    #[test]
    fn test_export_query_compiles() {
        let bash = get_bash();
        assert!(bash.export_query().is_some());
    }

    #[test]
    fn test_assignment_query_compiles() {
        let bash = get_bash();
        assert!(bash.assignment_query().is_some());
    }

    #[test]
    fn test_scope_query_compiles() {
        let bash = get_bash();
        assert!(bash.scope_query().is_some());
    }

    #[test]
    fn test_destructure_query_compiles() {
        let bash = get_bash();
        assert!(bash.destructure_query().is_some());
    }
}
