;; ═════════════════════════════════════════════════════════════════════════
;; PHP Environment Variable Reference Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; Primary patterns: $_ENV['VAR'], $_SERVER['VAR'], getenv('VAR'), env('VAR')

;; ───────────────────────────────────────────────────────────────────────────
;; $_ENV['VAR']
;; ───────────────────────────────────────────────────────────────────────────
(subscript_expression
  (variable_name) @_var
  (string
    (string_content) @env_var_name)
  (#eq? @_var "$_ENV")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; $_SERVER['VAR']
;; ───────────────────────────────────────────────────────────────────────────
(subscript_expression
  (variable_name) @_var
  (string
    (string_content) @env_var_name)
  (#eq? @_var "$_SERVER")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; getenv('VAR')
;; ───────────────────────────────────────────────────────────────────────────
(function_call_expression
  function: (name) @_func
  arguments: (arguments
    (argument
      (string
        (string_content) @env_var_name)))
  (#eq? @_func "getenv")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; env('VAR') - Laravel helper
;; ───────────────────────────────────────────────────────────────────────────
(function_call_expression
  function: (name) @_func
  arguments: (arguments
    (argument
      (string
        (string_content) @env_var_name)))
  (#eq? @_func "env")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; config('VAR') - Laravel config (can reference env vars)
;; ───────────────────────────────────────────────────────────────────────────
(function_call_expression
  function: (name) @_func
  arguments: (arguments
    (argument
      (string
        (string_content) @env_var_name)))
  (#eq? @_func "config")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; Env::get('VAR') - Dotenv static call
;; ───────────────────────────────────────────────────────────────────────────
(scoped_call_expression
  scope: (name) @_class
  name: (name) @_method
  arguments: (arguments
    (argument
      (string
        (string_content) @env_var_name)))
  (#eq? @_class "Env")
  (#eq? @_method "get")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; Dotenv::get('VAR')
;; ───────────────────────────────────────────────────────────────────────────
(scoped_call_expression
  scope: (name) @_class
  name: (name) @_method
  arguments: (arguments
    (argument
      (string
        (string_content) @env_var_name)))
  (#eq? @_class "Dotenv")
  (#eq? @_method "get")) @env_access
