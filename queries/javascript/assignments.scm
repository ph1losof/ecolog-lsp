;; ═════════════════════════════════════════════════════════════════
;; JavaScript Variable Assignment Queries (for chain tracking)
;; ═════════════════════════════════════════════════════════════════
;; These patterns capture variable-to-variable assignments that may
;; form chains back to environment variables.
;;
;; Example chain: const env = process.env; const config = env; const x = config.VAR;

;; ───────────────────────────────────────────────────────────────────
;; const/let/var b = a (simple chain assignment from identifier)
;; ───────────────────────────────────────────────────────────────────
(variable_declarator
  name: (identifier) @assignment_target
  value: (identifier) @assignment_source) @assignment

;; ───────────────────────────────────────────────────────────────────
;; b = a (reassignment from identifier)
;; ───────────────────────────────────────────────────────────────────
(assignment_expression
  left: (identifier) @assignment_target
  right: (identifier) @assignment_source) @assignment
