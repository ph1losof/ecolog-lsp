;; ═════════════════════════════════════════════════════════════════════════
;; Lua Variable Assignment Queries (for chain tracking)
;; ═════════════════════════════════════════════════════════════════════════
;;
;; These patterns capture variable-to-variable assignments that may
;; form chains back to environment variables.
;;
;; Example chain: local env = os.getenv("VAR"); local val = env

;; ───────────────────────────────────────────────────────────────────────────
;; local x = y (local variable from identifier)
;; ───────────────────────────────────────────────────────────────────────────
(variable_declaration
  (assignment_statement
    (variable_list
      name: (identifier) @assignment_target)
    (expression_list
      value: (identifier) @assignment_source))) @assignment

;; ───────────────────────────────────────────────────────────────────────────
;; x = y (global assignment from identifier)
;; ───────────────────────────────────────────────────────────────────────────
(assignment_statement
  (variable_list
    name: (identifier) @assignment_target)
  (expression_list
    value: (identifier) @assignment_source)) @assignment
