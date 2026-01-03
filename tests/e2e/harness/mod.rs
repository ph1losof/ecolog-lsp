//! E2E test harness for ecolog-lsp
//!
//! Provides utilities for spawning the LSP server and communicating
//! with it via JSON-RPC protocol.

pub mod client;
pub mod workspace;

pub use client::LspTestClient;
pub use workspace::TempWorkspace;
