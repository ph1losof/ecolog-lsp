use crate::languages::LanguageSupport;
use std::sync::OnceLock;
use tree_sitter::{Language, Query};

pub struct Rust;

static REFERENCE_QUERY: OnceLock<Query> = OnceLock::new();
static BINDING_QUERY: OnceLock<Query> = OnceLock::new();
static IMPORT_QUERY: OnceLock<Query> = OnceLock::new();
static COMPLETION_QUERY: OnceLock<Query> = OnceLock::new();
static REASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static IDENTIFIER_QUERY: OnceLock<Query> = OnceLock::new();

impl LanguageSupport for Rust {
    fn id(&self) -> &'static str {
        "rust"
    }

    fn is_standard_env_object(&self, name: &str) -> bool {
        name == "std::env" || name == "std" || name == "env"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["rust"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn reference_query(&self) -> &Query {
        REFERENCE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/references.scm"),
            )
            .expect("Failed to compile Rust reference query")
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(BINDING_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/bindings.scm"),
            )
            .expect("Failed to compile Rust binding query")
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(IMPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/imports.scm"),
            )
            .expect("Failed to compile Rust import query")
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(COMPLETION_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/completion.scm"),
            )
            .expect("Failed to compile Rust completion query")
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(REASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/reassignments.scm"),
            )
            .expect("Failed to compile Rust reassignment query")
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(IDENTIFIER_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/identifiers.scm"),
            )
            .expect("Failed to compile Rust identifier query")
        }))
    }

    fn known_env_modules(&self) -> &'static [&'static str] {
        &["std::env", "env"]
    }

    fn is_scope_node(&self, node: tree_sitter::Node) -> bool {
        match node.kind() {
            "function_item" | "closure_expression" | "block" | "for_expression"
            | "if_expression" | "loop_expression" | "while_expression" | "match_expression"
            | "impl_item" | "trait_item" | "mod_item" => true,
            _ => false,
        }
    }
}
