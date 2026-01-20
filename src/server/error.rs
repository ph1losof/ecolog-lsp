use thiserror::Error;
use tower_lsp::lsp_types::Position;

#[derive(Debug, Error)]
pub enum LspError {
    #[error("Parse failed for {uri}: {reason}")]
    ParseError { uri: String, reason: String },

    #[error("Resolution failed for {var_name}: {reason}")]
    ResolutionError { var_name: String, reason: String },

    #[error("Feature disabled: {feature}")]
    FeatureDisabled { feature: String },

    #[error("Document not found: {uri}")]
    DocumentNotFound { uri: String },

    #[error("Language not supported: {language_id}")]
    UnsupportedLanguage { language_id: String },

    #[error("Invalid position: line {line}, char {character} in {uri}")]
    InvalidPosition {
        uri: String,
        line: u32,
        character: u32,
    },

    #[error("Internal error: {0}")]
    Internal(String),
}

impl LspError {
    pub fn log_debug(&self) {
        tracing::debug!("LSP Error: {}", self);
    }

    pub fn log_warn(&self) {
        tracing::warn!("LSP Error: {}", self);
    }

    pub fn document_not_found(uri: &tower_lsp::lsp_types::Url) -> Self {
        Self::DocumentNotFound {
            uri: uri.to_string(),
        }
    }

    pub fn invalid_position(uri: &tower_lsp::lsp_types::Url, position: Position) -> Self {
        Self::InvalidPosition {
            uri: uri.to_string(),
            line: position.line,
            character: position.character,
        }
    }

    pub fn feature_disabled(feature: &str) -> Self {
        Self::FeatureDisabled {
            feature: feature.to_string(),
        }
    }
}
