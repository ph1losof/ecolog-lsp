;; ═════════════════════════════════════════════════════════════════════════
;; PHP Environment Variable Binding Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; $x = $_ENV['VAR']
;; ───────────────────────────────────────────────────────────────────────────
(assignment_expression
  left: (variable_name) @binding_name
  right: (subscript_expression
    (variable_name) @_var
    (string
      (string_content) @bound_env_var))
  (#eq? @_var "$_ENV")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; $x = $_SERVER['VAR']
;; ───────────────────────────────────────────────────────────────────────────
(assignment_expression
  left: (variable_name) @binding_name
  right: (subscript_expression
    (variable_name) @_var
    (string
      (string_content) @bound_env_var))
  (#eq? @_var "$_SERVER")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; $x = getenv('VAR')
;; ───────────────────────────────────────────────────────────────────────────
(assignment_expression
  left: (variable_name) @binding_name
  right: (function_call_expression
    function: (name) @_func
    arguments: (arguments
      (argument
        (string
          (string_content) @bound_env_var))))
  (#eq? @_func "getenv")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; $x = env('VAR') - Laravel
;; ───────────────────────────────────────────────────────────────────────────
(assignment_expression
  left: (variable_name) @binding_name
  right: (function_call_expression
    function: (name) @_func
    arguments: (arguments
      (argument
        (string
          (string_content) @bound_env_var))))
  (#eq? @_func "env")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; $env = $_ENV (object alias)
;; ───────────────────────────────────────────────────────────────────────────
(assignment_expression
  left: (variable_name) @binding_name
  right: (variable_name) @_var
  (#eq? @_var "$_ENV")) @env_object_binding

;; ───────────────────────────────────────────────────────────────────────────
;; $server = $_SERVER (object alias)
;; ───────────────────────────────────────────────────────────────────────────
(assignment_expression
  left: (variable_name) @binding_name
  right: (variable_name) @_var
  (#eq? @_var "$_SERVER")) @env_object_binding
