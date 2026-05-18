use crate::languages::LanguageSupport;
use tree_sitter::Language;

pub struct C;

define_language_queries!("c", "c");

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

    impl_language_queries!("c");

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
