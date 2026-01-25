;; ═════════════════════════════════════════════════════════════════════════
;; Lua Environment Variable Completion Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; These patterns identify locations where environment variable
;; completion should be triggered.

;; ───────────────────────────────────────────────────────────────────────────
;; os.getenv("[cursor]") - cursor inside string
;; ───────────────────────────────────────────────────────────────────────────
(function_call
  name: (dot_index_expression
    table: (identifier) @object
    field: (identifier) @_func)
  arguments: (arguments
    (string) @completion_target)
  (#eq? @object "os")
  (#eq? @_func "getenv"))

;; ───────────────────────────────────────────────────────────────────────────
;; os.getenv( - incomplete call, no string yet
;; ───────────────────────────────────────────────────────────────────────────
(function_call
  name: (dot_index_expression
    table: (identifier) @object
    field: (identifier) @_func)
  arguments: (arguments) @completion_target
  (#eq? @object "os")
  (#eq? @_func "getenv"))

;; ───────────────────────────────────────────────────────────────────────────
;; ERROR nodes - incomplete input during typing
;; ───────────────────────────────────────────────────────────────────────────
(ERROR) @completion_target
