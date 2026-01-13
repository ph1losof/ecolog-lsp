//! Tests for source precedence (Shell/File/Remote) functionality.
//!
//! These tests verify that:
//! - Source precedence can be changed via `setPrecedence` command
//! - Disabled sources don't appear in hover/completion
//! - Disabled sources trigger "undefined" diagnostics
//! - Re-enabling sources restores functionality
//! - Precedence persists across file changes

mod common;
use common::TestFixture;
use ecolog_lsp::server::handlers::{compute_diagnostics, handle_completion, handle_execute_command, handle_hover};
use serde_json::json;
use tower_lsp::lsp_types::{
    CompletionContext, CompletionParams, CompletionTriggerKind, ExecuteCommandParams,
    HoverParams, Position, TextDocumentIdentifier, TextDocumentPositionParams,
};

/// Helper to set shell environment variable and refresh core
async fn set_shell_var(fixture: &TestFixture, name: &str, value: &str) {
    std::env::set_var(name, value);
    fixture.state.core.refresh().await.expect("Refresh failed");
}

/// Helper to remove shell environment variable and refresh core
async fn remove_shell_var(fixture: &TestFixture, name: &str) {
    std::env::remove_var(name);
    fixture.state.core.refresh().await.expect("Refresh failed");
}

/// Helper to set precedence via command
async fn set_precedence(fixture: &TestFixture, sources: Vec<&str>) -> Option<serde_json::Value> {
    let args: Vec<serde_json::Value> = sources.iter().map(|s| json!(s)).collect();
    let params = ExecuteCommandParams {
        command: "ecolog.source.setPrecedence".to_string(),
        arguments: args,
        work_done_progress_params: Default::default(),
    };
    handle_execute_command(params, &fixture.state).await
}

/// Helper to get hover at position
async fn get_hover(fixture: &TestFixture, uri: &tower_lsp::lsp_types::Url, line: u32, col: u32) -> Option<tower_lsp::lsp_types::Hover> {
    handle_hover(
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(line, col),
            },
            work_done_progress_params: Default::default(),
        },
        &fixture.state,
    )
    .await
}

/// Helper to get completions at position
async fn get_completions(fixture: &TestFixture, uri: &tower_lsp::lsp_types::Url, line: u32, col: u32) -> Option<Vec<tower_lsp::lsp_types::CompletionItem>> {
    handle_completion(
        CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(line, col),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
                trigger_character: Some(".".to_string()),
            }),
        },
        &fixture.state,
    )
    .await
}

// =============================================================================
// Test: setPrecedence command works
// =============================================================================

#[tokio::test]
async fn test_set_precedence_command() {
    let fixture = TestFixture::new().await;

    // Set precedence to File only
    let result = set_precedence(&fixture, vec!["File"]).await;
    assert!(result.is_some());
    let json = result.unwrap();
    assert!(json.get("success").unwrap().as_bool().unwrap());

    // Set precedence to Shell only
    let result = set_precedence(&fixture, vec!["Shell"]).await;
    assert!(result.is_some());
    let json = result.unwrap();
    assert!(json.get("success").unwrap().as_bool().unwrap());

    // Set precedence to both
    let result = set_precedence(&fixture, vec!["Shell", "File"]).await;
    assert!(result.is_some());
    let json = result.unwrap();
    assert!(json.get("success").unwrap().as_bool().unwrap());
}

// =============================================================================
// Test: Shell disabled - no hover for shell-only variables
// =============================================================================

#[tokio::test]
async fn test_disabled_shell_no_hover() {
    let fixture = TestFixture::new().await;

    // Add a shell-only variable (not in .env file)
    set_shell_var(&fixture, "SHELL_ONLY_TEST_VAR", "shell_test_value").await;

    // Create test file referencing the shell variable
    let uri = fixture.create_file("test.js", "process.env.SHELL_ONLY_TEST_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.SHELL_ONLY_TEST_VAR".to_string(),
            0,
        )
        .await;

    // Verify hover works initially (both sources enabled by default)
    let hover_before = get_hover(&fixture, &uri, 0, 20).await;
    assert!(
        hover_before.is_some(),
        "Hover should work before disabling shell"
    );
    assert!(
        format!("{:?}", hover_before.unwrap()).contains("shell_test_value"),
        "Hover should show shell value"
    );

    // Disable shell source (only File enabled)
    set_precedence(&fixture, vec!["File"]).await;

    // Verify hover no longer works for shell-only variable
    let hover_after = get_hover(&fixture, &uri, 0, 20).await;
    assert!(
        hover_after.is_none(),
        "Hover should NOT work after disabling shell for shell-only variable"
    );

    // Cleanup
    remove_shell_var(&fixture, "SHELL_ONLY_TEST_VAR").await;
}

// =============================================================================
// Test: Shell disabled - no completion for shell-only variables
// =============================================================================

#[tokio::test]
async fn test_disabled_shell_no_completion() {
    let fixture = TestFixture::new().await;

    // Add a shell-only variable
    set_shell_var(&fixture, "SHELL_COMPLETION_VAR", "completion_value").await;

    // Create test file with incomplete env access
    let uri = fixture.create_file("test.js", "process.env.");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.".to_string(),
            0,
        )
        .await;

    // Verify completion includes shell variable initially
    let completions_before = get_completions(&fixture, &uri, 0, 12).await;
    assert!(completions_before.is_some());
    let comp_str_before = format!("{:?}", completions_before.unwrap());
    assert!(
        comp_str_before.contains("SHELL_COMPLETION_VAR"),
        "Completion should include shell var before disabling"
    );

    // Disable shell source
    set_precedence(&fixture, vec!["File"]).await;

    // Verify completion no longer includes shell variable
    let completions_after = get_completions(&fixture, &uri, 0, 12).await;
    if let Some(comp) = completions_after {
        let comp_str_after = format!("{:?}", comp);
        assert!(
            !comp_str_after.contains("SHELL_COMPLETION_VAR"),
            "Completion should NOT include shell var after disabling: {}",
            comp_str_after
        );
    }

    // Cleanup
    remove_shell_var(&fixture, "SHELL_COMPLETION_VAR").await;
}

// =============================================================================
// Test: Shell disabled - diagnostics show undefined for shell-only vars
// =============================================================================

#[tokio::test]
async fn test_disabled_shell_undefined_diagnostic() {
    let fixture = TestFixture::new().await;

    // Add a shell-only variable
    set_shell_var(&fixture, "SHELL_DIAG_VAR", "diag_value").await;

    // Create test file
    let uri = fixture.create_file("test.js", "const x = process.env.SHELL_DIAG_VAR;");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "const x = process.env.SHELL_DIAG_VAR;".to_string(),
            0,
        )
        .await;

    // Verify no diagnostics initially
    let diags_before = compute_diagnostics(&uri, &fixture.state).await;
    let undefined_before = diags_before
        .iter()
        .any(|d| d.message.contains("SHELL_DIAG_VAR") && d.message.contains("not defined"));
    assert!(
        !undefined_before,
        "Should NOT have undefined diagnostic before disabling shell"
    );

    // Disable shell source
    set_precedence(&fixture, vec!["File"]).await;

    // Verify diagnostic shows undefined
    let diags_after = compute_diagnostics(&uri, &fixture.state).await;
    let undefined_after = diags_after
        .iter()
        .any(|d| d.message.contains("SHELL_DIAG_VAR") && d.message.contains("not defined"));
    assert!(
        undefined_after,
        "Should have undefined diagnostic after disabling shell"
    );

    // Cleanup
    remove_shell_var(&fixture, "SHELL_DIAG_VAR").await;
}

// =============================================================================
// Test: File disabled - no hover for file-only variables
// =============================================================================

#[tokio::test]
async fn test_disabled_file_no_hover() {
    let fixture = TestFixture::new().await;

    // DB_URL is in the .env file (created by TestFixture)
    // Create test file referencing file variable
    let uri = fixture.create_file("test.js", "process.env.DB_URL");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.DB_URL".to_string(),
            0,
        )
        .await;

    // Verify hover works initially
    let hover_before = get_hover(&fixture, &uri, 0, 14).await;
    assert!(hover_before.is_some(), "Hover should work before disabling file");
    assert!(
        format!("{:?}", hover_before.unwrap()).contains("postgres://"),
        "Hover should show file value"
    );

    // Disable file source (only Shell enabled)
    set_precedence(&fixture, vec!["Shell"]).await;

    // Verify hover no longer works for file-only variable
    let hover_after = get_hover(&fixture, &uri, 0, 14).await;
    assert!(
        hover_after.is_none(),
        "Hover should NOT work after disabling file for file-only variable"
    );
}

// =============================================================================
// Test: Re-enabling sources restores functionality
// =============================================================================

#[tokio::test]
async fn test_enable_restores_functionality() {
    let fixture = TestFixture::new().await;

    // Add shell variable
    set_shell_var(&fixture, "RESTORE_TEST_VAR", "restore_value").await;

    let uri = fixture.create_file("test.js", "process.env.RESTORE_TEST_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.RESTORE_TEST_VAR".to_string(),
            0,
        )
        .await;

    // Verify hover works initially
    let hover1 = get_hover(&fixture, &uri, 0, 20).await;
    assert!(hover1.is_some(), "Hover should work initially");

    // Disable shell
    set_precedence(&fixture, vec!["File"]).await;

    // Verify hover doesn't work
    let hover2 = get_hover(&fixture, &uri, 0, 20).await;
    assert!(hover2.is_none(), "Hover should NOT work after disabling shell");

    // Re-enable shell
    set_precedence(&fixture, vec!["Shell", "File"]).await;

    // Verify hover works again
    let hover3 = get_hover(&fixture, &uri, 0, 20).await;
    assert!(hover3.is_some(), "Hover should work after re-enabling shell");

    // Cleanup
    remove_shell_var(&fixture, "RESTORE_TEST_VAR").await;
}

// =============================================================================
// Test: Empty precedence allows all sources (fallback behavior)
// =============================================================================

#[tokio::test]
async fn test_empty_precedence_allows_all() {
    let fixture = TestFixture::new().await;

    // Add shell variable
    set_shell_var(&fixture, "EMPTY_PREC_VAR", "empty_prec_value").await;

    let uri = fixture.create_file("test.js", "process.env.EMPTY_PREC_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.EMPTY_PREC_VAR".to_string(),
            0,
        )
        .await;

    // Set empty precedence (should allow all sources)
    let params = ExecuteCommandParams {
        command: "ecolog.source.setPrecedence".to_string(),
        arguments: vec![], // Empty
        work_done_progress_params: Default::default(),
    };
    handle_execute_command(params, &fixture.state).await;

    // Hover should work (all sources enabled by default when empty)
    let hover = get_hover(&fixture, &uri, 0, 20).await;
    assert!(
        hover.is_some(),
        "Hover should work with empty precedence (all sources enabled)"
    );

    // Cleanup
    remove_shell_var(&fixture, "EMPTY_PREC_VAR").await;
}

// =============================================================================
// Test: Precedence persists after core refresh
// =============================================================================

#[tokio::test]
async fn test_precedence_persists_after_refresh() {
    let fixture = TestFixture::new().await;

    // Add shell variable
    set_shell_var(&fixture, "PERSIST_TEST_VAR", "persist_value").await;

    let uri = fixture.create_file("test.js", "process.env.PERSIST_TEST_VAR");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.PERSIST_TEST_VAR".to_string(),
            0,
        )
        .await;

    // Disable shell
    set_precedence(&fixture, vec!["File"]).await;

    // Verify shell is disabled
    let hover1 = get_hover(&fixture, &uri, 0, 20).await;
    assert!(hover1.is_none(), "Hover should NOT work after disabling shell");

    // Trigger a refresh (simulating file change)
    fixture.state.core.refresh().await.expect("Refresh failed");

    // Verify shell is still disabled after refresh
    let hover2 = get_hover(&fixture, &uri, 0, 20).await;
    assert!(
        hover2.is_none(),
        "Hover should still NOT work after refresh (precedence should persist)"
    );

    // Cleanup
    remove_shell_var(&fixture, "PERSIST_TEST_VAR").await;
}

// =============================================================================
// Test: File variables still work when shell is disabled
// =============================================================================

#[tokio::test]
async fn test_file_works_when_shell_disabled() {
    let fixture = TestFixture::new().await;

    // DB_URL is from the .env file
    let uri = fixture.create_file("test.js", "process.env.DB_URL");
    fixture
        .state
        .document_manager
        .open(
            uri.clone(),
            "javascript".to_string(),
            "process.env.DB_URL".to_string(),
            0,
        )
        .await;

    // Disable shell (File still enabled)
    set_precedence(&fixture, vec!["File"]).await;

    // File variable should still work
    let hover = get_hover(&fixture, &uri, 0, 14).await;
    assert!(
        hover.is_some(),
        "File variable should still work when shell is disabled"
    );
    assert!(
        format!("{:?}", hover.unwrap()).contains("postgres://"),
        "Hover should show file value"
    );
}
