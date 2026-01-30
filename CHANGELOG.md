# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.3.0] - YYYY-MM-DD

### Added
- Lua language support
- Architecture improvements including error handling, caching, and diagnostics
- Improved memory usage stability

### Changed
- Removed inclusion of comments in generateExample

### Fixed
- Various e2e test issues

## [0.2.0] - YYYY-MM-DD

_Initial tracked release_

### Added
- Language Server Protocol implementation for environment variables
- Support for JavaScript, TypeScript, Python, Rust, and Go
- Auto-completion for environment variables
- Go to definition for env vars in .env files
- Hover information with values and sources
- Semantic token highlighting
- Diagnostics for undefined environment variables
- Value masking for sensitive data
- Configuration via ecolog.toml
- Variable interpolation support

[Unreleased]: https://github.com/OWNER/ecolog-lsp/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/OWNER/ecolog-lsp/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/OWNER/ecolog-lsp/releases/tag/v0.2.0
