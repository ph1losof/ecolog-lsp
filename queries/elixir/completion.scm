;; ═════════════════════════════════════════════════════════════════════════
;; Elixir Completion Context Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; System.get_env(" - trigger completion
;; ───────────────────────────────────────────────────────────────────────────
(call
  target: (dot
    left: (alias) @object
    right: (identifier) @_func)
  (arguments
    (string) @completion_target)
  (#eq? @object "System")
  (#any-of? @_func "get_env" "fetch_env" "fetch_env!")) @completion_call
