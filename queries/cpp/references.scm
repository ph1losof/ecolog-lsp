;; ═════════════════════════════════════════════════════════════════════════
;; C++ Environment Variable Reference Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; getenv("VAR") - C-style
;; ───────────────────────────────────────────────────────────────────────────
(call_expression
  function: (identifier) @_func
  arguments: (argument_list
    (string_literal
      (string_content) @env_var_name))
  (#any-of? @_func "getenv" "secure_getenv")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; std::getenv("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(call_expression
  function: (qualified_identifier
    scope: (namespace_identifier) @_ns
    name: (identifier) @_func)
  arguments: (argument_list
    (string_literal
      (string_content) @env_var_name))
  (#eq? @_ns "std")
  (#eq? @_func "getenv")) @env_access
