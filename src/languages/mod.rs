use crate::types::{EnvSourceKind, ScopeKind};
use compact_str::CompactString;
use tree_sitter::{Language, Node, Query};

pub mod go;
pub mod javascript;
pub mod python;
pub mod registry;
pub mod rust;
pub mod typescript;

pub use registry::LanguageRegistry;

/// Defines environment variable detection for a programming language
pub trait LanguageSupport: Send + Sync {
    // ─────────────────────────────────────────────────────────────
    // Identity
    // ─────────────────────────────────────────────────────────────

    /// Unique language identifier (e.g., "javascript", "python")
    fn id(&self) -> &'static str;

    /// File extensions this language handles (without dot)
    fn extensions(&self) -> &'static [&'static str];

    /// LSP language IDs that map to this language
    fn language_ids(&self) -> &'static [&'static str];

    // ─────────────────────────────────────────────────────────────
    // Tree-sitter Integration
    // ─────────────────────────────────────────────────────────────

    /// The tree-sitter Language grammar
    fn grammar(&self) -> Language;

    // ─────────────────────────────────────────────────────────────
    // Queries
    // ─────────────────────────────────────────────────────────────

    /// Query for detecting env var references
    fn reference_query(&self) -> &Query;

    /// Query for detecting variable bindings from env vars
    fn binding_query(&self) -> Option<&Query> {
        None
    }

    /// Query for detecting completion context (e.g. process.env.|)
    fn completion_query(&self) -> Option<&Query> {
        None
    }

    /// Query for detecting reassignments that invalidate bindings
    fn reassignment_query(&self) -> Option<&Query> {
        None
    }

    /// Query for detecting import statements
    fn import_query(&self) -> Option<&Query> {
        None
    }

    /// Query for detecting generic identifiers (for alias usage tracking)
    fn identifier_query(&self) -> Option<&Query> {
        None
    }

    // ─────────────────────────────────────────────────────────────
    // NEW: Enhanced Binding Resolution Queries
    // ─────────────────────────────────────────────────────────────

    /// Query for detecting variable-to-variable assignments (const b = a)
    /// Used for tracking binding chains.
    /// Captures: @assignment_target, @assignment_source
    fn assignment_query(&self) -> Option<&Query> {
        None
    }

    /// Query for detecting destructuring patterns from identifiers
    /// Example: const { VAR } = alias (where alias is an identifier)
    /// Captures: @destructure_target, @destructure_key, @destructure_source
    fn destructure_query(&self) -> Option<&Query> {
        None
    }

    /// Query for detecting scope-creating nodes
    /// Captures: @scope_node
    fn scope_query(&self) -> Option<&Query> {
        None
    }

    // ─────────────────────────────────────────────────────────────
    // Cross-Module Export Detection
    // ─────────────────────────────────────────────────────────────

    /// Query for detecting export statements.
    /// Language-agnostic interface - each language implements its own patterns.
    ///
    /// Expected captures (language-dependent):
    /// - @export_name: The exported identifier name
    /// - @export_value: The value being exported (optional)
    /// - @local_name: The local name if aliased (optional)
    /// - @reexport_source: Module specifier for re-exports (optional)
    /// - @wildcard_source: Module specifier for wildcard re-exports (optional)
    /// - @export_stmt: The entire export statement node
    /// - @default_export: Marks a default export
    fn export_query(&self) -> Option<&Query> {
        None
    }

    // ─────────────────────────────────────────────────────────────
    // Extraction
    // ─────────────────────────────────────────────────────────────

    /// Extract the variable name from a captured node
    fn extract_var_name(&self, node: Node, source: &[u8]) -> Option<CompactString> {
        node.utf8_text(source).ok().map(|s| s.trim().into())
    }

    /// Extract the identifier name from a captured node
    fn extract_identifier(&self, node: Node, source: &[u8]) -> Option<CompactString> {
        node.utf8_text(source).ok().map(|s| s.trim().into())
    }

    /// Extract the key from a destructure pattern node
    /// For patterns like `const { KEY: alias }`, returns "KEY"
    fn extract_destructure_key(&self, node: Node, source: &[u8]) -> Option<CompactString> {
        // Default: same as identifier (for shorthand like `const { KEY }`)
        node.utf8_text(source).ok().map(|s| s.trim().into())
    }

    /// Extract property access info from AST at position.
    /// Returns (object_name, property_name) if position is on a property access.
    ///
    /// This is language-specific because different languages use different AST node types:
    /// - JavaScript/TypeScript: `member_expression` → `property_identifier`
    /// - Python: `attribute` node
    /// - Rust: `field_expression` → `field_identifier`
    /// - Go: `selector_expression`
    ///
    /// Default implementation returns None (not supported).
    fn extract_property_access(
        &self,
        _tree: &tree_sitter::Tree,
        _content: &str,
        _byte_offset: usize,
    ) -> Option<(CompactString, CompactString)> {
        None
    }

    /// Check if a node represents an env source (process.env, os.environ, etc.)
    /// Returns the kind of env source if it is one.
    fn is_env_source_node(&self, _node: Node, _source: &[u8]) -> Option<EnvSourceKind> {
        None
    }

    /// Strip language-specific quote characters from a string literal
    /// Default implementation removes double and single quotes
    fn strip_quotes<'a>(&self, text: &'a str) -> &'a str {
        text.trim_matches(|c| c == '"' || c == '\'')
    }

    // ─────────────────────────────────────────────────────────────
    // Validation
    // ─────────────────────────────────────────────────────────────

    /// Known module paths for this language
    fn known_env_modules(&self) -> &'static [&'static str] {
        &[]
    }

    /// Check if the object name is a standard environment variable object (e.g. process.env)
    fn is_standard_env_object(&self, _name: &str) -> bool {
        false
    }

    /// Get the default environment object name (e.g. "process.env" or "os.environ")
    /// Used when the binding name is an object alias
    fn default_env_object_name(&self) -> Option<&'static str> {
        None
    }

    /// Get the characters that should trigger completion for this language.
    /// These are derived from the language's env access patterns:
    /// - `.` for member access (e.g., `process.env.`)
    /// - `"` and `'` for subscript access (e.g., `process.env["`)
    fn completion_trigger_characters(&self) -> &'static [&'static str] {
        &[]
    }

    /// Check if a node acts as a scope boundary (e.g. function, block)
    fn is_scope_node(&self, _node: Node) -> bool {
        // Default impl: program (whole file) is a scope
        _node.kind() == "program" || _node.kind() == "source_file" || _node.kind() == "module"
    }

    /// Check if a node is the root/document-level node (e.g. program, source_file, module)
    /// These nodes represent the entire document and shouldn't create sub-scopes.
    fn is_root_node(&self, node: Node) -> bool {
        matches!(node.kind(), "program" | "source_file" | "module")
    }

    /// Map a tree-sitter node kind to a ScopeKind.
    /// Override this in language implementations to customize scope classification.
    fn node_to_scope_kind(&self, kind: &str) -> ScopeKind {
        match kind {
            // Functions
            "function_declaration"
            | "arrow_function"
            | "function"
            | "method_definition"
            | "function_definition"
            | "function_item"
            | "func_literal"
            | "closure_expression"
            | "generator_function"
            | "generator_function_declaration" => ScopeKind::Function,

            // Classes
            "class_declaration"
            | "class_definition"
            | "class_body"
            | "impl_item"
            | "trait_item"
            | "class" => ScopeKind::Class,

            // Loops
            "for_statement"
            | "for_expression"
            | "while_statement"
            | "while_expression"
            | "loop_expression"
            | "do_statement"
            | "for_in_statement"
            | "for_of_statement" => ScopeKind::Loop,

            // Conditionals
            "if_statement"
            | "if_expression"
            | "else_clause"
            | "try_statement"
            | "catch_clause"
            | "match_expression"
            | "switch_statement"
            | "switch_case" => ScopeKind::Conditional,

            // Everything else is a block
            _ => ScopeKind::Block,
        }
    }
}
