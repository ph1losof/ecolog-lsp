;; ═════════════════════════════════════════════════════════════════════════
;; Kotlin Environment Variable Reference Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; System.getenv("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(call_expression
  (navigation_expression
    (identifier) @_obj
    (identifier) @_method)
  (value_arguments
    (value_argument
      (string_literal
        (string_content) @env_var_name)))
  (#eq? @_obj "System")
  (#eq? @_method "getenv")) @env_access
