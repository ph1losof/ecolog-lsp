;; ═════════════════════════════════════════════════════════════════════════
;; Elixir Environment Variable Binding Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; x = System.get_env("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(binary_operator
  left: (identifier) @binding_name
  operator: "="
  right: (call
    target: (dot
      left: (alias) @_obj
      right: (identifier) @_func)
    (arguments
      (string
        (quoted_content) @bound_env_var)))
  (#eq? @_obj "System")
  (#any-of? @_func "get_env" "fetch_env" "fetch_env!")) @env_binding
