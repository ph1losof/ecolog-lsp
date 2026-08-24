;; ═════════════════════════════════════════════════════════════════════════
;; C# Environment Variable Binding Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; `variable_declarator` holds the initializer directly (there is no
;; `equals_value_clause` node), and string contents are `string_literal_content`.

;; ───────────────────────────────────────────────────────────────────────────
;; var x = Environment.GetEnvironmentVariable("VAR");
;; string x = Environment.GetEnvironmentVariable("VAR");
;; ───────────────────────────────────────────────────────────────────────────
(local_declaration_statement
  (variable_declaration
    (variable_declarator
      name: (identifier) @binding_name
      (invocation_expression
        function: (member_access_expression
          expression: (identifier) @_obj
          name: (identifier) @_method)
        arguments: (argument_list
          (argument
            (string_literal
              (string_literal_content) @bound_env_var))))))
  (#eq? @_obj "Environment")
  (#eq? @_method "GetEnvironmentVariable")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; Field declaration: private string _x = Environment.GetEnvironmentVariable("VAR");
;; ───────────────────────────────────────────────────────────────────────────
(field_declaration
  (variable_declaration
    (variable_declarator
      name: (identifier) @binding_name
      (invocation_expression
        function: (member_access_expression
          expression: (identifier) @_obj
          name: (identifier) @_method)
        arguments: (argument_list
          (argument
            (string_literal
              (string_literal_content) @bound_env_var))))))
  (#eq? @_obj "Environment")
  (#eq? @_method "GetEnvironmentVariable")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; Fully qualified form, matching what references.scm already recognises:
;;   var x = System.Environment.GetEnvironmentVariable("VAR");
;; ───────────────────────────────────────────────────────────────────────────
(local_declaration_statement
  (variable_declaration
    (variable_declarator
      name: (identifier) @binding_name
      (invocation_expression
        function: (member_access_expression
          expression: (member_access_expression
            expression: (identifier) @_ns
            name: (identifier) @_obj)
          name: (identifier) @_method)
        arguments: (argument_list
          (argument
            (string_literal
              (string_literal_content) @bound_env_var))))))
  (#eq? @_ns "System")
  (#eq? @_obj "Environment")
  (#eq? @_method "GetEnvironmentVariable")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; private string _x = System.Environment.GetEnvironmentVariable("VAR");
;; ───────────────────────────────────────────────────────────────────────────
(field_declaration
  (variable_declaration
    (variable_declarator
      name: (identifier) @binding_name
      (invocation_expression
        function: (member_access_expression
          expression: (member_access_expression
            expression: (identifier) @_ns
            name: (identifier) @_obj)
          name: (identifier) @_method)
        arguments: (argument_list
          (argument
            (string_literal
              (string_literal_content) @bound_env_var))))))
  (#eq? @_ns "System")
  (#eq? @_obj "Environment")
  (#eq? @_method "GetEnvironmentVariable")) @env_binding
