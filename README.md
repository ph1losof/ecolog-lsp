# Ecolog LSP

[![CI](https://github.com/ecolog-lsp/ecolog-lsp/actions/workflows/ci.yml/badge.svg)](https://github.com/ecolog-lsp/ecolog-lsp/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/ecolog-lsp/ecolog-lsp/graph/badge.svg)](https://codecov.io/gh/ecolog-lsp/ecolog-lsp)

A language-agnostic Language Server Protocol (LSP) implementation for environment variables, providing intelligent code assistance for environment variable references across multiple programming languages.

## Features

- **Auto-completion**: Suggests available environment variables as you type
- **Go to Definition**: Navigate to where environment variables are defined in `.env` files
- **Hover Information**: View environment variable values, sources, and metadata on hover
- **Diagnostics**: Warnings for undefined or misconfigured environment variables
- **Find References**: Locate every use of a variable across the workspace
- **Rename**: Rename a variable across code and `.env` files together
- **Inlay Hints**: Show resolved values inline (opt in with `[features].inlay_hints`)
- **Workspace Symbols**: Search environment variables by name
- **Value Masking**: Hide resolved values in the editor (opt in with `[masking].enabled`)
- **Multi-language Support**: 17 languages via per-language tree-sitter queries

## Supported Languages

- JavaScript
- TypeScript (including TSX)
- Python
- Rust
- Go
- Lua
- PHP
- Ruby
- Java
- Kotlin
- C#
- C
- C++
- Elixir
- Zig
- Bash

Each language has custom tree-sitter queries to accurately detect environment variable access patterns specific to that language's idioms, including reads through an alias (`const env = process.env; env.PORT`) and, for JavaScript and TypeScript, Vite's `import.meta.env`.

## Installation

### Building from Source

```bash
cargo build --release
```

The compiled binary will be available at `target/release/ecolog-lsp`.

### Prerequisites

- Rust 1.70 or later
- Cargo

## Configuration

The LSP can be configured via an `ecolog.toml` file in your workspace root. If no configuration file is found, sensible defaults are used.

### Example Configuration

```toml
[workspace]
env_files = [".env", ".env.local", ".env.development"]

[features]
completion = true
hover = true
definition = true
diagnostics = true
# Off by default
inlay_hints = false

[masking]
# Off by default; turn on to hide resolved values in the editor
enabled = false
mask_in_hover = true
mask_in_completion = true
mask_in_inlay_hints = true
mask_char = "*"
# Trailing characters left visible, for recognising a value without showing it
show_last = 0

[interpolation]
enabled = true
max_depth = 10

[cache]
enabled = true
hot_cache_size = 100
ttl = 300

[indexing]
exclude = ["node_modules", ".git", "target", "dist", "build"]
max_files = 5000
max_file_size = 1048576
max_depth = 30
# 0 = pick automatically from the CPU count
parallelism = 0
```

### Configuration Options

#### `[workspace]`

- `env_files`: Array of environment file paths to load (relative to workspace root)

#### `[features]`

- `completion`: Enable/disable auto-completion
- `hover`: Enable/disable hover information
- `definition`: Enable/disable go-to-definition
- `diagnostics`: Enable/disable diagnostics
- `inlay_hints`: Show resolved values inline (default: off)

#### `[masking]`

- `enabled`: Master switch for value masking (default: off)
- `mask_in_hover`: Mask values in hover tooltips
- `mask_in_completion`: Mask values in completion item documentation
- `mask_in_inlay_hints`: Mask values in inlay hints
- `mask_char`: Character the value is replaced with
- `show_last`: How many trailing characters stay visible (`0` hides everything)

The mask is a fixed width, so it does not reveal how long the original value
was. Commands that return values programmatically are not masked.

#### `[interpolation]`

- `enabled`: Support variable interpolation (e.g., `${VAR}` syntax)
- `max_depth`: Maximum nesting depth for interpolated variables

#### `[cache]`

- `enabled`: Enable caching of resolved values
- `hot_cache_size`: Number of frequently accessed variables to cache
- `ttl`: Cache time-to-live in seconds

#### `[indexing]`

Controls the background scan of the workspace that powers workspace-wide
references, rename and diagnostics.

- `exclude`: Directory names skipped during the scan, at any depth
- `max_files`: Stop after this many files (`0` for no limit)
- `max_file_size`: Skip files larger than this many bytes (`0` for no limit)
- `max_depth`: Maximum directory nesting to walk (`0` for no limit)
- `parallelism`: How many files are analyzed concurrently. `0` derives a value
  from the CPU count that leaves headroom for the editor. Lower it to make
  indexing gentler on a busy machine, raise it to index a large repository
  faster. The `ECOLOG_INDEX_PARALLELISM` environment variable overrides it.

If indexing a large repository is using more CPU than you want, the two knobs
that matter most are `exclude` (keep generated and vendored trees out of the
scan) and `parallelism`.

## Editor Integration

### VSCode

Add to your `settings.json`:

```json
{
  "ecolog-lsp.enable": true,
  "ecolog-lsp.serverPath": "/path/to/ecolog-lsp"
}
```

Or install via a VSCode extension if available.

### Neovim

Using `nvim-lspconfig`:

```lua
local lspconfig = require('lspconfig')
local configs = require('lspconfig.configs')

if not configs.ecolog then
  configs.ecolog = {
    default_config = {
      cmd = {'/path/to/ecolog-lsp'},
      filetypes = {'javascript', 'typescript', 'python', 'rust', 'lua', 'go'},
      root_dir = lspconfig.util.root_pattern('.env', '.git'),
      settings = {},
    },
  }
end

lspconfig.ecolog.setup{}
```

### Other Editors

The LSP server communicates via stdin/stdout, so it can be integrated with any editor that supports the Language Server Protocol. Refer to your editor's LSP client documentation.

## Architecture

Ecolog LSP is built on several core components:

- **Tree-sitter**: For language-specific parsing and pattern matching
- **Abundantis**: Core environment variable resolution engine with support for multiple sources
- **Shelter**: Secure value masking to protect sensitive information
- **Korni**: Dotenv parser written in rust
- **tower-lsp**: LSP protocol implementation

### How It Works

1. The LSP monitors your workspace for `.env` files and code files
2. Tree-sitter parses code to identify environment variable references
3. Abundantis resolves variable values from configured sources
4. The LSP provides intelligent suggestions and information to your editor
5. Shelter masks sensitive values when configured

## Development

### Running Tests

```bash
cargo test
```

### Running with Logging

```bash
RUST_LOG=debug cargo run
```

### Project Structure

```
ecolog-lsp/
├── src/
│   ├── analysis/       # AST analysis and binding resolution
│   ├── languages/      # Language-specific parsers and queries
│   ├── server/         # LSP server implementation
│   └── types.rs        # Core type definitions
├── queries/            # Tree-sitter query files per language
├── tests/              # Integration and unit tests
└── Cargo.toml
```

## Use Cases

- **Development**: Real-time validation of environment variable usage
- **Onboarding**: Help new developers understand which variables are available
- **Refactoring**: Safely rename or restructure environment variables
- **Security**: Prevent accidental exposure of sensitive values
- **Documentation**: Inline documentation of variable purposes and sources

## License

See LICENSE file for details.

## Contributing

Contributions are welcome! Please ensure tests pass before submitting pull requests.

```bash
cargo test
cargo fmt
cargo clippy
```
