;; ═════════════════════════════════════════════════════════════════════════
;; Kotlin Environment Variable Binding Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; val x = System.getenv("VAR")
;; var x = System.getenv("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(property_declaration
  (variable_declaration
    (identifier) @binding_name)
  (call_expression
    (navigation_expression
      (identifier) @_obj
      (identifier) @_method)
    (value_arguments
      (value_argument
        (string_literal
          (string_content) @bound_env_var))))
  (#eq? @_obj "System")
  (#eq? @_method "getenv")) @env_binding
