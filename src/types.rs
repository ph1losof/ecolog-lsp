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
