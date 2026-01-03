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
