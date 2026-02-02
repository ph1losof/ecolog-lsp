;; ═════════════════════════════════════════════════════════════════════════
;; C++ Completion Context Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; getenv(" - trigger completion inside getenv call
;; ───────────────────────────────────────────────────────────────────────────
(call_expression
  function: (identifier) @object
  arguments: (argument_list
    (string_literal) @completion_target)
  (#any-of? @object "getenv" "secure_getenv")) @completion_call

;; ───────────────────────────────────────────────────────────────────────────
;; std::getenv(" - trigger completion inside std::getenv call
;; ───────────────────────────────────────────────────────────────────────────
(call_expression
  function: (qualified_identifier
    scope: (namespace_identifier) @object
    name: (identifier) @_func)
  arguments: (argument_list
    (string_literal) @completion_target)
  (#eq? @object "std")
  (#eq? @_func "getenv")) @completion_call
