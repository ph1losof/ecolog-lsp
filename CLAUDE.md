# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build              # Build the LSP server
cargo build --release    # Release build (outputs to target/release/ecolog-lsp)
cargo test               # Run all tests
cargo test <test_name>   # Run a specific test
cargo clippy             # Lint
cargo fmt                # Format code
RUST_LOG=debug cargo run # Run with debug logging
```

## Architecture Overview

Ecolog LSP is a language-agnostic Language Server Protocol implementation for environment variables. It provides IDE features (completion, hover, go-to-definition, diagnostics) for env var references across JavaScript, TypeScript, Python, Rust, and Go.

### Core Components

**Analysis Layer** (`src/analysis/`)
- `BindingGraph` - Arena-based sparse graph tracking env var bindings and their chains. Uses `SymbolId`/`ScopeId` for efficient ID-based references. Handles patterns like `const env = process.env; const { DB_URL } = env;`
- `AnalysisPipeline` - 6-phase document analysis: scopes → direct refs → bindings → resolve origins → usages → reassignments
- `BindingResolver` - Query-time resolution for LSP features. Returns `EnvHit` enum distinguishing direct references, symbol declarations, and usages
- `QueryEngine` - Executes tree-sitter queries with parser/cursor pooling
- `DocumentManager` - Manages per-document state and binding graphs using DashMap for concurrency

**Language Layer** (`src/languages/`)
- `LanguageSupport` trait - Defines language-specific behavior: grammar, queries, scope detection, env object patterns
- Each language (JavaScript, TypeScript, Python, Rust, Go) implements this trait and loads queries from `queries/<lang>/`

**Server Layer** (`src/server/`)
- `LspServer` - tower-lsp based server implementing standard LSP methods
- `handlers.rs` - Request handlers for hover, completion, definition, diagnostics, semantic tokens
- `config.rs` - Configuration from `ecolog.toml` with feature toggles, masking settings, workspace config

### External Dependencies (Workspace Crates)

- **abundantis** - Environment variable resolution engine with multi-source support, workspace awareness, and caching
- **shelter** - Value masking for sensitive data
- **korni** - Zero-copy .env file parser
- **germi** - SIMD-accelerated variable interpolation (used by abundantis)

### Data Flow

1. Document opens → `DocumentManager.open()` triggers `AnalysisPipeline.analyze()`
2. Pipeline extracts scopes, references, bindings via tree-sitter queries
3. `BindingGraph` stores symbols with `SymbolOrigin` enum tracking chain origins
4. LSP requests use `BindingResolver` to query the graph at cursor position
5. Values resolved via `abundantis` from configured sources (.env files, shell, etc.)

### Key Types (`src/types.rs`)

- `SymbolOrigin` - Tracks what a symbol resolves to: `EnvVar`, `EnvObject`, `Symbol` (chain), `DestructuredProperty`
- `Symbol` - Variable declaration in binding graph with origin, scope, validity
- `EnvReference` - Direct env var access (e.g., `process.env.DATABASE_URL`)
- `EnvBinding` - Local variable bound to an env var

### Tree-sitter Queries (`queries/`)

Each language has query files:
- `references.scm` - Detect direct env var accesses
- `bindings.scm` - Detect variable bindings from env
- `assignments.scm` - Track `const b = a` chains
- `destructures.scm` - Track `const { X } = obj` patterns
- `identifiers.scm` - Extract all identifiers for usage tracking
- `reassignments.scm` - Detect when bindings are invalidated

Queries use captures like `@env_access`, `@env_var_name`, `@binding_name`, `@bound_env_var`.

### Configuration

`ecolog.toml` in workspace root:
```toml
[features]
hover = true
completion = true
diagnostics = true
definition = true

[masking]
hover = false       # Mask values in hover
completion = false  # Mask values in completion

[workspace]
env_files = [".env", ".env.local"]

[interpolation]
enabled = true
max_depth = 10
```

### Adding a New Language

1. Create `src/languages/<lang>.rs` implementing `LanguageSupport`
2. Add tree-sitter queries in `queries/<lang>/`
3. Register in `LspServer::new_with_config()` via `registry.register()`
4. Add tree-sitter grammar dependency to `Cargo.toml`
