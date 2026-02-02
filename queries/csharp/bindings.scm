;; ═════════════════════════════════════════════════════════════════════════
;; C# Environment Variable Binding Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; var x = Environment.GetEnvironmentVariable("VAR");
;; string x = Environment.GetEnvironmentVariable("VAR");
;; ───────────────────────────────────────────────────────────────────────────
(local_declaration_statement
  (variable_declaration
    (variable_declarator
      (identifier) @binding_name
      (equals_value_clause
        (invocation_expression
          function: (member_access_expression
            expression: (identifier) @_obj
            name: (identifier) @_method)
          arguments: (argument_list
            (argument
              (string_literal
                (string_literal_fragment) @bound_env_var)))))))
  (#eq? @_obj "Environment")
  (#eq? @_method "GetEnvironmentVariable")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; Field declaration: private string _x = Environment.GetEnvironmentVariable("VAR");
;; ───────────────────────────────────────────────────────────────────────────
(field_declaration
  (variable_declaration
    (variable_declarator
      (identifier) @binding_name
      (equals_value_clause
        (invocation_expression
          function: (member_access_expression
            expression: (identifier) @_obj
            name: (identifier) @_method)
          arguments: (argument_list
            (argument
              (string_literal
                (string_literal_fragment) @bound_env_var)))))))
  (#eq? @_obj "Environment")
  (#eq? @_method "GetEnvironmentVariable")) @env_binding
