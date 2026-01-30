//! Service layer for the LSP server.
//!
//! This module provides focused service structs with single responsibilities,
//! decomposing the ServerState god object into cohesive components.

pub mod document_service;
pub mod env_service;
pub mod workspace_service;

pub use document_service::DocumentService;
pub use env_service::EnvService;
pub use workspace_service::WorkspaceService;
