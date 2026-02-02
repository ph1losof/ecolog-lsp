use crate::languages::LanguageSupport;
use std::sync::OnceLock;
use tree_sitter::{Language, Query};
use tracing::error;

pub struct Zig;

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
                language = "zig",
                query = query_name,
                error = %e,
                "Failed to compile query, using empty fallback"
            );
            Query::new(grammar, "").unwrap_or_else(|_| {
                panic!(
                    "Failed to create empty fallback query for Zig {}",
                    query_name
                )
            })
        }
    }
}

impl LanguageSupport for Zig {
    fn id(&self) -> &'static str {
        "zig"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["zig"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["zig"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_zig::LANGUAGE.into()
    }

    fn reference_query(&self) -> &Query {
        REFERENCE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/zig/references.scm"),
                "references",
            )
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(BINDING_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/zig/bindings.scm"),
                "bindings",
            )
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(IMPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/zig/imports.scm"),
                "imports",
            )
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(COMPLETION_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/zig/completion.scm"),
                "completion",
            )
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(REASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/zig/reassignments.scm"),
                "reassignments",
            )
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(IDENTIFIER_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/zig/identifiers.scm"),
                "identifiers",
            )
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(EXPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/zig/exports.scm"),
                "exports",
            )
        }))
    }

    fn assignment_query(&self) -> Option<&Query> {
        Some(ASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/zig/assignments.scm"),
                "assignments",
            )
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(DESTRUCTURE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/zig/destructures.scm"),
                "destructures",
            )
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(SCOPE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/zig/scopes.scm"),
                "scopes",
            )
        }))
    }

    fn completion_trigger_characters(&self) -> &'static [&'static str] {
        &["(\"", "('"]
    }

    fn is_standard_env_object(&self, name: &str) -> bool {
        name == "std"
    }

    fn comment_node_kinds(&self) -> &'static [&'static str] {
        &["line_comment", "doc_comment", "container_doc_comment"]
    }

    fn is_scope_node(&self, node: tree_sitter::Node) -> bool {
        matches!(
            node.kind(),
            "FnProto"
                | "Block"
                | "ForStatement"
                | "WhileStatement"
                | "IfStatement"
                | "SwitchExpr"
                | "ContainerDecl"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_zig() -> Zig {
        Zig
    }

    #[test]
    fn test_id() {
        assert_eq!(get_zig().id(), "zig");
    }

    #[test]
    fn test_extensions() {
        let exts = get_zig().extensions();
        assert!(exts.contains(&"zig"));
    }

    #[test]
    fn test_language_ids() {
        let ids = get_zig().language_ids();
        assert!(ids.contains(&"zig"));
    }

    #[test]
    fn test_grammar_compiles() {
        let zig = get_zig();
        let _grammar = zig.grammar();
    }

    #[test]
    fn test_reference_query_compiles() {
        let zig = get_zig();
        let _query = zig.reference_query();
    }

    #[test]
    fn test_binding_query_compiles() {
        let zig = get_zig();
        assert!(zig.binding_query().is_some());
    }

    #[test]
    fn test_import_query_compiles() {
        let zig = get_zig();
        assert!(zig.import_query().is_some());
    }

    #[test]
    fn test_completion_query_compiles() {
        let zig = get_zig();
        assert!(zig.completion_query().is_some());
    }

    #[test]
    fn test_reassignment_query_compiles() {
        let zig = get_zig();
        assert!(zig.reassignment_query().is_some());
    }

    #[test]
    fn test_identifier_query_compiles() {
        let zig = get_zig();
        assert!(zig.identifier_query().is_some());
    }

    #[test]
    fn test_export_query_compiles() {
        let zig = get_zig();
        assert!(zig.export_query().is_some());
    }

    #[test]
    fn test_assignment_query_compiles() {
        let zig = get_zig();
        assert!(zig.assignment_query().is_some());
    }

    #[test]
    fn test_scope_query_compiles() {
        let zig = get_zig();
        assert!(zig.scope_query().is_some());
    }

    #[test]
    fn test_destructure_query_compiles() {
        let zig = get_zig();
        assert!(zig.destructure_query().is_some());
    }
}
