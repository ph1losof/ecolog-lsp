;; ═════════════════════════════════════════════════════════════════════════
;; Java Environment Variable Binding Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; String x = System.getenv("VAR");
;; ───────────────────────────────────────────────────────────────────────────
(local_variable_declaration
  declarator: (variable_declarator
    name: (identifier) @binding_name
    value: (method_invocation
      object: (identifier) @_obj
      name: (identifier) @_method
      arguments: (argument_list
        (string_literal
          (string_fragment) @bound_env_var))))
  (#eq? @_obj "System")
  (#eq? @_method "getenv")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; var x = System.getenv("VAR"); (Java 10+ local variable type inference)
;; ───────────────────────────────────────────────────────────────────────────
(local_variable_declaration
  type: (type_identifier) @_type
  declarator: (variable_declarator
    name: (identifier) @binding_name
    value: (method_invocation
      object: (identifier) @_obj
      name: (identifier) @_method
      arguments: (argument_list
        (string_literal
          (string_fragment) @bound_env_var))))
  (#eq? @_type "var")
  (#eq? @_obj "System")
  (#eq? @_method "getenv")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; Field declaration: private String x = System.getenv("VAR");
;; ───────────────────────────────────────────────────────────────────────────
(field_declaration
  declarator: (variable_declarator
    name: (identifier) @binding_name
    value: (method_invocation
      object: (identifier) @_obj
      name: (identifier) @_method
      arguments: (argument_list
        (string_literal
          (string_fragment) @bound_env_var))))
  (#eq? @_obj "System")
  (#eq? @_method "getenv")) @env_binding
