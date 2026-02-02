;; ═════════════════════════════════════════════════════════════════════════
;; Bash/Shell Variable Assignment Queries (for chain tracking)
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; B=$A (assignment from variable)
;; ───────────────────────────────────────────────────────────────────────────
(variable_assignment
  name: (variable_name) @assignment_target
  value: (simple_expansion
    (variable_name) @assignment_source)) @assignment

(variable_assignment
  name: (variable_name) @assignment_target
  value: (expansion
    (variable_name) @assignment_source)) @assignment
