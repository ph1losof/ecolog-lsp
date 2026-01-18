//! End-to-end protocol tests for ecolog-lsp
//!
//! These tests spawn the LSP server as a subprocess and communicate
//! with it via JSON-RPC over stdio, testing the full protocol.
//!
//! Run with: `cargo test --test e2e`
//!
//! Set ECOLOG_LSP_BINARY to override the binary path:
//! `ECOLOG_LSP_BINARY=./target/release/ecolog-lsp cargo test --test e2e`

mod harness;

mod lifecycle_test;
mod hover_test;
mod completion_test;
mod diagnostics_test;
mod definition_test;
mod references_test;
mod rename_test;
mod commands_test;
mod sync_test;
mod error_test;
mod workspace_symbol_test;
