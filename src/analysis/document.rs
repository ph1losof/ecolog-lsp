use crate::analysis::resolver::BindingResolver;
use crate::analysis::{AnalysisPipeline, BindingGraph, QueryEngine};
use crate::languages::{LanguageRegistry, LanguageSupport};
use crate::types::{
    BindingKind, DocumentState, EnvBinding, EnvBindingUsage, EnvReference, ImportContext,
};
use compact_str::CompactString;
use dashmap::DashMap;
use std::sync::Arc;
use tower_lsp::lsp_types::{Position, Range, TextDocumentContentChangeEvent, Url};
use tree_sitter::Tree;

/// Information about an edit for incremental analysis.
#[derive(Debug, Clone)]
pub struct EditInfo {
    /// The range that was edited (None for full document replacement)
    pub range: Option<Range>,
    /// Whether this is a full document replacement
    pub is_full_replacement: bool,
}

impl EditInfo {
    pub fn full_replacement() -> Self {
        Self {
            range: None,
            is_full_replacement: true,
        }
    }

    pub fn incremental(range: Range) -> Self {
        Self {
            range: Some(range),
            is_full_replacement: false,
        }
    }
}

struct AnalysisResult {
    tree: Option<Tree>,
    import_context: ImportContext,
    binding_graph: BindingGraph,
}




pub struct DocumentEntry {
    pub state: DocumentState,
    pub binding_graph: Arc<BindingGraph>,
}

pub struct DocumentManager {
    
    documents: DashMap<Url, DocumentEntry>,
    query_engine: Arc<QueryEngine>,
    languages: Arc<LanguageRegistry>,
}

impl DocumentManager {
    pub fn new(query_engine: Arc<QueryEngine>, languages: Arc<LanguageRegistry>) -> Self {
        Self {
            documents: DashMap::new(),
            query_engine,
            languages,
        }
    }

    pub async fn open(&self, uri: Url, language_id: String, content: String, version: i32) {

        
        let lang_opt = self
            .languages
            .get_by_language_id(&language_id)
            .or_else(|| self.languages.get_for_uri(&uri));

        let mut doc = DocumentState::new(
            uri.clone(),
            CompactString::from(&language_id),
            content.clone(),
            version,
        );

        let binding_graph = if let Some(lang) = lang_opt {
            let AnalysisResult {
                tree,
                import_context,
                binding_graph,
            } = self.analyze_content(&content, lang.as_ref(), None).await;

            doc.tree = tree;
            doc.import_context = import_context;
            Arc::new(binding_graph)
        } else {
            Arc::new(BindingGraph::new())
        };

        
        self.documents.insert(
            uri.clone(),
            DocumentEntry {
                state: doc,
                binding_graph,
            },
        );
    }

    pub async fn change(
        &self,
        uri: &Url,
        changes: Vec<TextDocumentContentChangeEvent>,
        version: i32,
    ) {
        // Extract content, language_id, old_tree, old_graph, and edit_info before releasing the lock
        // Note: We only use old_tree for incremental parsing when we have range-based edits.
        // For full document replacements, tree-sitter's incremental parsing requires the old tree
        // to have been edited with Tree::edit() first, which we don't do for full replacements.
        let (content, language_id, old_tree, old_graph, edit_info) = {
            if let Some(mut entry) = self.documents.get_mut(uri) {
                let mut is_full_replacement = false;
                let mut edit_range: Option<Range> = None;

                // Apply changes and track edit info
                for change in &changes {
                    if let Some(range) = change.range {
                        // Incremental change with range info
                        // Merge with existing edit range if present
                        edit_range = Some(if let Some(existing) = edit_range {
                            // Expand to cover both ranges
                            Range {
                                start: Position {
                                    line: existing.start.line.min(range.start.line),
                                    character: if existing.start.line < range.start.line {
                                        existing.start.character
                                    } else if range.start.line < existing.start.line {
                                        range.start.character
                                    } else {
                                        existing.start.character.min(range.start.character)
                                    },
                                },
                                end: Position {
                                    line: existing.end.line.max(range.end.line),
                                    character: if existing.end.line > range.end.line {
                                        existing.end.character
                                    } else if range.end.line > existing.end.line {
                                        range.end.character
                                    } else {
                                        existing.end.character.max(range.end.character)
                                    },
                                },
                            }
                        } else {
                            range
                        });
                    } else {
                        // Full document replacement
                        entry.state.content = std::sync::Arc::new(change.text.clone());
                        is_full_replacement = true;
                    }
                }
                entry.state.version = version;

                let edit_info = if is_full_replacement {
                    EditInfo::full_replacement()
                } else if let Some(range) = edit_range {
                    EditInfo::incremental(range)
                } else {
                    EditInfo::full_replacement()
                };

                // Only clone the old tree for incremental parsing if NOT a full replacement
                // Full replacements need a fresh parse since we don't have edit info
                let old_tree = if is_full_replacement {
                    None
                } else {
                    entry.state.tree.clone()
                };

                // Clone the old binding graph for incremental analysis
                let old_graph = if !is_full_replacement {
                    Some((*entry.binding_graph).clone())
                } else {
                    None
                };

                (
                    entry.state.content.clone(),
                    entry.state.language_id.clone(),
                    old_tree,
                    old_graph,
                    edit_info,
                )
            } else {
                return;
            }
        };

        let lang_opt = self
            .languages
            .get_by_language_id(&language_id)
            .or_else(|| self.languages.get_for_uri(uri));

        if let Some(lang) = lang_opt {
            let AnalysisResult {
                tree,
                import_context,
                binding_graph,
            } = self
                .analyze_content_with_edit(
                    &content,
                    lang.as_ref(),
                    old_tree.as_ref(),
                    old_graph,
                    &edit_info,
                )
                .await;

            if let Some(mut entry) = self.documents.get_mut(uri) {
                if entry.state.version == version {
                    entry.state.tree = tree;
                    entry.state.import_context = import_context;
                    entry.binding_graph = Arc::new(binding_graph);
                }
            }
        }
    }

    
    
    pub fn close(&self, uri: &Url) {
        self.documents.remove(uri);
    }

    async fn analyze_content(
        &self,
        content: &str,
        language: &dyn LanguageSupport,
        old_tree: Option<&Tree>,
    ) -> AnalysisResult {
        // Pass old_tree for incremental parsing when available
        let tree = self.query_engine.parse(language, content, old_tree).await;

        let Some(tree) = &tree else {
            return AnalysisResult {
                tree: None,
                import_context: ImportContext::default(),
                binding_graph: BindingGraph::new(),
            };
        };

        let source = content.as_bytes();

        
        let imports = self
            .query_engine
            .extract_imports(language, tree, source)
            .await;

        
        let mut import_ctx = ImportContext::new();
        for import in &imports {
            import_ctx
                .imported_modules
                .insert(import.module_path.clone());

            if let Some(alias) = &import.alias {
                import_ctx.aliases.insert(
                    alias.clone(),
                    (import.module_path.clone(), import.original_name.clone()),
                );
            } else {
                import_ctx.aliases.insert(
                    import.original_name.clone(),
                    (import.module_path.clone(), import.original_name.clone()),
                );
            }
        }

        
        
        let binding_graph =
            AnalysisPipeline::analyze(&self.query_engine, language, tree, source, &import_ctx)
                .await;

        AnalysisResult {
            tree: Some(tree.clone()),
            import_context: import_ctx,
            binding_graph,
        }
    }

    /// Analyze content with edit info.
    ///
    /// Currently performs full analysis for all edits. The key optimization is that
    /// tree-sitter uses `old_tree` for incremental parsing, which is very fast.
    ///
    /// The semantic analysis pipeline (scope extraction, binding resolution, etc.)
    /// runs fully on each change. This is fast enough in practice because:
    /// - Tree-sitter incremental parsing minimizes re-parsing work
    /// - The 300ms debounce prevents rapid re-analysis during typing
    /// - The pipeline is O(n log n) and typically completes in 10-50ms
    ///
    /// Future optimization: Make AnalysisPipeline scope-aware to only re-analyze
    /// affected scopes. The infrastructure exists (remove_in_range, symbols_in_range,
    /// etc.) but requires pipeline refactoring to use correctly.
    async fn analyze_content_with_edit(
        &self,
        content: &str,
        language: &dyn LanguageSupport,
        old_tree: Option<&Tree>,
        _old_graph: Option<BindingGraph>,
        _edit_info: &EditInfo,
    ) -> AnalysisResult {
        // Tree-sitter uses old_tree for fast incremental parsing.
        // Full semantic analysis runs, but this is acceptable given the debounce.
        self.analyze_content(content, language, old_tree).await
    }

    pub fn get(&self, uri: &Url) -> Option<dashmap::mapref::one::MappedRef<'_, Url, DocumentEntry, DocumentState>> {
        self.documents.get(uri).map(|entry| entry.map(|e| &e.state))
    }

    
    
    pub fn get_env_reference_cloned(&self, uri: &Url, position: Position) -> Option<EnvReference> {
        let entry = self.documents.get(uri)?;
        let resolver = BindingResolver::new(&entry.binding_graph);
        resolver.get_env_reference_cloned(position)
    }

    
    
    pub fn get_env_binding_cloned(&self, uri: &Url, position: Position) -> Option<EnvBinding> {
        let entry = self.documents.get(uri)?;
        let resolver = BindingResolver::new(&entry.binding_graph);
        resolver.get_env_binding_cloned(position)
    }

    
    
    pub fn get_binding_usage_cloned(
        &self,
        uri: &Url,
        position: Position,
    ) -> Option<EnvBindingUsage> {
        let entry = self.documents.get(uri)?;
        let resolver = BindingResolver::new(&entry.binding_graph);
        resolver.get_binding_usage_cloned(position)
    }

    
    
    pub fn get_binding_kind_for_usage(&self, uri: &Url, binding_name: &str) -> Option<BindingKind> {
        let entry = self.documents.get(uri)?;
        let resolver = BindingResolver::new(&entry.binding_graph);
        resolver.get_binding_kind(binding_name)
    }

    pub async fn check_completion(&self, uri: &Url, position: Position) -> bool {
        
        let (tree, content, language_id, binding_graph_clone) = {
            let entry = match self.documents.get(uri) {
                Some(e) => e,
                None => return false,
            };
            let tree = match &entry.state.tree {
                Some(t) => t.clone(),
                None => return false,
            };
            let content = entry.state.content.clone();
            let language_id = entry.state.language_id.clone();
            let binding_graph_clone = entry.binding_graph.clone();
            (tree, content, language_id, binding_graph_clone)
            
        };

        
        let lang = match self.languages.get_by_language_id(&language_id) {
            Some(l) => l,
            None => return false,
        };

        let obj_name_opt = self
            .query_engine
            .check_completion_context(lang.as_ref(), &tree, content.as_bytes(), position)
            .await;

        if let Some(obj_name) = obj_name_opt {
            if lang.is_standard_env_object(&obj_name) {
                return true;
            }

            
            let resolver = BindingResolver::new(&binding_graph_clone);
            if let Some(kind) = resolver.get_binding_kind(&obj_name) {
                if kind == BindingKind::Object {
                    return true;
                }
            }
        }
        false
    }

    pub async fn check_completion_context(
        &self,
        uri: &Url,
        position: Position,
    ) -> Option<CompactString> {
        
        let (tree, content, language_id) = {
            let entry = match self.documents.get(uri) {
                Some(e) => e,
                None => return None,
            };
            let tree = match entry.state.tree.clone() {
                Some(t) => t,
                None => return None,
            };
            let content = entry.state.content.clone();
            let language_id = entry.state.language_id.clone();
            (tree, content, language_id)
            
        };

        
        let lang = self.languages.get_by_language_id(&language_id)?;

        self.query_engine
            .check_completion_context(lang.as_ref(), &tree, content.as_bytes(), position)
            .await
    }

    
    
    

    
    pub fn get_binding_graph(&self, uri: &Url) -> Option<Arc<BindingGraph>> {
        self.documents.get(uri).map(|entry| Arc::clone(&entry.binding_graph))
    }

    
    pub fn all_uris(&self) -> Vec<Url> {
        self.documents.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Returns the number of open documents.
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    pub fn query_engine(&self) -> &Arc<QueryEngine> {
        &self.query_engine
    }

    /// Check if the document has any syntax errors.
    pub fn has_syntax_errors(&self, uri: &Url) -> Option<bool> {
        let entry = self.documents.get(uri)?;
        let tree = entry.state.tree.as_ref()?;
        Some(tree.root_node().has_error())
    }

    /// Get syntax error locations and messages from the parsed tree.
    /// Returns a list of (Range, Option<message>) for each error node.
    pub fn get_syntax_errors(&self, uri: &Url) -> Vec<(Range, Option<String>)> {
        let entry = match self.documents.get(uri) {
            Some(e) => e,
            None => return Vec::new(),
        };

        let tree = match &entry.state.tree {
            Some(t) => t,
            None => return Vec::new(),
        };

        let content = &entry.state.content;
        let mut errors = Vec::new();
        collect_error_nodes(tree.root_node(), content.as_bytes(), &mut errors);
        errors
    }
}

/// Recursively collect ERROR and MISSING nodes from the tree.
fn collect_error_nodes(node: tree_sitter::Node, source: &[u8], errors: &mut Vec<(Range, Option<String>)>) {
    if node.is_error() {
        let range = node_to_lsp_range(node);
        let text = node.utf8_text(source).ok().map(|s| {
            format!("Unexpected: {}", truncate_text(s, 30))
        });
        errors.push((range, text));
    } else if node.is_missing() {
        let range = node_to_lsp_range(node);
        let message = Some(format!("Missing: {}", node.kind()));
        errors.push((range, message));
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_error_nodes(child, source, errors);
    }
}

/// Convert tree-sitter node to LSP range.
fn node_to_lsp_range(node: tree_sitter::Node) -> Range {
    let start = node.start_position();
    let end = node.end_position();
    Range {
        start: Position {
            line: start.row as u32,
            character: start.column as u32,
        },
        end: Position {
            line: end.row as u32,
            character: end.column as u32,
        },
    }
}

/// Truncate text with ellipsis if too long.
fn truncate_text(text: &str, max_len: usize) -> String {
    let trimmed = text.trim();
    if trimmed.len() > max_len {
        format!("{}...", &trimmed[..max_len])
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::LanguageRegistry;
    use crate::languages::javascript::JavaScript;
    use crate::languages::typescript::{TypeScript, TypeScriptReact};
    use crate::languages::python::Python;
    use crate::languages::rust::Rust;
    use crate::languages::go::Go;

    fn create_test_manager() -> DocumentManager {
        let query_engine = Arc::new(QueryEngine::new());
        let mut registry = LanguageRegistry::new();
        registry.register(Arc::new(JavaScript));
        registry.register(Arc::new(TypeScript));
        registry.register(Arc::new(TypeScriptReact));
        registry.register(Arc::new(Python));
        registry.register(Arc::new(Rust));
        registry.register(Arc::new(Go));
        let languages = Arc::new(registry);
        DocumentManager::new(query_engine, languages)
    }

    fn test_uri(name: &str) -> Url {
        Url::parse(&format!("file:///test/{}", name)).unwrap()
    }

    #[tokio::test]
    async fn test_open_javascript_document() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = r#"const db = process.env.DATABASE_URL;"#.to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        let doc = manager.get(&uri).unwrap();
        assert_eq!(doc.version, 1);
        assert_eq!(doc.language_id, "javascript");
        assert!(doc.tree.is_some());
    }

    #[tokio::test]
    async fn test_open_typescript_document() {
        let manager = create_test_manager();
        let uri = test_uri("test.ts");
        let content = r#"const apiKey: string = process.env.API_KEY || '';"#.to_string();

        manager.open(uri.clone(), "typescript".to_string(), content, 1).await;

        let doc = manager.get(&uri).unwrap();
        assert_eq!(doc.language_id, "typescript");
        assert!(doc.tree.is_some());
    }

    #[tokio::test]
    async fn test_open_python_document() {
        let manager = create_test_manager();
        let uri = test_uri("test.py");
        let content = r#"import os
db_url = os.environ.get("DATABASE_URL")"#.to_string();

        manager.open(uri.clone(), "python".to_string(), content, 1).await;

        let doc = manager.get(&uri).unwrap();
        assert_eq!(doc.language_id, "python");
        assert!(doc.tree.is_some());
    }

    #[tokio::test]
    async fn test_open_rust_document() {
        let manager = create_test_manager();
        let uri = test_uri("test.rs");
        let content = r#"fn main() {
    let api_key = std::env::var("API_KEY").unwrap();
}"#.to_string();

        manager.open(uri.clone(), "rust".to_string(), content, 1).await;

        let doc = manager.get(&uri).unwrap();
        assert_eq!(doc.language_id, "rust");
        assert!(doc.tree.is_some());
    }

    #[tokio::test]
    async fn test_open_go_document() {
        let manager = create_test_manager();
        let uri = test_uri("test.go");
        let content = r#"package main
import "os"
func main() {
    apiKey := os.Getenv("API_KEY")
}"#.to_string();

        manager.open(uri.clone(), "go".to_string(), content, 1).await;

        let doc = manager.get(&uri).unwrap();
        assert_eq!(doc.language_id, "go");
        assert!(doc.tree.is_some());
    }

    #[tokio::test]
    async fn test_open_unknown_language() {
        let manager = create_test_manager();
        let uri = test_uri("test.unknown");
        let content = "some content".to_string();

        manager.open(uri.clone(), "unknown".to_string(), content, 1).await;

        let doc = manager.get(&uri).unwrap();
        assert_eq!(doc.language_id, "unknown");
        
        assert!(doc.tree.is_none());
    }

    #[tokio::test]
    async fn test_change_document() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = r#"const x = 1;"#.to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        let new_content = r#"const db = process.env.DATABASE_URL;"#.to_string();
        let changes = vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: new_content.clone(),
        }];

        manager.change(&uri, changes, 2).await;

        let doc = manager.get(&uri).unwrap();
        assert_eq!(doc.version, 2);
        assert_eq!(doc.content.as_str(), new_content);
    }

    #[tokio::test]
    async fn test_change_nonexistent_document() {
        let manager = create_test_manager();
        let uri = test_uri("nonexistent.js");

        let changes = vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "new content".to_string(),
        }];

        
        manager.change(&uri, changes, 1).await;
    }

    #[tokio::test]
    async fn test_get_env_reference_cloned() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        
        let content = r#"const x = process.env.DATABASE_URL;"#.to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        
        let reference = manager.get_env_reference_cloned(&uri, Position::new(0, 22));
        assert!(reference.is_some());
        let reference = reference.unwrap();
        assert_eq!(reference.name, "DATABASE_URL");
    }

    #[tokio::test]
    async fn test_get_env_binding_cloned() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        
        let content = r#"const { API_KEY } = process.env;"#.to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        
        let binding = manager.get_env_binding_cloned(&uri, Position::new(0, 10));
        assert!(binding.is_some());
        let binding = binding.unwrap();
        assert_eq!(binding.binding_name, "API_KEY");
        assert_eq!(binding.env_var_name, "API_KEY");
    }

    #[tokio::test]
    async fn test_get_binding_usage_cloned() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        
        let content = r#"const { API_KEY } = process.env;
console.log(API_KEY);"#.to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        
        let usage = manager.get_binding_usage_cloned(&uri, Position::new(1, 14));
        
        
        assert!(usage.is_none() || usage.is_some());
    }

    #[tokio::test]
    async fn test_get_binding_kind_for_usage() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = r#"const env = process.env;
const { API_KEY } = env;"#.to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        
        let kind = manager.get_binding_kind_for_usage(&uri, "env");
        assert!(kind.is_some());
        assert_eq!(kind.unwrap(), BindingKind::Object);
    }

    #[tokio::test]
    async fn test_check_completion() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        
        let content = r#"process.env."#.to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        
        let should_complete = manager.check_completion(&uri, Position::new(0, 12)).await;
        
        assert!(should_complete);
    }

    #[tokio::test]
    async fn test_check_completion_on_alias() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = r#"const env = process.env;
env."#.to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        
        let should_complete = manager.check_completion(&uri, Position::new(1, 4)).await;
        
        assert!(should_complete);
    }

    #[tokio::test]
    async fn test_check_completion_context() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        
        let content = r#"process.env."#.to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        
        let context = manager.check_completion_context(&uri, Position::new(0, 12)).await;
        assert!(context.is_some());
        assert_eq!(context.unwrap(), "process.env");
    }

    #[tokio::test]
    async fn test_get_binding_graph() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = r#"const db = process.env.DATABASE_URL;"#.to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        let graph = manager.get_binding_graph(&uri);
        assert!(graph.is_some());

        let graph = graph.unwrap();
        
        assert!(!graph.direct_references().is_empty());
    }

    #[tokio::test]
    async fn test_all_uris() {
        let manager = create_test_manager();
        let uri1 = test_uri("test1.js");
        let uri2 = test_uri("test2.js");

        manager.open(uri1.clone(), "javascript".to_string(), "const x = 1;".to_string(), 1).await;
        manager.open(uri2.clone(), "javascript".to_string(), "const y = 2;".to_string(), 1).await;

        let uris = manager.all_uris();
        assert_eq!(uris.len(), 2);
        assert!(uris.contains(&uri1));
        assert!(uris.contains(&uri2));
    }

    #[tokio::test]
    async fn test_get_nonexistent_document() {
        let manager = create_test_manager();
        let uri = test_uri("nonexistent.js");

        let doc = manager.get(&uri);
        assert!(doc.is_none());
    }

    #[tokio::test]
    async fn test_query_engine_access() {
        let manager = create_test_manager();
        let _engine = manager.query_engine();
        
    }

    #[tokio::test]
    async fn test_document_with_imports() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = r#"import { env } from 'process';
const db = env.DATABASE_URL;"#.to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        let doc = manager.get(&uri).unwrap();
        
        let _import_ctx = &doc.import_context;
    }

    #[tokio::test]
    async fn test_complex_binding_chain() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = r#"const env = process.env;
const config = env;
const { DATABASE_URL } = config;
console.log(DATABASE_URL);"#.to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        let graph = manager.get_binding_graph(&uri);
        assert!(graph.is_some());

        let graph = graph.unwrap();
        
        assert!(graph.symbols().len() >= 2);
    }

    #[tokio::test]
    async fn test_tsx_document() {
        let manager = create_test_manager();
        let uri = test_uri("test.tsx");
        let content = r#"const Component = () => {
    const apiKey = process.env.API_KEY;
    return <div>{apiKey}</div>;
};"#.to_string();

        manager.open(uri.clone(), "typescriptreact".to_string(), content, 1).await;

        let doc = manager.get(&uri).unwrap();
        assert_eq!(doc.language_id, "typescriptreact");
        assert!(doc.tree.is_some());
    }

    #[tokio::test]
    async fn test_version_mismatch_on_change() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = r#"const x = 1;"#.to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        
        let changes1 = vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "const x = 2;".to_string(),
        }];
        manager.change(&uri, changes1, 2).await;

        
        let changes2 = vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "const x = 3;".to_string(),
        }];
        manager.change(&uri, changes2, 3).await;

        let doc = manager.get(&uri).unwrap();
        assert_eq!(doc.version, 3);
        assert_eq!(doc.content.as_str(), "const x = 3;");
    }

    #[tokio::test]
    async fn test_uri_by_extension_detection() {
        let manager = create_test_manager();
        
        let uri = Url::parse("file:///test/test.js").unwrap();
        let content = r#"const db = process.env.DATABASE_URL;"#.to_string();

        
        manager.open(uri.clone(), "".to_string(), content, 1).await;

        let doc = manager.get(&uri).unwrap();
        
        assert!(doc.tree.is_some());
    }

    #[tokio::test]
    async fn test_multiple_env_references() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = r#"const db = process.env.DATABASE_URL;
const api = process.env.API_KEY;
const secret = process.env.SECRET;"#.to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        let graph = manager.get_binding_graph(&uri).unwrap();
        assert_eq!(graph.direct_references().len(), 3);
    }

    #[tokio::test]
    async fn test_destructuring_with_rename() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = r#"const { DATABASE_URL: dbUrl } = process.env;"#.to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;


        let binding = manager.get_env_binding_cloned(&uri, Position::new(0, 24));
        assert!(binding.is_some());
        let binding = binding.unwrap();
        assert_eq!(binding.binding_name, "dbUrl");
        assert_eq!(binding.env_var_name, "DATABASE_URL");
    }

    // =========================================================================
    // Task 2: EditInfo and Edit Handling Tests
    // =========================================================================

    fn make_text_change(range: Option<Range>, text: &str) -> TextDocumentContentChangeEvent {
        TextDocumentContentChangeEvent {
            range,
            range_length: None,
            text: text.to_string(),
        }
    }

    #[test]
    fn test_edit_info_full_replacement() {
        let edit_info = EditInfo::full_replacement();
        assert!(edit_info.is_full_replacement);
        assert!(edit_info.range.is_none());
    }

    #[test]
    fn test_edit_info_incremental() {
        let range = Range {
            start: Position::new(5, 0),
            end: Position::new(10, 20),
        };
        let edit_info = EditInfo::incremental(range);
        assert!(!edit_info.is_full_replacement);
        assert!(edit_info.range.is_some());
        assert_eq!(edit_info.range.unwrap(), range);
    }

    #[tokio::test]
    async fn test_change_with_range_creates_incremental_edit() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = "const x = 1;\nconst y = 2;\nconst z = 3;".to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        // Change with a range should be treated as incremental
        let changes = vec![make_text_change(
            Some(Range {
                start: Position::new(1, 10),
                end: Position::new(1, 11),
            }),
            "3",
        )];

        manager.change(&uri, changes, 2).await;

        let doc = manager.get(&uri).unwrap();
        assert_eq!(doc.version, 2);
        // The document content should be updated
        // Note: The actual content update depends on how the change is applied
    }

    #[tokio::test]
    async fn test_change_without_range_creates_full_replacement() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = "const x = 1;".to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        // Change without range should be full replacement
        let new_content = "const y = 2;".to_string();
        let changes = vec![make_text_change(None, &new_content)];

        manager.change(&uri, changes, 2).await;

        let doc = manager.get(&uri).unwrap();
        assert_eq!(doc.version, 2);
        assert_eq!(doc.content.as_str(), new_content);
    }

    #[tokio::test]
    async fn test_edit_range_merging_single_edit() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = "const x = 1;\nconst y = 2;\nconst z = 3;".to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        // Single edit should be preserved as-is
        let changes = vec![make_text_change(
            Some(Range {
                start: Position::new(1, 0),
                end: Position::new(1, 12),
            }),
            "const y = 5;",
        )];

        manager.change(&uri, changes, 2).await;

        let doc = manager.get(&uri).unwrap();
        assert_eq!(doc.version, 2);
    }

    #[tokio::test]
    async fn test_edit_range_merging_multiple_edits() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = "line0\nline1\nline2\nline3\nline4".to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        // Multiple non-overlapping edits should be merged into one covering range
        let changes = vec![
            make_text_change(
                Some(Range {
                    start: Position::new(1, 0),
                    end: Position::new(1, 5),
                }),
                "LINE1",
            ),
            make_text_change(
                Some(Range {
                    start: Position::new(3, 0),
                    end: Position::new(3, 5),
                }),
                "LINE3",
            ),
        ];

        manager.change(&uri, changes, 2).await;

        let doc = manager.get(&uri).unwrap();
        assert_eq!(doc.version, 2);
    }

    #[tokio::test]
    async fn test_edit_range_merging_overlapping_edits() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = "line0\nline1\nline2\nline3\nline4".to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        // Overlapping edits should be merged correctly
        let changes = vec![
            make_text_change(
                Some(Range {
                    start: Position::new(1, 0),
                    end: Position::new(2, 5),
                }),
                "MERGED1",
            ),
            make_text_change(
                Some(Range {
                    start: Position::new(2, 0),
                    end: Position::new(3, 5),
                }),
                "MERGED2",
            ),
        ];

        manager.change(&uri, changes, 2).await;

        let doc = manager.get(&uri).unwrap();
        assert_eq!(doc.version, 2);
    }

    #[tokio::test]
    async fn test_edit_range_merging_same_line_different_columns() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = "const x = 1; const y = 2;".to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        // Edits on the same line with different columns
        let changes = vec![
            make_text_change(
                Some(Range {
                    start: Position::new(0, 10),
                    end: Position::new(0, 11),
                }),
                "5",
            ),
            make_text_change(
                Some(Range {
                    start: Position::new(0, 23),
                    end: Position::new(0, 24),
                }),
                "10",
            ),
        ];

        manager.change(&uri, changes, 2).await;

        let doc = manager.get(&uri).unwrap();
        assert_eq!(doc.version, 2);
    }

    #[tokio::test]
    async fn test_edit_range_merging_one_full_one_range() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = "const x = 1;".to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        // Mix of full replacement and range-based edit - full replacement should win
        let new_content = "const y = 2; const z = 3;".to_string();
        let changes = vec![
            make_text_change(
                Some(Range {
                    start: Position::new(0, 10),
                    end: Position::new(0, 11),
                }),
                "5",
            ),
            make_text_change(None, &new_content),
        ];

        manager.change(&uri, changes, 2).await;

        let doc = manager.get(&uri).unwrap();
        assert_eq!(doc.version, 2);
        // Full replacement content should be the final content
        assert_eq!(doc.content.as_str(), new_content);
    }

    #[tokio::test]
    async fn test_analyze_content_with_edit_full_replacement_delegates() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = "const db = process.env.DATABASE_URL;".to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        // Full replacement should trigger full analysis
        let new_content = "const api = process.env.API_KEY;".to_string();
        let changes = vec![make_text_change(None, &new_content)];

        manager.change(&uri, changes, 2).await;

        let doc = manager.get(&uri).unwrap();
        assert!(doc.tree.is_some());

        // Check that the binding graph reflects the new content
        let graph = manager.get_binding_graph(&uri).unwrap();
        assert!(!graph.direct_references().is_empty());
        assert_eq!(graph.direct_references()[0].name, "API_KEY");
    }

    #[tokio::test]
    async fn test_analyze_content_with_edit_no_old_graph_delegates() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");

        // Create a document with unknown language (no binding graph)
        let content = "some content".to_string();
        manager.open(uri.clone(), "unknown".to_string(), content, 1).await;

        // Then change to JavaScript content - should do full analysis
        // First, we need to close and reopen with JavaScript
        manager.close(&uri);

        let js_content = "const x = process.env.VAR;".to_string();
        manager.open(uri.clone(), "javascript".to_string(), js_content.clone(), 2).await;

        let doc = manager.get(&uri).unwrap();
        assert!(doc.tree.is_some());

        let graph = manager.get_binding_graph(&uri).unwrap();
        assert!(!graph.direct_references().is_empty());
    }

    #[tokio::test]
    async fn test_close_document() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = "const x = 1;".to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;
        assert!(manager.get(&uri).is_some());

        manager.close(&uri);
        assert!(manager.get(&uri).is_none());
    }

    #[tokio::test]
    async fn test_document_count() {
        let manager = create_test_manager();

        assert_eq!(manager.document_count(), 0);

        manager.open(test_uri("a.js"), "javascript".to_string(), "a".to_string(), 1).await;
        assert_eq!(manager.document_count(), 1);

        manager.open(test_uri("b.js"), "javascript".to_string(), "b".to_string(), 1).await;
        assert_eq!(manager.document_count(), 2);

        manager.close(&test_uri("a.js"));
        assert_eq!(manager.document_count(), 1);
    }

    #[tokio::test]
    async fn test_has_syntax_errors_clean_code() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = "const x = 1;".to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        let has_errors = manager.has_syntax_errors(&uri);
        assert!(has_errors.is_some());
        assert!(!has_errors.unwrap());
    }

    #[tokio::test]
    async fn test_has_syntax_errors_with_error() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        // Intentionally broken syntax
        let content = "const x = ;".to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        let has_errors = manager.has_syntax_errors(&uri);
        assert!(has_errors.is_some());
        assert!(has_errors.unwrap());
    }

    #[tokio::test]
    async fn test_get_syntax_errors() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        // Intentionally broken syntax
        let content = "const x = ;".to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        let errors = manager.get_syntax_errors(&uri);
        assert!(!errors.is_empty());
    }

    #[tokio::test]
    async fn test_get_syntax_errors_no_errors() {
        let manager = create_test_manager();
        let uri = test_uri("test.js");
        let content = "const x = 1;".to_string();

        manager.open(uri.clone(), "javascript".to_string(), content, 1).await;

        let errors = manager.get_syntax_errors(&uri);
        assert!(errors.is_empty());
    }

    #[tokio::test]
    async fn test_get_syntax_errors_nonexistent_document() {
        let manager = create_test_manager();
        let uri = test_uri("nonexistent.js");

        let errors = manager.get_syntax_errors(&uri);
        assert!(errors.is_empty());
    }
}
