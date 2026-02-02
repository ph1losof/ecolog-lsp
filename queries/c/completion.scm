;; ═════════════════════════════════════════════════════════════════════════
;; C Completion Context Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; getenv(" - trigger completion inside getenv call
;; ───────────────────────────────────────────────────────────────────────────
(call_expression
  function: (identifier) @object
  arguments: (argument_list
    (string_literal) @completion_target)
  (#any-of? @object "getenv" "secure_getenv")) @completion_call
