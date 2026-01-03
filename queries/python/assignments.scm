;; ═════════════════════════════════════════════════════════════════
;; Python Variable Assignment Queries (for chain tracking)
;; ═════════════════════════════════════════════════════════════════
;; These patterns capture variable-to-variable assignments that may
;; form chains back to environment variables.
;;
;; Example chain: env = os.environ; config = env; val = config["KEY"]

;; ───────────────────────────────────────────────────────────────────
;; b = a (simple assignment from identifier)
;; ───────────────────────────────────────────────────────────────────
(assignment
  left: (identifier) @assignment_target
  right: (identifier) @assignment_source) @assignment

;; ───────────────────────────────────────────────────────────────────
;; b: type = a (annotated assignment)
;; ───────────────────────────────────────────────────────────────────
(assignment
  left: (identifier) @assignment_target
  type: (_)?
  right: (identifier) @assignment_source) @assignment

;; ───────────────────────────────────────────────────────────────────
;; Walrus operator: (b := a)
;; ───────────────────────────────────────────────────────────────────
(named_expression
  name: (identifier) @assignment_target
  value: (identifier) @assignment_source) @assignment
