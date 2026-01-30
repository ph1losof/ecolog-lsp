# Ecolog LSP

[![CI](https://github.com/ecolog-lsp/ecolog-lsp/actions/workflows/ci.yml/badge.svg)](https://github.com/ecolog-lsp/ecolog-lsp/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/ecolog-lsp/ecolog-lsp/graph/badge.svg)](https://codecov.io/gh/ecolog-lsp/ecolog-lsp)

A language-agnostic Language Server Protocol (LSP) implementation for environment variables, providing intelligent code assistance for environment variable references across multiple programming languages.

## Features

- **Auto-completion**: Suggests available environment variables as you type
- **Go to Definition**: Navigate to where environment variables are defined in `.env` files
- **Hover Information**: View environment variable values, sources, and metadata on hover
- **Semantic Tokens**: Syntax highlighting for environment variable references
- **Diagnostics**: Warnings for undefined or misconfigured environment variables
- **Value Masking**: Secure handling of sensitive values in editor tooltips
- **Multi-language Support**: Works across JavaScript, TypeScript, Python, Rust, Lua and Go

## Supported Languages

- JavaScript
- TypeScript
- Python
- Rust
- Lua
- Go

Each language has custom tree-sitter queries to accurately detect environment variable access patterns specific to that language's idioms.

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
semantic_tokens = true

[masking]
enabled = true
# Mask values in hover tooltips for security
mask_in_hover = true
mask_in_completion = false

[interpolation]
enabled = true
max_depth = 10

[cache]
enabled = true
hot_cache_size = 100
ttl = 300
```

### Configuration Options

#### `[workspace]`

- `env_files`: Array of environment file paths to load (relative to workspace root)

#### `[features]`

- `completion`: Enable/disable auto-completion
- `hover`: Enable/disable hover information
- `definition`: Enable/disable go-to-definition
- `diagnostics`: Enable/disable diagnostics
- `semantic_tokens`: Enable/disable semantic token highlighting

#### `[masking]`

- `enabled`: Master switch for value masking
- `mask_in_hover`: Mask sensitive values in hover tooltips
- `mask_in_completion`: Mask values in completion items

#### `[interpolation]`

- `enabled`: Support variable interpolation (e.g., `${VAR}` syntax)
- `max_depth`: Maximum nesting depth for interpolated variables

#### `[cache]`

- `enabled`: Enable caching of resolved values
- `hot_cache_size`: Number of frequently accessed variables to cache
- `ttl`: Cache time-to-live in seconds

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
