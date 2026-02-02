;; ═════════════════════════════════════════════════════════════════════════
;; Java Completion Context Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; System.getenv(" - trigger completion inside getenv call
;; ───────────────────────────────────────────────────────────────────────────
(method_invocation
  object: (identifier) @object
  name: (identifier) @_method
  arguments: (argument_list
    (string_literal) @completion_target)
  (#eq? @object "System")
  (#eq? @_method "getenv")) @completion_call
