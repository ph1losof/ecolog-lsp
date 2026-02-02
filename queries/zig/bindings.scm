;; ═════════════════════════════════════════════════════════════════════════
;; Zig Environment Variable Binding Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; const x = std.os.getenv("VAR");
;; var x = std.posix.getenv("VAR");
;; ───────────────────────────────────────────────────────────────────────────
(variable_declaration
  (identifier) @binding_name
  (call_expression
    (field_expression
      (field_expression
        (identifier) @_root
        (identifier) @_module)
      (identifier) @_func)
    (string
      (string_content) @bound_env_var))
  (#eq? @_root "std")
  (#any-of? @_module "os" "posix")
  (#eq? @_func "getenv")) @env_binding
