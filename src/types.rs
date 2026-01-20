use compact_str::CompactString;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;
use tower_lsp::lsp_types::{Range, Url};
use tree_sitter::Tree;


#[derive(Debug, Clone)]
pub struct EnvReference {
   
    pub name: CompactString,

   
   
    pub full_range: Range,

   
   
    pub name_range: Range,

   
    pub access_type: AccessType,

   
    pub has_default: bool,

   
    pub default_value: Option<CompactString>,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
   
    Property,
   
    Subscript,
   
    Variable,
   
    Dictionary,
   
    FunctionCall,
   
    Macro,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingKind {
   
    Value,
   
    Object,
}


#[derive(Debug, Clone)]
pub struct EnvBinding {
   
    pub binding_name: CompactString,

   
    pub env_var_name: CompactString,

   
    pub binding_range: Range,

   
    pub declaration_range: Range,

   
    pub scope_range: Range,

   
    pub is_valid: bool,

   
    pub kind: BindingKind,

   
   
    pub destructured_key_range: Option<Range>,
}


#[derive(Debug, Clone)]
pub struct EnvBindingUsage {
   
    pub name: CompactString,
   
    pub range: Range,
   
    pub declaration_range: Range,
   
    pub env_var_name: CompactString,
}


#[derive(Debug, Clone)]
pub struct ImportAlias {
   
    pub module_path: CompactString,

   
    pub original_name: CompactString,

   
   
    pub alias: Option<CompactString>,

   
    pub range: Range,
}


#[derive(Debug, Clone, Default)]
pub struct ImportContext {
   
   
    pub aliases: HashMap<CompactString, (CompactString, CompactString)>,

   
   
    pub imported_modules: HashSet<CompactString>,
}

impl ImportContext {
    pub fn new() -> Self {
        Self::default()
    }

   
    pub fn is_env_related(&self, identifier: &str, known_modules: &[&str]) -> bool {
       
        if known_modules.contains(&identifier) && self.imported_modules.contains(identifier) {
            return true;
        }

       
        if let Some((module, _)) = self.aliases.get(identifier) {
            return known_modules.contains(&module.as_str());
        }

        false
    }
}




#[derive(Debug)]
pub struct DocumentState {
   
    pub uri: Url,

   
    pub content: String,

   
    pub version: i32,

   
    pub language_id: CompactString,

   
    pub tree: Option<Tree>,

   
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

   
    pub fn has_intelligence(&self) -> bool {
        self.tree.is_some()
    }
}


#[derive(Debug)]
pub enum EnvVarHit<'a> {
   
    DirectReference(&'a EnvReference),
   
    ViaBinding(&'a EnvBinding),
}







#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(NonZeroU32);

impl SymbolId {
   
   
    #[inline]
    pub fn new(id: u32) -> Option<Self> {
        NonZeroU32::new(id).map(SymbolId)
    }

   
    #[inline]
    pub fn index(&self) -> usize {
        (self.0.get() - 1) as usize
    }

   
    #[inline]
    pub fn raw(&self) -> u32 {
        self.0.get()
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(NonZeroU32);

impl ScopeId {
   
    #[inline]
    pub fn new(id: u32) -> Option<Self> {
        NonZeroU32::new(id).map(ScopeId)
    }

   
    #[inline]
    pub fn index(&self) -> usize {
        (self.0.get() - 1) as usize
    }

   
    #[inline]
    pub fn raw(&self) -> u32 {
        self.0.get()
    }

   
    #[inline]
    pub fn root() -> Self {
       
        ScopeId(NonZeroU32::new(1).unwrap())
    }
}



#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolOrigin {
   
   
    EnvVar { name: CompactString },

   
   
    EnvObject { canonical_name: CompactString },

   
   
    Symbol { target: SymbolId },

   
   
    DestructuredProperty {
        source: SymbolId,
        key: CompactString,
    },

   
   
    Unknown,

   
   
   
    UnresolvedSymbol { source_name: CompactString },

   
   
   
    UnresolvedDestructure {
        source_name: CompactString,
        key: CompactString,
    },

   
    Unresolvable,
}

impl SymbolOrigin {
   
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


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
   
   
    Value,

   
   
    EnvObject,

   
   
    Variable,

   
   
    DestructuredProperty,
}



#[derive(Debug, Clone)]
pub struct Symbol {
   
    pub id: SymbolId,

   
    pub name: CompactString,

   
    pub declaration_range: Range,

   
    pub name_range: Range,

   
    pub scope: ScopeId,

   
    pub origin: SymbolOrigin,

   
    pub kind: SymbolKind,

   
    pub is_valid: bool,

   
   
    pub destructured_key_range: Option<Range>,
}

impl Symbol {
   
    pub fn is_env_related(&self) -> bool {
        self.origin.is_env_related()
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
   
    Module,
   
    Function,
   
    Block,
   
    Class,
   
    Loop,
   
    Conditional,
}


#[derive(Debug, Clone)]
pub struct Scope {
   
    pub id: ScopeId,

   
    pub parent: Option<ScopeId>,

   
    pub range: Range,

   
    pub kind: ScopeKind,
}

impl Scope {
   
    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }
}


#[derive(Debug, Clone)]
pub struct SymbolUsage {
   
    pub symbol_id: SymbolId,

   
    pub range: Range,

   
    pub scope: ScopeId,

   
   
    pub property_access: Option<CompactString>,

   
   
    pub property_access_range: Option<Range>,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedEnv {
   
    Variable(CompactString),
   
    Object(CompactString),
}

impl ResolvedEnv {
   
    pub fn as_variable(&self) -> Option<&CompactString> {
        match self {
            ResolvedEnv::Variable(name) => Some(name),
            ResolvedEnv::Object(_) => None,
        }
    }

   
    pub fn as_object(&self) -> Option<&CompactString> {
        match self {
            ResolvedEnv::Object(name) => Some(name),
            ResolvedEnv::Variable(_) => None,
        }
    }
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvSourceKind {
   
    Object { canonical_name: CompactString },
   
    Variable { name: CompactString },
}







#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportResolution {
   
   
    EnvVar { name: CompactString },

   
   
    EnvObject { canonical_name: CompactString },

   
   
    ReExport {
       
        source_module: CompactString,
       
        original_name: CompactString,
    },

   
   
   
    LocalChain { symbol_id: SymbolId },

   
    Unknown,
}

impl ExportResolution {
   
    pub fn is_env_related(&self) -> bool {
        !matches!(self, ExportResolution::Unknown)
    }
}



#[derive(Debug, Clone)]
pub struct ModuleExport {
   
    pub exported_name: CompactString,

   
   
    pub local_name: Option<CompactString>,

   
    pub resolution: ExportResolution,

   
    pub declaration_range: Range,

   
    pub is_default: bool,
}

impl ModuleExport {
   
    pub fn local_name_or_exported(&self) -> &CompactString {
        self.local_name.as_ref().unwrap_or(&self.exported_name)
    }
}



#[derive(Debug, Clone, Default)]
pub struct FileExportEntry {
   
    pub named_exports: HashMap<CompactString, ModuleExport>,

   
    pub default_export: Option<ModuleExport>,

   
   
    pub wildcard_reexports: Vec<CompactString>,
}

impl FileExportEntry {
   
    pub fn new() -> Self {
        Self::default()
    }

   
    pub fn is_empty(&self) -> bool {
        self.named_exports.is_empty()
            && self.default_export.is_none()
            && self.wildcard_reexports.is_empty()
    }

   
    pub fn get_export(&self, name: &str) -> Option<&ModuleExport> {
        self.named_exports.get(name)
    }

   
    pub fn env_related_exports(&self) -> impl Iterator<Item = &ModuleExport> {
        self.named_exports
            .values()
            .chain(self.default_export.iter())
            .filter(|e| e.resolution.is_env_related())
    }

   
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



#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportKind {
   
    Named {
       
        imported_name: CompactString,
    },

   
    Default,

   
    Namespace,

   
    CommonJSNamed {
       
        imported_name: CompactString,
    },

   
    CommonJSDefault,
}

impl ImportKind {
   
    pub fn imported_name(&self) -> Option<&CompactString> {
        match self {
            ImportKind::Named { imported_name } | ImportKind::CommonJSNamed { imported_name } => {
                Some(imported_name)
            }
            _ => None,
        }
    }

   
    pub fn is_namespace(&self) -> bool {
        matches!(self, ImportKind::Namespace)
    }

   
    pub fn is_default(&self) -> bool {
        matches!(self, ImportKind::Default | ImportKind::CommonJSDefault)
    }
}



#[derive(Debug, Clone)]
pub struct ModuleImport {
   
    pub module_specifier: CompactString,

   
   
    pub resolved_uri: Option<Url>,

   
    pub kind: ImportKind,

   
    pub local_name: CompactString,

   
    pub range: Range,
}

impl ModuleImport {
   
    pub fn is_resolved(&self) -> bool {
        self.resolved_uri.is_some()
    }

   
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

   

    #[test]
    fn test_module_import_is_resolved() {
        let resolved = ModuleImport {
            module_specifier: "./config".into(),
            resolved_uri: Some(Url::parse("file
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

   

    #[test]
    fn test_import_context_new() {
        let ctx = ImportContext::new();
        assert!(ctx.aliases.is_empty());
        assert!(ctx.imported_modules.is_empty());
    }

    #[test]
    fn test_import_context_is_env_related() {
        let mut ctx = ImportContext::new();

       
        ctx.imported_modules.insert("os".into());
        assert!(ctx.is_env_related("os", &["os", "process"]));
        assert!(!ctx.is_env_related("fs", &["os", "process"]));

       
        ctx.aliases
            .insert("env".into(), ("os".into(), "environ".into()));
        assert!(ctx.is_env_related("env", &["os", "process"]));
        assert!(!ctx.is_env_related("unknown".into(), &["os", "process"]));
    }

   

    #[test]
    fn test_document_state_new() {
        let uri = Url::parse("file
        let state = DocumentState::new(uri.clone(), "typescript".into(), "const x = 1;".into(), 1);

        assert_eq!(state.uri, uri);
        assert_eq!(state.content, "const x = 1;");
        assert_eq!(state.version, 1);
        assert_eq!(state.language_id, "typescript");
        assert!(state.tree.is_none());
    }

    #[test]
    fn test_document_state_has_intelligence() {
        let uri = Url::parse("file
        let state = DocumentState::new(uri, "typescript".into(), "const x = 1;".into(), 1);

       
        assert!(!state.has_intelligence());
    }
}
