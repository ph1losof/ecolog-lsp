;; ═════════════════════════════════════════════════════════════════════════
;; Java Environment Variable Reference Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; System.getenv("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(method_invocation
  object: (identifier) @_obj
  name: (identifier) @_method
  arguments: (argument_list
    (string_literal
      (string_fragment) @env_var_name))
  (#eq? @_obj "System")
  (#eq? @_method "getenv")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; System.getProperty("VAR") - for system properties
;; ───────────────────────────────────────────────────────────────────────────
(method_invocation
  object: (identifier) @_obj
  name: (identifier) @_method
  arguments: (argument_list
    (string_literal
      (string_fragment) @env_var_name))
  (#eq? @_obj "System")
  (#eq? @_method "getProperty")) @env_access
