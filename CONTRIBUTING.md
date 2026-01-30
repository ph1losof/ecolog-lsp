# Contributing to ecolog-lsp

Thank you for your interest in contributing to ecolog-lsp! This document provides guidelines and instructions for contributing.

## Development Setup

### Prerequisites

- Rust 1.70 or later
- Cargo

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run with debug logging
RUST_LOG=debug cargo run
```

### Testing

```bash
# Run all tests
cargo test

# Run a specific test
cargo test <test_name>

# Run e2e tests only
cargo test --test e2e
```

### Code Quality

Before submitting a PR, ensure your code passes all checks:

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Run all checks (recommended before commit)
cargo fmt && cargo clippy && cargo test
```

## Pull Request Process

1. Fork the repository and create your branch from `main`
2. Make your changes and add tests if applicable
3. Ensure all tests pass and code is formatted
4. Update documentation if you're changing behavior
5. Submit your pull request

### Commit Messages

Use clear, descriptive commit messages:
- `feat:` for new features
- `fix:` for bug fixes
- `docs:` for documentation changes
- `refactor:` for code refactoring
- `test:` for adding tests
- `chore:` for maintenance tasks

Example: `feat: add Ruby language support`

## Release Process

Releases are automated via GitHub Actions. Here's how to create a new release:

### 1. Update Version

Update the version in `Cargo.toml`:

```toml
[package]
version = "0.4.0"  # Update this
```

### 2. Update Changelog

Add release notes to `CHANGELOG.md`:

```markdown
## [0.4.0] - 2024-XX-XX

### Added
- New feature description

### Fixed
- Bug fix description
```

### 3. Commit and Tag

```bash
# Commit version bump
git add Cargo.toml CHANGELOG.md
git commit -m "chore: bump version to 0.4.0"

# Create and push tag
git tag v0.4.0
git push origin main
git push origin v0.4.0
```

### 4. Automated Release

Once the tag is pushed, GitHub Actions will automatically:
- Build binaries for all supported platforms
- Create a GitHub Release with the binaries
- Generate SHA256 checksums
- Create release notes from commits

### Pre-releases

For alpha, beta, or release candidate versions:

```bash
# Alpha release
git tag v0.4.0-alpha.1
git push origin v0.4.0-alpha.1

# Beta release
git tag v0.4.0-beta.1
git push origin v0.4.0-beta.1

# Release candidate
git tag v0.4.0-rc.1
git push origin v0.4.0-rc.1
```

Pre-releases are automatically marked as such on GitHub.

### Manual Release

You can also trigger a release manually from the GitHub Actions UI:

1. Go to Actions > Release
2. Click "Run workflow"
3. Enter the tag (e.g., `v0.4.0`)
4. Optionally mark as pre-release
5. Click "Run workflow"

## Adding Language Support

To add support for a new programming language:

1. **Create the language module** in `src/languages/<lang>.rs`:
   - Implement the `LanguageSupport` trait
   - Define language-specific env access patterns

2. **Add tree-sitter queries** in `queries/<lang>/`:
   - `references.scm` - Direct env var accesses
   - `bindings.scm` - Variable bindings from env
   - `assignments.scm` - Variable chain assignments
   - `destructures.scm` - Destructuring patterns
   - `identifiers.scm` - All identifiers
   - `reassignments.scm` - Binding invalidations

3. **Add tree-sitter dependency** to `Cargo.toml`:
   ```toml
   tree-sitter-<lang> = "x.y"
   ```

4. **Register the language** in `src/server/mod.rs`

5. **Add tests** for the new language

6. **Update documentation**

## Project Structure

```
ecolog-lsp/
├── src/
│   ├── analysis/       # AST analysis and binding resolution
│   ├── languages/      # Language-specific parsers
│   ├── server/         # LSP server implementation
│   └── types.rs        # Core types
├── queries/            # Tree-sitter queries per language
├── tests/              # Integration tests
├── .github/workflows/  # CI/CD workflows
└── Cargo.toml
```

## Questions?

Feel free to open an issue for questions or discussions about potential contributions.
