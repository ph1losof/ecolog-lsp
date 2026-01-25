;; ═════════════════════════════════════════════════════════════════════════
;; Lua Environment Variable Binding Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; These patterns capture variable bindings to environment variables.
;; Example: local db_url = os.getenv("DATABASE_URL")

;; ───────────────────────────────────────────────────────────────────────────
;; local x = os.getenv("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(variable_declaration
  (assignment_statement
    (variable_list
      name: (identifier) @binding_name)
    (expression_list
      value: (function_call
        name: (dot_index_expression
          table: (identifier) @_module
          field: (identifier) @_func)
        arguments: (arguments
          (string
            content: (string_content) @bound_env_var)))))
  (#eq? @_module "os")
  (#eq? @_func "getenv")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; x = os.getenv("VAR") (global assignment)
;; ───────────────────────────────────────────────────────────────────────────
(assignment_statement
  (variable_list
    name: (identifier) @binding_name)
  (expression_list
    value: (function_call
      name: (dot_index_expression
        table: (identifier) @_module
        field: (identifier) @_func)
      arguments: (arguments
        (string
          content: (string_content) @bound_env_var))))
  (#eq? @_module "os")
  (#eq? @_func "getenv")) @env_binding
