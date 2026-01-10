use crate::languages::LanguageSupport;
use crate::types::EnvSourceKind;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

pub struct Rust;

static REFERENCE_QUERY: OnceLock<Query> = OnceLock::new();
static BINDING_QUERY: OnceLock<Query> = OnceLock::new();
static IMPORT_QUERY: OnceLock<Query> = OnceLock::new();
static COMPLETION_QUERY: OnceLock<Query> = OnceLock::new();
static REASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static IDENTIFIER_QUERY: OnceLock<Query> = OnceLock::new();
static EXPORT_QUERY: OnceLock<Query> = OnceLock::new();
// Enhanced binding resolution queries
static ASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static SCOPE_QUERY: OnceLock<Query> = OnceLock::new();

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

    fn export_query(&self) -> Option<&Query> {
        Some(EXPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/exports.scm"),
            )
            .expect("Failed to compile Rust export query")
        }))
    }

    // ─────────────────────────────────────────────────────────────
    // Enhanced Binding Resolution Queries
    // ─────────────────────────────────────────────────────────────

    fn assignment_query(&self) -> Option<&Query> {
        Some(ASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/assignments.scm"),
            )
            .expect("Failed to compile Rust assignment query")
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(SCOPE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/scopes.scm"),
            )
            .expect("Failed to compile Rust scope query")
        }))
    }

    fn is_env_source_node(&self, node: Node, source: &[u8]) -> Option<EnvSourceKind> {
        // Check for std::env or env module references
        // Rust env vars are typically accessed via function calls, not object access
        // But we can detect if someone does: let env = std::env; and then uses env.var()
        if node.kind() == "scoped_identifier" {
            let text = node.utf8_text(source).ok()?;
            if text == "std::env" || text == "env" {
                return Some(EnvSourceKind::Object {
                    canonical_name: "std::env".into(),
                });
            }
        }

        if node.kind() == "identifier" {
            let text = node.utf8_text(source).ok()?;
            // Just "env" might be from a use statement
            if text == "env" {
                return Some(EnvSourceKind::Object {
                    canonical_name: "std::env".into(),
                });
            }
        }

        None
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

    fn extract_property_access(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        byte_offset: usize,
    ) -> Option<(compact_str::CompactString, compact_str::CompactString)> {
        let node = tree
            .root_node()
            .descendant_for_byte_range(byte_offset, byte_offset)?;

        // In Rust, we might be on the field_identifier inside a `field_expression`
        // Check if current node or parent is a `field_expression`
        let field_expr = if node.kind() == "field_expression" {
            node
        } else if let Some(parent) = node.parent() {
            if parent.kind() == "field_expression" {
                parent
            } else {
                return None;
            }
        } else {
            return None;
        };

        // Get the value (object) and field from the field_expression
        let value_node = field_expr.child_by_field_name("value")?;
        let field_node = field_expr.child_by_field_name("field")?;

        // We want the value to be a simple identifier
        if value_node.kind() != "identifier" {
            return None;
        }

        let object_name = value_node.utf8_text(content.as_bytes()).ok()?;
        let property_name = field_node.utf8_text(content.as_bytes()).ok()?;

        Some((object_name.into(), property_name.into()))
    }
}
