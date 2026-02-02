;; ═════════════════════════════════════════════════════════════════════════
;; Elixir Environment Variable Reference Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; System.get_env("VAR")
;; System.fetch_env("VAR")
;; System.fetch_env!("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(call
  target: (dot
    left: (alias) @_obj
    right: (identifier) @_func)
  (arguments
    (string
      (quoted_content) @env_var_name))
  (#eq? @_obj "System")
  (#any-of? @_func "get_env" "fetch_env" "fetch_env!")) @env_access
