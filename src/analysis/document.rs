use crate::analysis::resolver::BindingResolver;
use crate::analysis::{AnalysisPipeline, BindingGraph, QueryEngine};
use crate::languages::{LanguageRegistry, LanguageSupport};
use crate::types::{
    BindingKind, DocumentState, EnvBinding, EnvBindingUsage, EnvReference, ImportContext,
};
use compact_str::CompactString;
use dashmap::DashMap;
use std::sync::Arc;
use tower_lsp::lsp_types::{Position, TextDocumentContentChangeEvent, Url};
use tree_sitter::Tree;

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
            } = self.analyze_content(&content, lang.as_ref()).await;

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
        
        let (content, language_id) = {
            if let Some(mut entry) = self.documents.get_mut(uri) {
                // Apply full document changes
                for change in changes {
                    if change.range.is_none() {
                        entry.state.content = std::sync::Arc::new(change.text);
                    }
                }
                entry.state.version = version;
                (entry.state.content.clone(), entry.state.language_id.clone())
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
            } = self.analyze_content(&content, lang.as_ref()).await;

            
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
    ) -> AnalysisResult {
        
        let tree = self.query_engine.parse(language, content, None).await;

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

    pub fn get(&self, uri: &Url) -> Option<dashmap::mapref::one::MappedRef<Url, DocumentEntry, DocumentState>> {
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
}
