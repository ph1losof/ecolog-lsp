use crate::languages::LanguageSupport;
use tree_sitter::Language;

pub struct Zig;

define_language_queries!("zig", "zig");

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

    impl_language_queries!("zig");

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
