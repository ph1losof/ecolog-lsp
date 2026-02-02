;; ═════════════════════════════════════════════════════════════════════════
;; C# Completion Context Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; Environment.GetEnvironmentVariable(" - trigger completion
;; ───────────────────────────────────────────────────────────────────────────
(invocation_expression
  function: (member_access_expression
    expression: (identifier) @object
    name: (identifier) @_method)
  arguments: (argument_list
    (argument
      (string_literal) @completion_target))
  (#eq? @object "Environment")
  (#eq? @_method "GetEnvironmentVariable")) @completion_call
