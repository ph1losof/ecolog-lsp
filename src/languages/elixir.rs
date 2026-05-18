use crate::languages::LanguageSupport;
use tree_sitter::Language;

pub struct Elixir;

define_language_queries!("elixir", "elixir");

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

    impl_language_queries!("elixir");

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
