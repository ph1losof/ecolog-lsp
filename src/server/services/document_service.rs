//! Document service for managing document state and analysis.
//!
//! This service wraps `DocumentManager` and provides a focused interface
//! for document-related operations with 100% cohesion.

use crate::analysis::{BindingGraph, DocumentEntry, DocumentManager, QueryEngine};
use crate::types::{BindingKind, DocumentState, EnvBinding, EnvBindingUsage, EnvReference};
use compact_str::CompactString;
use std::sync::Arc;
use tower_lsp::lsp_types::{Position, Range, TextDocumentContentChangeEvent, Url};

/// Service for document management operations.
///
/// Provides a cohesive interface for all document-related operations including:
/// - Opening, closing, and updating documents
/// - Retrieving document state and binding graphs
/// - Querying for env references, bindings, and usages
pub struct DocumentService {
    manager: Arc<DocumentManager>,
}

impl DocumentService {
    /// Creates a new DocumentService wrapping the given DocumentManager.
    pub fn new(manager: Arc<DocumentManager>) -> Self {
        Self { manager }
    }

    /// Returns a reference to the underlying DocumentManager.
    ///
    /// This allows direct access when needed for operations not yet wrapped
    /// by the service interface.
    #[inline]
    pub fn manager(&self) -> &Arc<DocumentManager> {
        &self.manager
    }

    /// Opens a document with the given parameters.
    pub async fn open(&self, uri: Url, language_id: String, content: String, version: i32) {
        self.manager.open(uri, language_id, content, version).await;
    }

    /// Applies changes to an open document.
    pub async fn update(
        &self,
        uri: &Url,
        changes: Vec<TextDocumentContentChangeEvent>,
        version: i32,
    ) {
        self.manager.change(uri, changes, version).await;
    }

    /// Closes a document, removing it from management.
    pub fn close(&self, uri: &Url) {
        self.manager.close(uri);
    }

    /// Gets the document state for a URI.
    pub fn get(
        &self,
        uri: &Url,
    ) -> Option<dashmap::mapref::one::MappedRef<'_, Url, DocumentEntry, DocumentState>>
    {
        self.manager.get(uri)
    }

    /// Gets the binding graph for a document.
    pub fn get_binding_graph(&self, uri: &Url) -> Option<Arc<BindingGraph>> {
        self.manager.get_binding_graph(uri)
    }

    /// Gets an env reference at the given position.
    pub fn get_env_reference(&self, uri: &Url, position: Position) -> Option<EnvReference> {
        self.manager.get_env_reference_cloned(uri, position)
    }

    /// Gets an env binding at the given position.
    pub fn get_env_binding(&self, uri: &Url, position: Position) -> Option<EnvBinding> {
        self.manager.get_env_binding_cloned(uri, position)
    }

    /// Gets a binding usage at the given position.
    pub fn get_binding_usage(&self, uri: &Url, position: Position) -> Option<EnvBindingUsage> {
        self.manager.get_binding_usage_cloned(uri, position)
    }

    /// Gets the binding kind for a usage by name.
    pub fn get_binding_kind(&self, uri: &Url, binding_name: &str) -> Option<BindingKind> {
        self.manager.get_binding_kind_for_usage(uri, binding_name)
    }

    /// Checks if completion should be triggered at the given position.
    pub async fn check_completion(&self, uri: &Url, position: Position) -> bool {
        self.manager.check_completion(uri, position).await
    }

    /// Gets the completion context (object name) at the given position.
    pub async fn check_completion_context(
        &self,
        uri: &Url,
        position: Position,
    ) -> Option<CompactString> {
        self.manager.check_completion_context(uri, position).await
    }

    /// Returns all open document URIs.
    pub fn all_uris(&self) -> Vec<Url> {
        self.manager.all_uris()
    }

    /// Returns the number of open documents.
    pub fn document_count(&self) -> usize {
        self.manager.document_count()
    }

    /// Returns a reference to the query engine.
    pub fn query_engine(&self) -> &Arc<QueryEngine> {
        self.manager.query_engine()
    }

    /// Checks if a document has syntax errors.
    pub fn has_syntax_errors(&self, uri: &Url) -> Option<bool> {
        self.manager.has_syntax_errors(uri)
    }

    /// Gets syntax errors with their ranges and messages.
    pub fn get_syntax_errors(&self, uri: &Url) -> Vec<(Range, Option<String>)> {
        self.manager.get_syntax_errors(uri)
    }
}

impl Clone for DocumentService {
    fn clone(&self) -> Self {
        Self {
            manager: Arc::clone(&self.manager),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::QueryEngine;
    use crate::languages::go::Go;
    use crate::languages::javascript::JavaScript;
    use crate::languages::python::Python;
    use crate::languages::rust::Rust;
    use crate::languages::typescript::{TypeScript, TypeScriptReact};
    use crate::languages::LanguageRegistry;

    fn create_test_service() -> DocumentService {
        let query_engine = Arc::new(QueryEngine::new());
        let mut registry = LanguageRegistry::new();
        registry.register(Arc::new(JavaScript));
        registry.register(Arc::new(TypeScript));
        registry.register(Arc::new(TypeScriptReact));
        registry.register(Arc::new(Python));
        registry.register(Arc::new(Rust));
        registry.register(Arc::new(Go));
        let languages = Arc::new(registry);
        let manager = Arc::new(DocumentManager::new(query_engine, languages));
        DocumentService::new(manager)
    }

    fn test_uri(name: &str) -> Url {
        Url::parse(&format!("file:///test/{}", name)).unwrap()
    }

    #[tokio::test]
    async fn test_open_and_get() {
        let service = create_test_service();
        let uri = test_uri("test.js");
        let content = "const x = process.env.DATABASE_URL;".to_string();

        service
            .open(uri.clone(), "javascript".to_string(), content, 1)
            .await;

        let doc = service.get(&uri);
        assert!(doc.is_some());
        let doc = doc.unwrap();
        assert_eq!(doc.version, 1);
    }

    #[tokio::test]
    async fn test_update_document() {
        let service = create_test_service();
        let uri = test_uri("test.js");

        service
            .open(uri.clone(), "javascript".to_string(), "const x = 1;".to_string(), 1)
            .await;

        let changes = vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "const y = 2;".to_string(),
        }];

        service.update(&uri, changes, 2).await;

        let doc = service.get(&uri).unwrap();
        assert_eq!(doc.version, 2);
        assert_eq!(doc.content.as_str(), "const y = 2;");
    }

    #[tokio::test]
    async fn test_close_document() {
        let service = create_test_service();
        let uri = test_uri("test.js");

        service
            .open(uri.clone(), "javascript".to_string(), "const x = 1;".to_string(), 1)
            .await;

        assert!(service.get(&uri).is_some());

        service.close(&uri);

        assert!(service.get(&uri).is_none());
    }

    #[tokio::test]
    async fn test_get_binding_graph() {
        let service = create_test_service();
        let uri = test_uri("test.js");
        let content = "const db = process.env.DATABASE_URL;".to_string();

        service
            .open(uri.clone(), "javascript".to_string(), content, 1)
            .await;

        let graph = service.get_binding_graph(&uri);
        assert!(graph.is_some());
        assert!(!graph.unwrap().direct_references().is_empty());
    }

    #[tokio::test]
    async fn test_get_env_reference() {
        let service = create_test_service();
        let uri = test_uri("test.js");
        let content = "const x = process.env.DATABASE_URL;".to_string();

        service
            .open(uri.clone(), "javascript".to_string(), content, 1)
            .await;

        let reference = service.get_env_reference(&uri, Position::new(0, 22));
        assert!(reference.is_some());
        assert_eq!(reference.unwrap().name, "DATABASE_URL");
    }

    #[tokio::test]
    async fn test_document_count() {
        let service = create_test_service();

        assert_eq!(service.document_count(), 0);

        service
            .open(test_uri("a.js"), "javascript".to_string(), "a".to_string(), 1)
            .await;
        assert_eq!(service.document_count(), 1);

        service
            .open(test_uri("b.js"), "javascript".to_string(), "b".to_string(), 1)
            .await;
        assert_eq!(service.document_count(), 2);
    }

    #[tokio::test]
    async fn test_all_uris() {
        let service = create_test_service();
        let uri1 = test_uri("a.js");
        let uri2 = test_uri("b.js");

        service
            .open(uri1.clone(), "javascript".to_string(), "a".to_string(), 1)
            .await;
        service
            .open(uri2.clone(), "javascript".to_string(), "b".to_string(), 1)
            .await;

        let uris = service.all_uris();
        assert_eq!(uris.len(), 2);
        assert!(uris.contains(&uri1));
        assert!(uris.contains(&uri2));
    }

    #[tokio::test]
    async fn test_clone() {
        let service = create_test_service();
        let uri = test_uri("test.js");

        service
            .open(uri.clone(), "javascript".to_string(), "const x = 1;".to_string(), 1)
            .await;

        let cloned = service.clone();
        assert!(cloned.get(&uri).is_some());
    }
}
