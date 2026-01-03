use std::sync::OnceLock;
use tree_sitter::{Language, Query};
use crate::languages::LanguageSupport;

pub struct Go;

static REFERENCE_QUERY: OnceLock<Query> = OnceLock::new();
static BINDING_QUERY: OnceLock<Query> = OnceLock::new();
static IMPORT_QUERY: OnceLock<Query> = OnceLock::new();
static COMPLETION_QUERY: OnceLock<Query> = OnceLock::new();
static REASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static IDENTIFIER_QUERY: OnceLock<Query> = OnceLock::new();

impl LanguageSupport for Go {
    fn id(&self) -> &'static str {
        "go"
    }

    fn is_standard_env_object(&self, name: &str) -> bool {
        name == "os"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["go"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["go"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn reference_query(&self) -> &Query {
        REFERENCE_QUERY.get_or_init(|| {
            Query::new(&self.grammar(), include_str!("../../queries/go/references.scm"))
                .expect("Failed to compile Go reference query")
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(BINDING_QUERY.get_or_init(|| {
            Query::new(&self.grammar(), include_str!("../../queries/go/bindings.scm"))
                .expect("Failed to compile Go binding query")
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(IMPORT_QUERY.get_or_init(|| {
            Query::new(&self.grammar(), include_str!("../../queries/go/imports.scm"))
                .expect("Failed to compile Go import query")
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(COMPLETION_QUERY.get_or_init(|| {
            Query::new(&self.grammar(), include_str!("../../queries/go/completion.scm"))
                .expect("Failed to compile Go completion query")
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(REASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(&self.grammar(), include_str!("../../queries/go/reassignments.scm"))
                .expect("Failed to compile Go reassignment query")
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(IDENTIFIER_QUERY.get_or_init(|| {
            Query::new(&self.grammar(), include_str!("../../queries/go/identifiers.scm"))
                .expect("Failed to compile Go identifier query")
        }))
    }

    fn known_env_modules(&self) -> &'static [&'static str] {
        &["os"]
    }

    fn is_scope_node(&self, node: tree_sitter::Node) -> bool {
        match node.kind() {
            "function_declaration" | "method_declaration" | "func_literal" | 
            "block" | "for_statement" | "if_statement" | 
            "switch_statement" | "select_statement" => true,
            _ => false,
        }
    }

    fn extract_var_name(&self, node: tree_sitter::Node, source: &[u8]) -> Option<compact_str::CompactString> {
        use compact_str::CompactString;
        node.utf8_text(source).ok().map(|s| {
            CompactString::from(s.trim_matches(|c| c == '"' || c == '\''))
        })
    }
}

