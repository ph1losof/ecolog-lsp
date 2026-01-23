;; ═════════════════════════════════════════════════════════════════════════
;; Rust Environment Variable Binding Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; let x = env::var("VAR")
;; let x = std::env::var("VAR")
;; let x = env!("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(let_declaration
  pattern: (identifier) @binding_name
  value: [
    ;; let x = env::var("VAR")
    (call_expression
      function: (scoped_identifier
        path: [(scoped_identifier
          path: (identifier) @_module
          name: (identifier) @_path)
        (identifier) @_path]
        name: (identifier) @_fn)
      arguments: (arguments
        (string_literal
          (string_content) @bound_env_var)
        (_)?)
      (#match? @_path "(std::)?env")
      (#any-of? @_fn "var" "var_os"))
    ;; let x = env!("VAR")
    (macro_invocation
      macro: (identifier) @_macro
      (token_tree
        (string_literal
          (string_content) @bound_env_var)
        (_)?)
      (#any-of? @_macro "env" "option_env"))
  ]) @env_binding

;; ═════════════════════════════════════════════════════════════════════════
;; Result/Option Destructuring Patterns
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; let Ok(val) = std::env::var("VAR")
;; AST: scoped_identifier(path: scoped_identifier(path: "std", name: "env"), name: "var")
;; ───────────────────────────────────────────────────────────────────────────
(let_declaration
  pattern: (tuple_struct_pattern
    type: (identifier) @_variant
    (identifier) @binding_name)
  value: (call_expression
    function: (scoped_identifier
      path: (scoped_identifier
        path: (identifier) @_std
        name: (identifier) @_env)
      name: (identifier) @_fn)
    arguments: (arguments
      (string_literal
        (string_content) @bound_env_var)))
  (#eq? @_std "std")
  (#eq? @_env "env")
  (#any-of? @_variant "Ok" "Some")
  (#any-of? @_fn "var" "var_os")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; let Ok(val) = env::var("VAR")
;; AST: scoped_identifier(path: "env", name: "var")
;; ───────────────────────────────────────────────────────────────────────────
(let_declaration
  pattern: (tuple_struct_pattern
    type: (identifier) @_variant
    (identifier) @binding_name)
  value: (call_expression
    function: (scoped_identifier
      path: (identifier) @_env
      name: (identifier) @_fn)
    arguments: (arguments
      (string_literal
        (string_content) @bound_env_var)))
  (#eq? @_env "env")
  (#any-of? @_variant "Ok" "Some")
  (#any-of? @_fn "var" "var_os")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; let Some(val) = std::env::var("VAR").ok()
;; AST: call_expression(field_expression(call_expression(scoped_identifier...)))
;; The outer call_expression is .ok(), inner is std::env::var
;; ───────────────────────────────────────────────────────────────────────────
(let_declaration
  pattern: (tuple_struct_pattern
    type: (identifier) @_variant
    (identifier) @binding_name)
  value: (call_expression
    function: (field_expression
      value: (call_expression
        function: (scoped_identifier
          path: (scoped_identifier
            path: (identifier) @_std
            name: (identifier) @_env)
          name: (identifier) @_fn)
        arguments: (arguments
          (string_literal
            (string_content) @bound_env_var)))))
  (#eq? @_std "std")
  (#eq? @_env "env")
  (#any-of? @_variant "Some")
  (#any-of? @_fn "var" "var_os")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; let Some(val) = env::var("VAR").ok()
;; Short path version
;; ───────────────────────────────────────────────────────────────────────────
(let_declaration
  pattern: (tuple_struct_pattern
    type: (identifier) @_variant
    (identifier) @binding_name)
  value: (call_expression
    function: (field_expression
      value: (call_expression
        function: (scoped_identifier
          path: (identifier) @_env
          name: (identifier) @_fn)
        arguments: (arguments
          (string_literal
            (string_content) @bound_env_var)))))
  (#eq? @_env "env")
  (#any-of? @_variant "Some")
  (#any-of? @_fn "var" "var_os")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; if let Ok(val) = std::env::var("VAR") { ... }
;; ───────────────────────────────────────────────────────────────────────────
(if_expression
  condition: (let_condition
    pattern: (tuple_struct_pattern
      type: (identifier) @_variant
      (identifier) @binding_name)
    value: (call_expression
      function: (scoped_identifier
        path: (scoped_identifier
          path: (identifier) @_std
          name: (identifier) @_env)
        name: (identifier) @_fn)
      arguments: (arguments
        (string_literal
          (string_content) @bound_env_var))))
  (#eq? @_std "std")
  (#eq? @_env "env")
  (#any-of? @_variant "Ok" "Some")
  (#any-of? @_fn "var" "var_os")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; if let Ok(val) = env::var("VAR") { ... }
;; ───────────────────────────────────────────────────────────────────────────
(if_expression
  condition: (let_condition
    pattern: (tuple_struct_pattern
      type: (identifier) @_variant
      (identifier) @binding_name)
    value: (call_expression
      function: (scoped_identifier
        path: (identifier) @_env
        name: (identifier) @_fn)
      arguments: (arguments
        (string_literal
          (string_content) @bound_env_var))))
  (#eq? @_env "env")
  (#any-of? @_variant "Ok" "Some")
  (#any-of? @_fn "var" "var_os")) @env_binding

;; ═════════════════════════════════════════════════════════════════════════
;; Match Arm Destructuring Patterns
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; match std::env::var("VAR") { Ok(val) => ... }
;; ───────────────────────────────────────────────────────────────────────────
(match_expression
  value: (call_expression
    function: (scoped_identifier
      path: (scoped_identifier
        path: (identifier) @_std
        name: (identifier) @_env)
      name: (identifier) @_fn)
    arguments: (arguments
      (string_literal
        (string_content) @bound_env_var)))
  body: (match_block
    (match_arm
      pattern: (match_pattern
        (tuple_struct_pattern
          type: (identifier) @_variant
          (identifier) @binding_name))))
  (#eq? @_std "std")
  (#eq? @_env "env")
  (#any-of? @_variant "Ok" "Some")
  (#any-of? @_fn "var" "var_os")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; match env::var("VAR") { Ok(val) => ... }
;; ───────────────────────────────────────────────────────────────────────────
(match_expression
  value: (call_expression
    function: (scoped_identifier
      path: (identifier) @_env
      name: (identifier) @_fn)
    arguments: (arguments
      (string_literal
        (string_content) @bound_env_var)))
  body: (match_block
    (match_arm
      pattern: (match_pattern
        (tuple_struct_pattern
          type: (identifier) @_variant
          (identifier) @binding_name))))
  (#eq? @_env "env")
  (#any-of? @_variant "Ok" "Some")
  (#any-of? @_fn "var" "var_os")) @env_binding

;; ═════════════════════════════════════════════════════════════════════════
;; const/static items with env!()
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; const DB: &str = env!("DATABASE_URL");
;; ───────────────────────────────────────────────────────────────────────────
(const_item
  name: (identifier) @binding_name
  value: (macro_invocation
    macro: (identifier) @_macro
    (token_tree
      (string_literal
        (string_content) @bound_env_var)
      (_)?))
  (#any-of? @_macro "env" "option_env")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; static DB: &str = env!("DATABASE_URL");
;; ───────────────────────────────────────────────────────────────────────────
(static_item
  name: (identifier) @binding_name
  value: (macro_invocation
    macro: (identifier) @_macro
    (token_tree
      (string_literal
        (string_content) @bound_env_var)
      (_)?))
  (#any-of? @_macro "env" "option_env")) @env_binding

;; Note: Struct field initialization patterns (Config { db: env::var("DB") })
;; are detected via the reference queries rather than binding queries,
;; as the env var reference within the field value is captured directly.

;; ═════════════════════════════════════════════════════════════════════════
;; dotenv / dotenvy crate bindings
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; let x = dotenv::var("VAR") / dotenvy::var("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(let_declaration
  pattern: (identifier) @binding_name
  value: (call_expression
    function: (scoped_identifier
      path: (identifier) @_crate
      name: (identifier) @_fn)
    arguments: (arguments
      (string_literal
        (string_content) @bound_env_var)
      (_)?))
  (#any-of? @_crate "dotenv" "dotenvy")
  (#eq? @_fn "var")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; let x = dotenv::var("VAR")? / let x = dotenvy::var("VAR")?
;; ───────────────────────────────────────────────────────────────────────────
(let_declaration
  pattern: (identifier) @binding_name
  value: (try_expression
    (call_expression
      function: (scoped_identifier
        path: (identifier) @_crate
        name: (identifier) @_fn)
      arguments: (arguments
        (string_literal
          (string_content) @bound_env_var)
        (_)?)))
  (#any-of? @_crate "dotenv" "dotenvy")
  (#eq? @_fn "var")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; let x = env::var("VAR")? - try expression
;; ───────────────────────────────────────────────────────────────────────────
(let_declaration
  pattern: (identifier) @binding_name
  value: (try_expression
    (call_expression
      function: (scoped_identifier
        path: [(scoped_identifier
          path: (identifier)
          name: (identifier) @_path)
        (identifier) @_path]
        name: (identifier) @_fn)
      arguments: (arguments
        (string_literal
          (string_content) @bound_env_var)
        (_)?)))
  (#any-of? @_path "env" "std")
  (#any-of? @_fn "var" "var_os")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; let x = env::var("VAR").unwrap() - method chain
;; ───────────────────────────────────────────────────────────────────────────
(let_declaration
  pattern: (identifier) @binding_name
  value: (call_expression
    function: (field_expression
      value: (call_expression
        function: (scoped_identifier
          path: [(scoped_identifier
            path: (identifier)
            name: (identifier) @_path)
          (identifier) @_path]
          name: (identifier) @_fn)
        arguments: (arguments
          (string_literal
            (string_content) @bound_env_var)
          (_)?))
      field: (field_identifier) @_method))
  (#any-of? @_path "env" "std")
  (#any-of? @_fn "var" "var_os")
  (#any-of? @_method "unwrap" "unwrap_or" "unwrap_or_else" "unwrap_or_default" "expect")) @env_binding
