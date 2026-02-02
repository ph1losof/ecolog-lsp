;; ═════════════════════════════════════════════════════════════════════════
;; Zig Completion Context Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; std.os.getenv(" - trigger completion
;; ───────────────────────────────────────────────────────────────────────────
(call_expression
  (field_expression
    (field_expression
      (identifier) @object
      (identifier) @_module)
    (identifier) @_func)
  (string) @completion_target
  (#eq? @object "std")
  (#any-of? @_module "os" "posix")
  (#eq? @_func "getenv")) @completion_call
