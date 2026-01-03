use std::sync::Arc;
use dashmap::DashMap;
use tower_lsp::lsp_types::{Url, TextDocumentContentChangeEvent, Position};
use crate::types::{DocumentState, EnvReference, EnvBinding, ImportContext, EnvBindingUsage, BindingKind};
use crate::languages::{LanguageRegistry, LanguageSupport};
use crate::analysis::{QueryEngine, BindingGraph, AnalysisPipeline};
use crate::analysis::resolver::BindingResolver;
use compact_str::CompactString;
use tree_sitter::Tree;

struct AnalysisResult {
    tree: Option<Tree>,
    import_context: ImportContext,
    binding_graph: BindingGraph,
}

pub struct DocumentManager {
    documents: DashMap<Url, DocumentState>,
    /// Binding graphs for enhanced resolution (parallel to documents).
    binding_graphs: DashMap<Url, BindingGraph>,
    query_engine: Arc<QueryEngine>,
    languages: Arc<LanguageRegistry>,
}

impl DocumentManager {
    pub fn new(query_engine: Arc<QueryEngine>, languages: Arc<LanguageRegistry>) -> Self {
        Self {
            documents: DashMap::new(),
            binding_graphs: DashMap::new(),
            query_engine,
            languages,
        }
    }

    pub async fn open(&self, uri: Url, language_id: String, content: String, version: i32) {
        // Detect language
        let lang_opt = self.languages.get_by_language_id(&language_id)
            .or_else(|| self.languages.get_for_uri(&uri));

        let mut doc = DocumentState::new(uri.clone(), CompactString::from(&language_id), content.clone(), version);

        if let Some(lang) = lang_opt {
            let AnalysisResult {
                tree,
                import_context,
                binding_graph,
            } = self.analyze_content(&content, lang.as_ref()).await;

            doc.tree = tree;
            doc.import_context = import_context;
            self.binding_graphs.insert(uri.clone(), binding_graph);
        } else {
            self.binding_graphs.insert(uri.clone(), BindingGraph::new());
        }

        self.documents.insert(uri, doc);
    }

    pub async fn change(&self, uri: &Url, changes: Vec<TextDocumentContentChangeEvent>, version: i32) {
        // 1. Update content and get snapshot (short lock)
        let (content, language_id) = {
            if let Some(mut doc) = self.documents.get_mut(uri) {
                // Apply changes - assuming FULL sync mode
                for change in changes {
                    if change.range.is_none() {
                        doc.content = change.text;
                    }
                }
                doc.version = version;
                (doc.content.clone(), doc.language_id.clone())
            } else {
                return;
            }
        };

        // 2. Analyze without lock
        let lang_opt = self.languages.get_by_language_id(&language_id)
            .or_else(|| self.languages.get_for_uri(uri));

        if let Some(lang) = lang_opt {
            let AnalysisResult {
                tree,
                import_context,
                binding_graph,
            } = self.analyze_content(&content, lang.as_ref()).await;

            // 3. Apply results if version matches (short lock)
            if let Some(mut doc) = self.documents.get_mut(uri) {
                if doc.version == version {
                    doc.tree = tree;
                    doc.import_context = import_context;
                    self.binding_graphs.insert(uri.clone(), binding_graph);
                }
            }
        }
    }

    async fn analyze_content(&self, content: &str, language: &dyn LanguageSupport) -> AnalysisResult {
        // Step 1: Parse the document
        let tree = self.query_engine.parse(language, content, None).await;

        let Some(tree) = &tree else {
            return AnalysisResult {
                tree: None,
                import_context: ImportContext::default(),
                binding_graph: BindingGraph::new(),
            };
        };

        let source = content.as_bytes();

        // Step 2: Extract imports (needed for reference validation)
        let imports = self.query_engine.extract_imports(language, tree, source).await;

        // Build ImportContext from extracted imports
        let mut import_ctx = ImportContext::new();
        for import in &imports {
            import_ctx.imported_modules.insert(import.module_path.clone());

            if let Some(alias) = &import.alias {
                import_ctx.aliases.insert(
                    alias.clone(),
                    (import.module_path.clone(), import.original_name.clone())
                );
            } else {
                import_ctx.aliases.insert(
                    import.original_name.clone(),
                    (import.module_path.clone(), import.original_name.clone())
                );
            }
        }

        // Step 3: Run the AnalysisPipeline to create the BindingGraph
        // This handles all reference extraction, binding tracking, and chain resolution
        let binding_graph = AnalysisPipeline::analyze(
            &self.query_engine,
            language,
            tree,
            source,
            &import_ctx,
        ).await;

        AnalysisResult {
            tree: Some(tree.clone()),
            import_context: import_ctx,
            binding_graph,
        }
    }

    pub fn get(&self, uri: &Url) -> Option<dashmap::mapref::one::Ref<Url, DocumentState>> {
        self.documents.get(uri)
    }

    /// Get an environment variable reference at the given position (cloned for thread safety).
    /// Uses BindingGraph for resolution.
    pub fn get_env_reference_cloned(&self, uri: &Url, position: Position) -> Option<EnvReference> {
        let graph = self.binding_graphs.get(uri)?;
        let resolver = BindingResolver::new(&graph);
        resolver.get_env_reference_cloned(position)
    }

    /// Get an environment variable binding at the given position (cloned for thread safety).
    /// Uses BindingGraph for resolution.
    pub fn get_env_binding_cloned(&self, uri: &Url, position: Position) -> Option<EnvBinding> {
        let graph = self.binding_graphs.get(uri)?;
        let resolver = BindingResolver::new(&graph);
        resolver.get_env_binding_cloned(position)
    }

    /// Get a usage of an alias binding at the given position.
    /// Uses BindingGraph for resolution.
    pub fn get_binding_usage_cloned(&self, uri: &Url, position: Position) -> Option<EnvBindingUsage> {
        let graph = self.binding_graphs.get(uri)?;
        let resolver = BindingResolver::new(&graph);
        resolver.get_binding_usage_cloned(position)
    }

    /// Get the BindingKind for a usage by looking up its original binding declaration.
    /// Uses BindingGraph for resolution.
    pub fn get_binding_kind_for_usage(&self, uri: &Url, binding_name: &str) -> Option<BindingKind> {
        let graph = self.binding_graphs.get(uri)?;
        let resolver = BindingResolver::new(&graph);
        resolver.get_binding_kind(binding_name)
    }

    pub async fn check_completion(&self, uri: &Url, position: Position) -> bool {
        if let Some(doc) = self.documents.get(uri) {
            if let Some(tree) = &doc.tree {
                if let Some(lang) = self.languages.get_by_language_id(&doc.language_id) {
                    let obj_name_opt = self.query_engine.check_completion_context(
                        lang.as_ref(),
                        tree,
                        doc.content.as_bytes(),
                        position
                    ).await;

                    if let Some(obj_name) = obj_name_opt {
                        if lang.is_standard_env_object(&obj_name) {
                            return true;
                        }

                        // Check if obj_name is an env object alias via BindingGraph
                        if let Some(graph) = self.binding_graphs.get(uri) {
                            let resolver = BindingResolver::new(&graph);
                            if let Some(kind) = resolver.get_binding_kind(&obj_name) {
                                if kind == BindingKind::Object {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }

    pub async fn check_completion_context(&self, uri: &Url, position: Position) -> Option<CompactString> {
        if let Some(doc) = self.documents.get(uri) {
             if let Some(tree) = &doc.tree {
                 if let Some(lang) = self.languages.get_by_language_id(&doc.language_id) {
                     return self.query_engine.check_completion_context(
                         lang.as_ref(),
                         tree,
                         doc.content.as_bytes(),
                         position
                     ).await;
                 }
             }
        }
        None
    }

    // =========================================================================
    // BindingGraph Access
    // =========================================================================

    /// Get a reference to the binding graph for a document.
    pub fn get_binding_graph(&self, uri: &Url) -> Option<dashmap::mapref::one::Ref<Url, BindingGraph>> {
        self.binding_graphs.get(uri)
    }
}
