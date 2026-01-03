;; ═════════════════════════════════════════════════════════════════════════
;; Go Environment Variable Reference Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; os.Getenv("VAR")
;; os.LookupEnv("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(call_expression
  function: (selector_expression
    operand: (identifier) @object
    field: (field_identifier) @_fn)
  arguments: (argument_list
    (interpreted_string_literal) @env_var_name
    (_)?)
  (#any-of? @_fn "Getenv" "LookupEnv" "Setenv" "Unsetenv")) @env_access
