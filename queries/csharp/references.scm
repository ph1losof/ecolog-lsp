;; ═════════════════════════════════════════════════════════════════════════
;; C# Environment Variable Reference Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; Environment.GetEnvironmentVariable("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(invocation_expression
  function: (member_access_expression
    expression: (identifier) @_obj
    name: (identifier) @_method)
  arguments: (argument_list
    (argument
      (string_literal
        (string_literal_content) @env_var_name)))
  (#eq? @_obj "Environment")
  (#eq? @_method "GetEnvironmentVariable")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; System.Environment.GetEnvironmentVariable("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(invocation_expression
  function: (member_access_expression
    expression: (member_access_expression
      expression: (identifier) @_ns
      name: (identifier) @_obj)
    name: (identifier) @_method)
  arguments: (argument_list
    (argument
      (string_literal
        (string_literal_content) @env_var_name)))
  (#eq? @_ns "System")
  (#eq? @_obj "Environment")
  (#eq? @_method "GetEnvironmentVariable")) @env_access
