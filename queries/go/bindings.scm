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

;; ═════════════════════════════════════════════════════════════════════════
;; Struct Field Initialization Patterns
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; Config{DB: os.Getenv("DB")}
;; ───────────────────────────────────────────────────────────────────────────
(composite_literal
  body: (literal_value
    (keyed_element
      (literal_element
        (identifier) @binding_name)
      (literal_element
        (call_expression
          function: (selector_expression
            operand: (identifier) @_module
            field: (field_identifier) @_fn)
          arguments: (argument_list
            (interpreted_string_literal) @bound_env_var
            (_)?)
          (#eq? @_module "os")
          (#any-of? @_fn "Getenv" "LookupEnv")))))) @env_binding

;; ═════════════════════════════════════════════════════════════════════════
;; Multiple Assignment Patterns
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; x, ok := os.LookupEnv("VAR") - with ok check
;; ───────────────────────────────────────────────────────────────────────────
(short_var_declaration
  left: (expression_list
    (identifier) @binding_name
    (identifier))
  right: (expression_list
    (call_expression
      function: (selector_expression
        operand: (identifier) @_module
        field: (field_identifier) @_fn)
      arguments: (argument_list
        (interpreted_string_literal) @bound_env_var
        (_)?)
      (#eq? @_module "os")
      (#eq? @_fn "LookupEnv")))
) @env_binding

;; ═════════════════════════════════════════════════════════════════════════
;; Conditional Binding (if statement init)
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; if val, ok := os.LookupEnv("VAR"); ok { ... }
;; ───────────────────────────────────────────────────────────────────────────
(if_statement
  initializer: (short_var_declaration
    left: (expression_list
      (identifier) @binding_name
      (identifier))
    right: (expression_list
      (call_expression
        function: (selector_expression
          operand: (identifier) @_module
          field: (field_identifier) @_fn)
        arguments: (argument_list
          (interpreted_string_literal) @bound_env_var
          (_)?)
        (#eq? @_module "os")
        (#eq? @_fn "LookupEnv"))))) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; if val := os.Getenv("VAR"); val != "" { ... }
;; ───────────────────────────────────────────────────────────────────────────
(if_statement
  initializer: (short_var_declaration
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
        (#eq? @_fn "Getenv"))))) @env_binding

;; ═════════════════════════════════════════════════════════════════════════
;; viper bindings (popular config library)
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; x := viper.GetString("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(short_var_declaration
  left: (expression_list
    (identifier) @binding_name)
  right: (expression_list
    (call_expression
      function: (selector_expression
        operand: (identifier) @_pkg
        field: (field_identifier) @_fn)
      arguments: (argument_list
        (interpreted_string_literal) @bound_env_var
        (_)?)
      (#eq? @_pkg "viper")
      (#any-of? @_fn "GetString" "GetInt" "GetBool" "GetFloat64" "GetDuration" "Get")))
) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; var x = viper.GetString("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(var_declaration
  (var_spec
    name: (identifier) @binding_name
    value: (expression_list
      (call_expression
        function: (selector_expression
          operand: (identifier) @_pkg
          field: (field_identifier) @_fn)
        arguments: (argument_list
          (interpreted_string_literal) @bound_env_var
          (_)?)
        (#eq? @_pkg "viper")
        (#any-of? @_fn "GetString" "GetInt" "GetBool" "GetFloat64" "GetDuration" "Get")))
  )
) @env_binding
