use compact_str::CompactString;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;
use tower_lsp::lsp_types::{Range, Url};
use tree_sitter::Tree;

/// A detected reference to an environment variable
#[derive(Debug, Clone)]
pub struct EnvReference {
    /// The environment variable name (e.g., "DATABASE_URL")
    pub name: CompactString,

    /// Range of the entire access expression
    /// e.g., `process.env.DATABASE_URL` or `os.getenv("DATABASE_URL")`
    pub full_range: Range,

    /// Range of just the variable name
    /// e.g., `DATABASE_URL` within the full expression
    pub name_range: Range,

    /// The access pattern detected
    pub access_type: AccessType,

    /// Whether a default value is provided
    pub has_default: bool,

    /// The default value if present and extractable
    pub default_value: Option<CompactString>,
}

/// How the environment variable is being accessed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    /// Property access (e.g. process.env.VAR)
    Property,
    /// Subscript access (e.g. process.env["VAR"])
    Subscript,
    /// Direct variable usage (e.g. usage of alias)
    Variable,
    /// Dictionary access (e.g. env["VAR"]) - effectively same as Subscript but distinct context
    Dictionary,
    /// Function call argument (e.g. getenv("VAR"))
    FunctionCall,
    /// Macro invocation: `env!("VAR")`
    Macro,
}

/// Type of the binding
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingKind {
    /// Binding to a specific value (e.g. `const x = process.env.VAL`)
    Value,
    /// Binding to the environment object itself (e.g. `const env = process.env`)
    Object,
}

/// A local variable bound to an environment variable value
#[derive(Debug, Clone)]
pub struct EnvBinding {
    /// The local variable name (e.g., "dbUrl", "port")
    pub binding_name: CompactString,

    /// The environment variable it references
    pub env_var_name: CompactString,

    /// Range of the binding identifier (for hover)
    pub binding_range: Range,

    /// Range of the entire declaration
    pub declaration_range: Range,

    /// Range of the scope where this binding is valid
    pub scope_range: Range,

    /// Whether this binding has been reassigned (invalidated)
    pub is_valid: bool,

    /// The kind of binding (Value or Object)
    pub kind: BindingKind,

    /// For destructured bindings with rename, the range of the original key.
    /// E.g., for `{ API_KEY: apiKey }`, this is the range of `API_KEY`.
    pub destructured_key_range: Option<Range>,
}

/// A usage of a local variable derived from an environment variable
#[derive(Debug, Clone)]
pub struct EnvBindingUsage {
    /// The name of the variable being used
    pub name: CompactString,
    /// Range of the usage
    pub range: Range,
    /// The declaration range of the original binding (to link back)
    pub declaration_range: Range,
    /// The environment variable it refers to
    pub env_var_name: CompactString,
}

/// An import that may alias env-related modules
#[derive(Debug, Clone)]
pub struct ImportAlias {
    /// The module path (e.g., "os", "std::env")
    pub module_path: CompactString,

    /// The original name being imported (e.g., "environ", "var")
    pub original_name: CompactString,

    /// The alias if present (e.g., "env" from `import environ as env`)
    /// If None, original_name is used directly
    pub alias: Option<CompactString>,

    /// Range of the import statement
    pub range: Range,
}

/// Resolved import context for a document
#[derive(Debug, Clone, Default)]
pub struct ImportContext {
    /// Maps alias -> (module_path, original_name)
    /// e.g., "env" -> ("os", "environ")
    pub aliases: HashMap<CompactString, (CompactString, CompactString)>,

    /// Set of directly imported module paths
    /// e.g., {"os", "std::env"}
    pub imported_modules: HashSet<CompactString>,
}

impl ImportContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if an identifier could refer to a known env module/function
    pub fn is_env_related(&self, identifier: &str, known_modules: &[&str]) -> bool {
        // 1. Direct match with known module
        if known_modules.contains(&identifier) && self.imported_modules.contains(identifier) {
            return true;
        }

        // 2. Check if it's an alias to a known module
        if let Some((module, _)) = self.aliases.get(identifier) {
            return known_modules.contains(&module.as_str());
        }

        false
    }
}

/// Intelligence state for a single document.
/// Note: Environment variable references, bindings, and usages are now stored
/// in the BindingGraph (see DocumentManager.binding_graphs).
#[derive(Debug)]
pub struct DocumentState {
    /// Document URI
    pub uri: Url,

    /// Document content (owned for incremental updates)
    pub content: String,

    /// LSP document version
    pub version: i32,

    /// Language identifier
    pub language_id: CompactString,

    /// Parsed syntax tree (None if parse failed)
    pub tree: Option<Tree>,

    /// Import context for alias resolution
    pub import_context: ImportContext,
}

impl DocumentState {
    pub fn new(uri: Url, language_id: CompactString, content: String, version: i32) -> Self {
        Self {
            uri,
            content,
            version,
            language_id,
            tree: None,
            import_context: ImportContext::default(),
        }
    }

    /// Check if document has valid intelligence
    pub fn has_intelligence(&self) -> bool {
        self.tree.is_some()
    }
}

/// Result of looking up an env var at a position
#[derive(Debug)]
pub enum EnvVarHit<'a> {
    /// Direct reference (e.g., hovering on `process.env.DATABASE_URL`)
    DirectReference(&'a EnvReference),
    /// Via binding (e.g., hovering on `dbUrl` which was assigned from env)
    ViaBinding(&'a EnvBinding),
}

// ============================================================================
// NEW: Arena-based Binding Resolution System
// ============================================================================

/// Unique identifier for a symbol within a document's binding graph.
/// Uses NonZeroU32 for niche optimization (Option<SymbolId> is same size as SymbolId).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(NonZeroU32);

impl SymbolId {
    /// Create a new SymbolId from a 1-based index.
    /// Returns None if id is 0.
    #[inline]
    pub fn new(id: u32) -> Option<Self> {
        NonZeroU32::new(id).map(SymbolId)
    }

    /// Get the 0-based index for arena access.
    #[inline]
    pub fn index(&self) -> usize {
        (self.0.get() - 1) as usize
    }

    /// Get the raw u32 value (1-based).
    #[inline]
    pub fn raw(&self) -> u32 {
        self.0.get()
    }
}

/// Unique identifier for a scope within a document's binding graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(NonZeroU32);

impl ScopeId {
    /// Create a new ScopeId from a 1-based index.
    #[inline]
    pub fn new(id: u32) -> Option<Self> {
        NonZeroU32::new(id).map(ScopeId)
    }

    /// Get the 0-based index for arena access.
    #[inline]
    pub fn index(&self) -> usize {
        (self.0.get() - 1) as usize
    }

    /// Get the raw u32 value (1-based).
    #[inline]
    pub fn raw(&self) -> u32 {
        self.0.get()
    }

    /// The root/module scope ID (always 1).
    #[inline]
    pub fn root() -> Self {
        // SAFETY: 1 is always non-zero
        ScopeId(NonZeroU32::new(1).unwrap())
    }
}

/// What a symbol ultimately resolves to in the binding chain.
/// This is the key data structure for tracking env var origins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolOrigin {
    /// Resolves to a specific environment variable.
    /// Example: `const x = process.env.DATABASE_URL` → EnvVar { name: "DATABASE_URL" }
    EnvVar { name: CompactString },

    /// Resolves to the entire environment object.
    /// Example: `const env = process.env` → EnvObject { canonical_name: "process.env" }
    EnvObject { canonical_name: CompactString },

    /// Resolves to another symbol (for chain tracking).
    /// Example: `const b = a` where `a` is already tracked → Symbol { target: a's SymbolId }
    Symbol { target: SymbolId },

    /// Destructured property from another symbol.
    /// Example: `const { DB_URL } = env` → DestructuredProperty { source: env's SymbolId, key: "DB_URL" }
    DestructuredProperty {
        source: SymbolId,
        key: CompactString,
    },

    /// Origin is unknown or not environment-related.
    /// Used as a placeholder before resolution or for non-env symbols.
    Unknown,

    /// Unresolved chain assignment (forward reference).
    /// Example: `const b = a` where `a` is declared later.
    /// Will be resolved to `Symbol { target }` in Phase 4.
    UnresolvedSymbol { source_name: CompactString },

    /// Unresolved destructure from a symbol (forward reference).
    /// Example: `const { X } = obj` where `obj` is declared later.
    /// Will be resolved to `DestructuredProperty` in Phase 4.
    UnresolvedDestructure {
        source_name: CompactString,
        key: CompactString,
    },

    /// Resolution was attempted but failed (cycle detected or depth limit).
    Unresolvable,
}

impl SymbolOrigin {
    /// Check if this origin is env-related (directly or transitively).
    pub fn is_env_related(&self) -> bool {
        matches!(
            self,
            SymbolOrigin::EnvVar { .. }
                | SymbolOrigin::EnvObject { .. }
                | SymbolOrigin::Symbol { .. }
                | SymbolOrigin::DestructuredProperty { .. }
                | SymbolOrigin::UnresolvedSymbol { .. }
                | SymbolOrigin::UnresolvedDestructure { .. }
        )
    }

    /// Check if this is a terminal (non-chain) origin.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            SymbolOrigin::EnvVar { .. }
                | SymbolOrigin::EnvObject { .. }
                | SymbolOrigin::Unknown
                | SymbolOrigin::Unresolvable
        )
    }
}

/// Kind of symbol in the binding graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    /// A binding to a specific env var value.
    /// Example: `const dbUrl = process.env.DATABASE_URL`
    Value,

    /// A binding to the entire env object.
    /// Example: `const env = process.env`
    EnvObject,

    /// A variable that references another symbol.
    /// Example: `const b = a`
    Variable,

    /// A destructured property from an object.
    /// Example: `const { DB_URL } = process.env` or `const { DB_URL: url } = env`
    DestructuredProperty,
}

/// A symbol in the binding graph.
/// Represents a variable declaration that may be env-related.
#[derive(Debug, Clone)]
pub struct Symbol {
    /// Unique ID within this document's binding graph.
    pub id: SymbolId,

    /// The symbol name as it appears in source code.
    pub name: CompactString,

    /// Range of the entire declaration statement.
    pub declaration_range: Range,

    /// Range of just the symbol name/identifier.
    pub name_range: Range,

    /// The scope this symbol is declared in.
    pub scope: ScopeId,

    /// What this symbol resolves to (origin tracking).
    pub origin: SymbolOrigin,

    /// Kind of symbol.
    pub kind: SymbolKind,

    /// Whether this symbol is still valid (false if reassigned).
    pub is_valid: bool,

    /// For destructured bindings, the range of the original property key.
    /// E.g., for `{ API_KEY: apiKey }`, this is the range of `API_KEY`.
    pub destructured_key_range: Option<Range>,
}

impl Symbol {
    /// Check if this symbol is env-related (based on origin).
    pub fn is_env_related(&self) -> bool {
        self.origin.is_env_related()
    }
}

/// Kind of lexical scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// Module/file-level scope (root).
    Module,
    /// Function scope (function, arrow function, method).
    Function,
    /// Block scope (statement block, for/while body).
    Block,
    /// Class scope.
    Class,
    /// Loop scope (for, while, do-while).
    Loop,
    /// Conditional scope (if, else, switch, match).
    Conditional,
}

/// A lexical scope in the binding graph.
#[derive(Debug, Clone)]
pub struct Scope {
    /// Unique ID within this document's binding graph.
    pub id: ScopeId,

    /// Parent scope (None for module/root scope).
    pub parent: Option<ScopeId>,

    /// Range of the scope in source code.
    pub range: Range,

    /// Kind of scope.
    pub kind: ScopeKind,
}

impl Scope {
    /// Check if this is the root/module scope.
    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }
}

/// A usage of a symbol in the binding graph.
#[derive(Debug, Clone)]
pub struct SymbolUsage {
    /// The symbol being used.
    pub symbol_id: SymbolId,

    /// Range of this usage in source code.
    pub range: Range,

    /// The scope where this usage occurs.
    pub scope: ScopeId,

    /// For object usages with property access (e.g., `env.VAR`),
    /// this is the property being accessed ("VAR").
    pub property_access: Option<CompactString>,

    /// For object usages with property access, the range of just the property name.
    /// E.g., for `env.VAR`, this is the range of "VAR".
    pub property_access_range: Option<Range>,
}

/// Result of resolving a symbol to its terminal env var.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedEnv {
    /// Resolves to a specific env var name.
    Variable(CompactString),
    /// Resolves to the env object (for object aliases).
    Object(CompactString),
}

impl ResolvedEnv {
    /// Get the env var name if this is a Variable resolution.
    pub fn as_variable(&self) -> Option<&CompactString> {
        match self {
            ResolvedEnv::Variable(name) => Some(name),
            ResolvedEnv::Object(_) => None,
        }
    }

    /// Get the object name if this is an Object resolution.
    pub fn as_object(&self) -> Option<&CompactString> {
        match self {
            ResolvedEnv::Object(name) => Some(name),
            ResolvedEnv::Variable(_) => None,
        }
    }
}

/// Kind of env source detected in source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvSourceKind {
    /// The env object itself (process.env, os.environ, std::env).
    Object { canonical_name: CompactString },
    /// A specific env variable access.
    Variable { name: CompactString },
}

// ============================================================================
// Cross-Module Import/Export Tracking
// ============================================================================

/// What an export resolves to at module boundary.
/// Language-agnostic representation of an exported binding's resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportResolution {
    /// Directly exports an env var.
    /// Example: `export const dbUrl = process.env.DATABASE_URL`
    EnvVar { name: CompactString },

    /// Exports the env object itself.
    /// Example: `export const env = process.env`
    EnvObject { canonical_name: CompactString },

    /// Re-exports from another module (chain tracking).
    /// Example: `export { dbUrl } from "./config"`
    ReExport {
        /// The module specifier being re-exported from
        source_module: CompactString,
        /// The original name in the source module
        original_name: CompactString,
    },

    /// Exports a symbol that resolves through local binding chain.
    /// The SymbolId can be used to resolve via the file's BindingGraph.
    /// Example: `const x = env.DB; export { x }`
    LocalChain { symbol_id: SymbolId },

    /// Non-env-related export (skip during resolution).
    Unknown,
}

impl ExportResolution {
    /// Check if this export is potentially env-related.
    pub fn is_env_related(&self) -> bool {
        !matches!(self, ExportResolution::Unknown)
    }
}

/// An exported symbol from a module.
/// Language-agnostic representation of a module export.
#[derive(Debug, Clone)]
pub struct ModuleExport {
    /// The exported name (what importers use).
    pub exported_name: CompactString,

    /// Original/local name if different (for `export { local as exported }`).
    /// None if exported_name == local name.
    pub local_name: Option<CompactString>,

    /// What this export resolves to.
    pub resolution: ExportResolution,

    /// Range of the export declaration (for go-to-definition).
    pub declaration_range: Range,

    /// Whether this is a default export.
    pub is_default: bool,
}

impl ModuleExport {
    /// Get the local name (falls back to exported_name if not aliased).
    pub fn local_name_or_exported(&self) -> &CompactString {
        self.local_name.as_ref().unwrap_or(&self.exported_name)
    }
}

/// Per-file export information stored in WorkspaceIndex.
/// Contains all exports from a single module, language-agnostic.
#[derive(Debug, Clone, Default)]
pub struct FileExportEntry {
    /// Named exports: exported_name -> ModuleExport
    pub named_exports: HashMap<CompactString, ModuleExport>,

    /// Default export (if any).
    pub default_export: Option<ModuleExport>,

    /// Re-export all patterns: `export * from "./module"`
    /// Stores the module specifiers.
    pub wildcard_reexports: Vec<CompactString>,
}

impl FileExportEntry {
    /// Create a new empty FileExportEntry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if this file has any exports.
    pub fn is_empty(&self) -> bool {
        self.named_exports.is_empty()
            && self.default_export.is_none()
            && self.wildcard_reexports.is_empty()
    }

    /// Get an export by name (checks named exports).
    pub fn get_export(&self, name: &str) -> Option<&ModuleExport> {
        self.named_exports.get(name)
    }

    /// Get all env-related exports.
    pub fn env_related_exports(&self) -> impl Iterator<Item = &ModuleExport> {
        self.named_exports
            .values()
            .chain(self.default_export.iter())
            .filter(|e| e.resolution.is_env_related())
    }

    /// Get all exported env var names (for reverse indexing).
    pub fn exported_env_vars(&self) -> Vec<CompactString> {
        self.env_related_exports()
            .filter_map(|e| {
                if let ExportResolution::EnvVar { name } = &e.resolution {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Kind of import statement.
/// Language-agnostic categorization of import types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportKind {
    /// Named import: `import { foo } from "./module"` or `from module import foo`
    Named {
        /// The name being imported from the source module
        imported_name: CompactString,
    },

    /// Default import: `import foo from "./module"`
    Default,

    /// Namespace import: `import * as foo from "./module"`
    Namespace,

    /// CommonJS-style named: `const { foo } = require("./module")`
    CommonJSNamed {
        /// The name being imported
        imported_name: CompactString,
    },

    /// CommonJS-style default: `const foo = require("./module")`
    CommonJSDefault,
}

impl ImportKind {
    /// Get the imported name for named imports.
    pub fn imported_name(&self) -> Option<&CompactString> {
        match self {
            ImportKind::Named { imported_name } | ImportKind::CommonJSNamed { imported_name } => {
                Some(imported_name)
            }
            _ => None,
        }
    }

    /// Check if this is a namespace import.
    pub fn is_namespace(&self) -> bool {
        matches!(self, ImportKind::Namespace)
    }

    /// Check if this is a default import.
    pub fn is_default(&self) -> bool {
        matches!(self, ImportKind::Default | ImportKind::CommonJSDefault)
    }
}

/// Import information for cross-module resolution.
/// Represents a single import binding in a file.
#[derive(Debug, Clone)]
pub struct ModuleImport {
    /// The import specifier (e.g., "./config", "../utils/env").
    pub module_specifier: CompactString,

    /// Resolved file URI (after module resolution).
    /// None if resolution failed or hasn't been attempted.
    pub resolved_uri: Option<Url>,

    /// What's being imported.
    pub kind: ImportKind,

    /// Local binding name in the importing file.
    pub local_name: CompactString,

    /// Range of the import statement.
    pub range: Range,
}

impl ModuleImport {
    /// Check if this import has been resolved to a file.
    pub fn is_resolved(&self) -> bool {
        self.resolved_uri.is_some()
    }

    /// Check if this is a relative import (starts with ./ or ../).
    pub fn is_relative(&self) -> bool {
        self.module_specifier.starts_with("./") || self.module_specifier.starts_with("../")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::{Position, Url};

    fn make_range(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Range {
        Range {
            start: Position {
                line: start_line,
                character: start_char,
            },
            end: Position {
                line: end_line,
                character: end_char,
            },
        }
    }

    // ========== SymbolId Tests ==========

    #[test]
    fn test_symbol_id_new() {
        assert!(SymbolId::new(0).is_none());
        assert!(SymbolId::new(1).is_some());
        assert!(SymbolId::new(100).is_some());
    }

    #[test]
    fn test_symbol_id_index() {
        let id = SymbolId::new(1).unwrap();
        assert_eq!(id.index(), 0);

        let id = SymbolId::new(5).unwrap();
        assert_eq!(id.index(), 4);
    }

    #[test]
    fn test_symbol_id_raw() {
        let id = SymbolId::new(42).unwrap();
        assert_eq!(id.raw(), 42);
    }

    // ========== ScopeId Tests ==========

    #[test]
    fn test_scope_id_new() {
        assert!(ScopeId::new(0).is_none());
        assert!(ScopeId::new(1).is_some());
    }

    #[test]
    fn test_scope_id_index() {
        let id = ScopeId::new(1).unwrap();
        assert_eq!(id.index(), 0);

        let id = ScopeId::new(10).unwrap();
        assert_eq!(id.index(), 9);
    }

    #[test]
    fn test_scope_id_raw() {
        let id = ScopeId::new(7).unwrap();
        assert_eq!(id.raw(), 7);
    }

    #[test]
    fn test_scope_id_root() {
        let root = ScopeId::root();
        assert_eq!(root.raw(), 1);
        assert_eq!(root.index(), 0);
    }

    // ========== SymbolOrigin Tests ==========

    #[test]
    fn test_symbol_origin_is_env_related() {
        assert!(SymbolOrigin::EnvVar {
            name: "TEST".into()
        }
        .is_env_related());
        assert!(SymbolOrigin::EnvObject {
            canonical_name: "process.env".into()
        }
        .is_env_related());
        assert!(SymbolOrigin::Symbol {
            target: SymbolId::new(1).unwrap()
        }
        .is_env_related());
        assert!(SymbolOrigin::DestructuredProperty {
            source: SymbolId::new(1).unwrap(),
            key: "KEY".into()
        }
        .is_env_related());
        assert!(SymbolOrigin::UnresolvedSymbol {
            source_name: "x".into()
        }
        .is_env_related());
        assert!(SymbolOrigin::UnresolvedDestructure {
            source_name: "x".into(),
            key: "KEY".into()
        }
        .is_env_related());

        assert!(!SymbolOrigin::Unknown.is_env_related());
        assert!(!SymbolOrigin::Unresolvable.is_env_related());
    }

    #[test]
    fn test_symbol_origin_is_terminal() {
        assert!(SymbolOrigin::EnvVar {
            name: "TEST".into()
        }
        .is_terminal());
        assert!(SymbolOrigin::EnvObject {
            canonical_name: "process.env".into()
        }
        .is_terminal());
        assert!(SymbolOrigin::Unknown.is_terminal());
        assert!(SymbolOrigin::Unresolvable.is_terminal());

        assert!(!SymbolOrigin::Symbol {
            target: SymbolId::new(1).unwrap()
        }
        .is_terminal());
        assert!(!SymbolOrigin::DestructuredProperty {
            source: SymbolId::new(1).unwrap(),
            key: "KEY".into()
        }
        .is_terminal());
    }

    // ========== Symbol Tests ==========

    #[test]
    fn test_symbol_is_env_related() {
        let symbol = Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "test".into(),
            declaration_range: make_range(0, 0, 0, 10),
            name_range: make_range(0, 6, 0, 10),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "TEST".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        };
        assert!(symbol.is_env_related());

        let symbol2 = Symbol {
            origin: SymbolOrigin::Unknown,
            ..symbol
        };
        assert!(!symbol2.is_env_related());
    }

    // ========== Scope Tests ==========

    #[test]
    fn test_scope_is_root() {
        let root_scope = Scope {
            id: ScopeId::root(),
            parent: None,
            range: make_range(0, 0, 100, 0),
            kind: ScopeKind::Module,
        };
        assert!(root_scope.is_root());

        let child_scope = Scope {
            id: ScopeId::new(2).unwrap(),
            parent: Some(ScopeId::root()),
            range: make_range(5, 0, 10, 0),
            kind: ScopeKind::Function,
        };
        assert!(!child_scope.is_root());
    }

    // ========== ResolvedEnv Tests ==========

    #[test]
    fn test_resolved_env_as_variable() {
        let var = ResolvedEnv::Variable("DATABASE_URL".into());
        assert_eq!(var.as_variable(), Some(&"DATABASE_URL".into()));
        assert_eq!(var.as_object(), None);
    }

    #[test]
    fn test_resolved_env_as_object() {
        let obj = ResolvedEnv::Object("process.env".into());
        assert_eq!(obj.as_object(), Some(&"process.env".into()));
        assert_eq!(obj.as_variable(), None);
    }

    // ========== ExportResolution Tests ==========

    #[test]
    fn test_export_resolution_is_env_related() {
        assert!(ExportResolution::EnvVar {
            name: "TEST".into()
        }
        .is_env_related());
        assert!(ExportResolution::EnvObject {
            canonical_name: "process.env".into()
        }
        .is_env_related());
        assert!(ExportResolution::ReExport {
            source_module: "./config".into(),
            original_name: "dbUrl".into()
        }
        .is_env_related());
        assert!(ExportResolution::LocalChain {
            symbol_id: SymbolId::new(1).unwrap()
        }
        .is_env_related());

        assert!(!ExportResolution::Unknown.is_env_related());
    }

    // ========== ModuleExport Tests ==========

    #[test]
    fn test_module_export_local_name_or_exported() {
        let export_no_alias = ModuleExport {
            exported_name: "dbUrl".into(),
            local_name: None,
            resolution: ExportResolution::EnvVar {
                name: "DATABASE_URL".into(),
            },
            declaration_range: make_range(0, 0, 0, 50),
            is_default: false,
        };
        assert_eq!(
            export_no_alias.local_name_or_exported(),
            &CompactString::from("dbUrl")
        );

        let export_with_alias = ModuleExport {
            exported_name: "publicName".into(),
            local_name: Some("internalName".into()),
            resolution: ExportResolution::EnvVar {
                name: "SECRET".into(),
            },
            declaration_range: make_range(0, 0, 0, 50),
            is_default: false,
        };
        assert_eq!(
            export_with_alias.local_name_or_exported(),
            &CompactString::from("internalName")
        );
    }

    // ========== FileExportEntry Tests ==========

    #[test]
    fn test_file_export_entry_new_and_is_empty() {
        let entry = FileExportEntry::new();
        assert!(entry.is_empty());
    }

    #[test]
    fn test_file_export_entry_get_export() {
        let mut entry = FileExportEntry::new();
        let export = ModuleExport {
            exported_name: "test".into(),
            local_name: None,
            resolution: ExportResolution::EnvVar {
                name: "TEST".into(),
            },
            declaration_range: make_range(0, 0, 0, 20),
            is_default: false,
        };
        entry.named_exports.insert("test".into(), export.clone());

        assert!(entry.get_export("test").is_some());
        assert!(entry.get_export("nonexistent").is_none());
        assert!(!entry.is_empty());
    }

    #[test]
    fn test_file_export_entry_env_related_exports() {
        let mut entry = FileExportEntry::new();

        entry.named_exports.insert(
            "envVar".into(),
            ModuleExport {
                exported_name: "envVar".into(),
                local_name: None,
                resolution: ExportResolution::EnvVar {
                    name: "VAR".into(),
                },
                declaration_range: make_range(0, 0, 0, 20),
                is_default: false,
            },
        );

        entry.named_exports.insert(
            "unknown".into(),
            ModuleExport {
                exported_name: "unknown".into(),
                local_name: None,
                resolution: ExportResolution::Unknown,
                declaration_range: make_range(1, 0, 1, 20),
                is_default: false,
            },
        );

        let env_exports: Vec<_> = entry.env_related_exports().collect();
        assert_eq!(env_exports.len(), 1);
    }

    #[test]
    fn test_file_export_entry_exported_env_vars() {
        let mut entry = FileExportEntry::new();

        entry.named_exports.insert(
            "envVar".into(),
            ModuleExport {
                exported_name: "envVar".into(),
                local_name: None,
                resolution: ExportResolution::EnvVar {
                    name: "DATABASE_URL".into(),
                },
                declaration_range: make_range(0, 0, 0, 20),
                is_default: false,
            },
        );

        entry.named_exports.insert(
            "envObj".into(),
            ModuleExport {
                exported_name: "envObj".into(),
                local_name: None,
                resolution: ExportResolution::EnvObject {
                    canonical_name: "process.env".into(),
                },
                declaration_range: make_range(1, 0, 1, 20),
                is_default: false,
            },
        );

        let vars = entry.exported_env_vars();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0], CompactString::from("DATABASE_URL"));
    }

    // ========== ImportKind Tests ==========

    #[test]
    fn test_import_kind_imported_name() {
        let named = ImportKind::Named {
            imported_name: "foo".into(),
        };
        assert_eq!(named.imported_name(), Some(&"foo".into()));

        let cjs_named = ImportKind::CommonJSNamed {
            imported_name: "bar".into(),
        };
        assert_eq!(cjs_named.imported_name(), Some(&"bar".into()));

        let default = ImportKind::Default;
        assert_eq!(default.imported_name(), None);

        let namespace = ImportKind::Namespace;
        assert_eq!(namespace.imported_name(), None);
    }

    #[test]
    fn test_import_kind_is_namespace() {
        assert!(ImportKind::Namespace.is_namespace());
        assert!(!ImportKind::Default.is_namespace());
        assert!(!ImportKind::Named {
            imported_name: "x".into()
        }
        .is_namespace());
    }

    #[test]
    fn test_import_kind_is_default() {
        assert!(ImportKind::Default.is_default());
        assert!(ImportKind::CommonJSDefault.is_default());
        assert!(!ImportKind::Namespace.is_default());
        assert!(!ImportKind::Named {
            imported_name: "x".into()
        }
        .is_default());
    }

    // ========== ModuleImport Tests ==========

    #[test]
    fn test_module_import_is_resolved() {
        let resolved = ModuleImport {
            module_specifier: "./config".into(),
            resolved_uri: Some(Url::parse("file:///project/config.ts").unwrap()),
            kind: ImportKind::Default,
            local_name: "config".into(),
            range: make_range(0, 0, 0, 30),
        };
        assert!(resolved.is_resolved());

        let unresolved = ModuleImport {
            module_specifier: "./config".into(),
            resolved_uri: None,
            kind: ImportKind::Default,
            local_name: "config".into(),
            range: make_range(0, 0, 0, 30),
        };
        assert!(!unresolved.is_resolved());
    }

    #[test]
    fn test_module_import_is_relative() {
        let relative1 = ModuleImport {
            module_specifier: "./config".into(),
            resolved_uri: None,
            kind: ImportKind::Default,
            local_name: "config".into(),
            range: make_range(0, 0, 0, 30),
        };
        assert!(relative1.is_relative());

        let relative2 = ModuleImport {
            module_specifier: "../utils/env".into(),
            resolved_uri: None,
            kind: ImportKind::Default,
            local_name: "env".into(),
            range: make_range(0, 0, 0, 30),
        };
        assert!(relative2.is_relative());

        let absolute = ModuleImport {
            module_specifier: "lodash".into(),
            resolved_uri: None,
            kind: ImportKind::Default,
            local_name: "lodash".into(),
            range: make_range(0, 0, 0, 30),
        };
        assert!(!absolute.is_relative());
    }

    // ========== ImportContext Tests ==========

    #[test]
    fn test_import_context_new() {
        let ctx = ImportContext::new();
        assert!(ctx.aliases.is_empty());
        assert!(ctx.imported_modules.is_empty());
    }

    #[test]
    fn test_import_context_is_env_related() {
        let mut ctx = ImportContext::new();

        // Direct module import
        ctx.imported_modules.insert("os".into());
        assert!(ctx.is_env_related("os", &["os", "process"]));
        assert!(!ctx.is_env_related("fs", &["os", "process"]));

        // With alias
        ctx.aliases
            .insert("env".into(), ("os".into(), "environ".into()));
        assert!(ctx.is_env_related("env", &["os", "process"]));
        assert!(!ctx.is_env_related("unknown".into(), &["os", "process"]));
    }

    // ========== DocumentState Tests ==========

    #[test]
    fn test_document_state_new() {
        let uri = Url::parse("file:///test.ts").unwrap();
        let state = DocumentState::new(uri.clone(), "typescript".into(), "const x = 1;".into(), 1);

        assert_eq!(state.uri, uri);
        assert_eq!(state.content, "const x = 1;");
        assert_eq!(state.version, 1);
        assert_eq!(state.language_id, "typescript");
        assert!(state.tree.is_none());
    }

    #[test]
    fn test_document_state_has_intelligence() {
        let uri = Url::parse("file:///test.ts").unwrap();
        let state = DocumentState::new(uri, "typescript".into(), "const x = 1;".into(), 1);

        // No tree = no intelligence
        assert!(!state.has_intelligence());
    }
}
