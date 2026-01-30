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

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::Url;

    #[test]
    fn test_parse_error_display() {
        let err = LspError::ParseError {
            uri: "file:///test.js".to_string(),
            reason: "unexpected token".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Parse failed for file:///test.js: unexpected token"
        );
    }

    #[test]
    fn test_resolution_error_display() {
        let err = LspError::ResolutionError {
            var_name: "DATABASE_URL".to_string(),
            reason: "not found".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Resolution failed for DATABASE_URL: not found"
        );
    }

    #[test]
    fn test_feature_disabled_display() {
        let err = LspError::FeatureDisabled {
            feature: "hover".to_string(),
        };
        assert_eq!(err.to_string(), "Feature disabled: hover");
    }

    #[test]
    fn test_document_not_found_display() {
        let err = LspError::DocumentNotFound {
            uri: "file:///missing.js".to_string(),
        };
        assert_eq!(err.to_string(), "Document not found: file:///missing.js");
    }

    #[test]
    fn test_unsupported_language_display() {
        let err = LspError::UnsupportedLanguage {
            language_id: "cobol".to_string(),
        };
        assert_eq!(err.to_string(), "Language not supported: cobol");
    }

    #[test]
    fn test_invalid_position_display() {
        let err = LspError::InvalidPosition {
            uri: "file:///test.js".to_string(),
            line: 10,
            character: 25,
        };
        assert_eq!(
            err.to_string(),
            "Invalid position: line 10, char 25 in file:///test.js"
        );
    }

    #[test]
    fn test_internal_error_display() {
        let err = LspError::Internal("unexpected state".to_string());
        assert_eq!(err.to_string(), "Internal error: unexpected state");
    }

    #[test]
    fn test_document_not_found_helper() {
        let uri = Url::parse("file:///path/to/test.js").unwrap();
        let err = LspError::document_not_found(&uri);
        assert!(matches!(err, LspError::DocumentNotFound { .. }));
        assert_eq!(err.to_string(), "Document not found: file:///path/to/test.js");
    }

    #[test]
    fn test_invalid_position_helper() {
        let uri = Url::parse("file:///path/to/test.js").unwrap();
        let position = Position::new(5, 10);
        let err = LspError::invalid_position(&uri, position);
        assert!(matches!(err, LspError::InvalidPosition { .. }));
        assert!(err.to_string().contains("line 5"));
        assert!(err.to_string().contains("char 10"));
    }

    #[test]
    fn test_feature_disabled_helper() {
        let err = LspError::feature_disabled("completion");
        assert!(matches!(err, LspError::FeatureDisabled { .. }));
        assert_eq!(err.to_string(), "Feature disabled: completion");
    }

    #[test]
    fn test_error_debug_format() {
        let err = LspError::Internal("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Internal"));
        assert!(debug_str.contains("test"));
    }
}
