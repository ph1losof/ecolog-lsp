;; ═════════════════════════════════════════════════════════════════════════
;; Kotlin Completion Context Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; System.getenv(" - trigger completion inside getenv call
;; ───────────────────────────────────────────────────────────────────────────
(call_expression
  (navigation_expression
    (identifier) @object
    (identifier) @_method)
  (value_arguments
    (value_argument
      (string_literal) @completion_target))
  (#eq? @object "System")
  (#eq? @_method "getenv")) @completion_call
