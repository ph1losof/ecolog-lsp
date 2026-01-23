;; ═════════════════════════════════════════════════════════════════════════
;; Rust Environment Variable Reference Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; std::env::var("VAR")
;; env::var("VAR")
;; var("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(call_expression
  function: (scoped_identifier
    path: [(scoped_identifier
      path: (identifier) @module
      name: (identifier) @_path)
    (identifier) @module]
    name: (identifier) @_fn)
  arguments: (arguments
    (string_literal
      (string_content) @env_var_name)
    (_)?)
  (#any-of? @_fn "var" "var_os")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; env!("VAR")
;; option_env!("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(macro_invocation
  macro: (identifier) @_macro
  (token_tree
    (string_literal
      (string_content) @env_var_name)
    (_)?)
  (#any-of? @_macro "env" "option_env")) @env_access

;; ═════════════════════════════════════════════════════════════════════════
;; dotenv / dotenvy crate patterns
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; dotenv::var("VAR") / dotenvy::var("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(call_expression
  function: (scoped_identifier
    path: (identifier) @_crate
    name: (identifier) @_fn)
  arguments: (arguments
    (string_literal
      (string_content) @env_var_name)
    (_)?)
  (#any-of? @_crate "dotenv" "dotenvy")
  (#eq? @_fn "var")) @env_access

;; Note: Method chain patterns (env::var("VAR").unwrap()) and try expressions
;; (env::var("VAR")?) are already matched by the base pattern above since
;; tree-sitter matches the inner call_expression. No need for separate patterns.
