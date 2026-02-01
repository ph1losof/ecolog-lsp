;; ═════════════════════════════════════════════════════════════════════════
;; PHP Environment Variable Completion Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; $_ENV['[cursor]'] - cursor inside string
;; ───────────────────────────────────────────────────────────────────────────
(subscript_expression
  (variable_name) @object
  (string) @completion_target
  (#any-of? @object "$_ENV" "$_SERVER"))

;; ───────────────────────────────────────────────────────────────────────────
;; getenv('[cursor]') - function call with string
;; ───────────────────────────────────────────────────────────────────────────
(function_call_expression
  function: (name) @object
  arguments: (arguments
    (argument
      (string) @completion_target))
  (#any-of? @object "getenv" "env" "config"))

;; ───────────────────────────────────────────────────────────────────────────
;; getenv( - incomplete call, no string yet
;; ───────────────────────────────────────────────────────────────────────────
(function_call_expression
  function: (name) @object
  arguments: (arguments) @completion_target
  (#any-of? @object "getenv" "env" "config"))

;; ───────────────────────────────────────────────────────────────────────────
;; ERROR nodes - incomplete input during typing
;; ───────────────────────────────────────────────────────────────────────────
(ERROR) @completion_target
