;; ═════════════════════════════════════════════════════════════════════════
;; PHP Variable Assignment Queries (for chain tracking)
;; ═════════════════════════════════════════════════════════════════════════
;;
;; These patterns capture variable-to-variable assignments that may
;; form chains back to environment variables.

;; ───────────────────────────────────────────────────────────────────────────
;; $x = $y (variable to variable assignment)
;; ───────────────────────────────────────────────────────────────────────────
(assignment_expression
  left: (variable_name) @assignment_target
  right: (variable_name) @assignment_source) @assignment
