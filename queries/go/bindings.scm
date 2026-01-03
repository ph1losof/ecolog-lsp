;; ═══════════════════════════════════════════════════════════════════════════
;; Go Environment Variable Binding Queries
;; ═══════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; x := os.Getenv("VAR")
;; x, ok := os.LookupEnv("VAR")
;; var x = os.Getenv("VAR")
;; ───────────────────────────────────────────────────────────────────────────
;; x := os.Getenv("VAR")
(short_var_declaration
  left: (expression_list
    (identifier) @binding_name)
  right: (expression_list
    (call_expression
      function: (selector_expression
        operand: (identifier) @_module
        field: (field_identifier) @_fn)
      arguments: (argument_list
        (interpreted_string_literal) @bound_env_var
        (_)?)
      (#eq? @_module "os")
      (#any-of? @_fn "Getenv" "LookupEnv")))
) @env_binding

;; var x = os.Getenv("VAR")
(var_declaration
  (var_spec
    name: (identifier) @binding_name
    value: (expression_list
      (call_expression
        function: (selector_expression
          operand: (identifier) @_module
          field: (field_identifier) @_fn)
        arguments: (argument_list
          (interpreted_string_literal) @bound_env_var
          (_)?)
        (#eq? @_module "os")
        (#any-of? @_fn "Getenv" "LookupEnv")))
  )
) @env_binding
