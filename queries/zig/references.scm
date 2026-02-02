;; ═════════════════════════════════════════════════════════════════════════
;; Zig Environment Variable Reference Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; std.os.getenv("VAR") or std.posix.getenv("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(call_expression
  (field_expression
    (field_expression
      (identifier) @_root
      (identifier) @_module)
    (identifier) @_func)
  (string
    (string_content) @env_var_name)
  (#eq? @_root "std")
  (#any-of? @_module "os" "posix")
  (#eq? @_func "getenv")) @env_access
