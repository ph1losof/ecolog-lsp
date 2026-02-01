;; ═════════════════════════════════════════════════════════════════════════
;; Ruby Variable Assignment Queries (for chain tracking)
;; ═════════════════════════════════════════════════════════════════════════
;;
;; These patterns capture variable-to-variable assignments that may
;; form chains back to environment variables.

;; ───────────────────────────────────────────────────────────────────────────
;; x = y (variable to variable assignment)
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (identifier) @assignment_target
  right: (identifier) @assignment_source) @assignment

;; ───────────────────────────────────────────────────────────────────────────
;; @instance_var = local_var
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (instance_variable) @assignment_target
  right: (identifier) @assignment_source) @assignment
