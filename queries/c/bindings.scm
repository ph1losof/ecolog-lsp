;; ═════════════════════════════════════════════════════════════════════════
;; C Environment Variable Binding Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; char* x = getenv("VAR");
;; const char* x = getenv("VAR");
;; ───────────────────────────────────────────────────────────────────────────
(declaration
  declarator: (init_declarator
    declarator: (pointer_declarator
      declarator: (identifier) @binding_name)
    value: (call_expression
      function: (identifier) @_func
      arguments: (argument_list
        (string_literal
          (string_content) @bound_env_var))))
  (#any-of? @_func "getenv" "secure_getenv")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; char* x = getenv("VAR"); (simple identifier declarator)
;; ───────────────────────────────────────────────────────────────────────────
(declaration
  declarator: (init_declarator
    declarator: (identifier) @binding_name
    value: (call_expression
      function: (identifier) @_func
      arguments: (argument_list
        (string_literal
          (string_content) @bound_env_var))))
  (#any-of? @_func "getenv" "secure_getenv")) @env_binding
