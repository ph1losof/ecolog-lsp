;; ═════════════════════════════════════════════════════════════════════════
;; C Environment Variable Reference Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; getenv("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(call_expression
  function: (identifier) @_func
  arguments: (argument_list
    (string_literal
      (string_content) @env_var_name))
  (#any-of? @_func "getenv" "secure_getenv")) @env_access
