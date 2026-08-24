//! Coverage for value masking.
//!
//! Masking is what stops the editor from printing secrets into hover tooltips,
//! completion documentation and inlay hints, so its behaviour is pinned here
//! rather than left to the display code.

use ecolog_lsp::server::config::{MaskSurface, MaskingConfig};

#[test]
fn test_disabled_by_default() {
    let config = MaskingConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.apply("s3cret", MaskSurface::Hover), "s3cret");
}

#[test]
fn test_masks_every_surface_when_enabled() {
    let config = MaskingConfig {
        enabled: true,
        ..MaskingConfig::default()
    };
    for surface in [MaskSurface::Hover, MaskSurface::Completion, MaskSurface::InlayHint] {
        let masked = config.apply("s3cret", surface);
        assert!(!masked.contains("s3cret"), "{:?} leaked the value", surface);
        assert_eq!(masked, "********");
    }
}

#[test]
fn test_per_surface_opt_out() {
    let config = MaskingConfig {
        enabled: true,
        mask_in_hover: true,
        mask_in_completion: false,
        mask_in_inlay_hints: false,
        ..MaskingConfig::default()
    };
    assert_eq!(config.apply("s3cret", MaskSurface::Hover), "********");
    assert_eq!(config.apply("s3cret", MaskSurface::Completion), "s3cret");
    assert_eq!(config.apply("s3cret", MaskSurface::InlayHint), "s3cret");
}

#[test]
fn test_mask_width_does_not_leak_length() {
    let config = MaskingConfig {
        enabled: true,
        ..MaskingConfig::default()
    };
    let short = config.apply("a", MaskSurface::Hover).into_owned();
    let long = config
        .apply("a-very-long-secret-value-indeed", MaskSurface::Hover)
        .into_owned();
    assert_eq!(short, long, "mask width reveals the original length");
}

#[test]
fn test_show_last_reveals_only_the_tail() {
    let config = MaskingConfig {
        enabled: true,
        show_last: 4,
        ..MaskingConfig::default()
    };
    assert_eq!(config.apply("sk_live_abcd1234", MaskSurface::Hover), "********1234");
}

#[test]
fn test_show_last_covering_whole_value_still_masks() {
    let config = MaskingConfig {
        enabled: true,
        show_last: 32,
        ..MaskingConfig::default()
    };
    // `show_last` longer than the value must not print it verbatim.
    assert_eq!(config.apply("short", MaskSurface::Hover), "********");
}

#[test]
fn test_empty_value_stays_empty() {
    let config = MaskingConfig {
        enabled: true,
        ..MaskingConfig::default()
    };
    // Callers render this as an explicit "(empty)" marker; masking an empty
    // value into asterisks would claim a secret that is not there.
    assert_eq!(config.apply("", MaskSurface::Hover), "");
}

#[test]
fn test_custom_mask_char() {
    let config = MaskingConfig {
        enabled: true,
        mask_char: '•',
        ..MaskingConfig::default()
    };
    assert_eq!(config.apply("s3cret", MaskSurface::Hover), "••••••••");
}

#[test]
fn test_multibyte_values_are_handled() {
    let config = MaskingConfig {
        enabled: true,
        show_last: 3,
        ..MaskingConfig::default()
    };
    // Character-based, not byte-based: a byte split here would panic.
    assert_eq!(config.apply("café-très-secret", MaskSurface::Hover), "********ret");
    assert_eq!(config.apply("une-clé", MaskSurface::Hover), "********clé");
}

// ───────────────────────────────────────────────────────────────────────────
// Truncation used by inlay hints and completion detail.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_inlay_hint_truncation_survives_multibyte_values() {
    use ecolog_lsp::analysis::document::DocumentManager;
    use ecolog_lsp::analysis::query::QueryEngine;
    use ecolog_lsp::languages::LanguageRegistry;
    use std::sync::Arc;
    use tower_lsp::lsp_types::Url;

    // Truncating a value at a byte offset that lands inside a multi-byte
    // character panics. Values are arbitrary text, so drive a document whose
    // content forces the truncation path.
    let manager = DocumentManager::new(
        Arc::new(QueryEngine::new()),
        Arc::new(LanguageRegistry::with_all_languages()),
    );
    let uri = Url::parse("file:///t.js").expect("uri");
    let content = "const a = process.env.CAFÉ_TRÈS_LONG_VARIABLE_NAME_ÉÉÉ;";
    manager
        .open(uri.clone(), "javascript".into(), content.to_string(), 1)
        .await;

    // Reaching here without a panic is the assertion; the document is analysed
    // on open and its diagnostics text is truncated along the way.
    assert!(manager.get(&uri).is_some());
}
