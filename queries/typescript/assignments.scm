;; ═════════════════════════════════════════════════════════════════
;; TypeScript Variable Assignment Queries (for chain tracking)
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

;; ───────────────────────────────────────────────────────────────────
;; TypeScript-specific: const b = a as SomeType
;; ───────────────────────────────────────────────────────────────────
(variable_declarator
  name: (identifier) @assignment_target
  value: (as_expression
    (identifier) @assignment_source)) @assignment

;; ───────────────────────────────────────────────────────────────────
;; TypeScript-specific: const b = a!
;; ───────────────────────────────────────────────────────────────────
(variable_declarator
  name: (identifier) @assignment_target
  value: (non_null_expression
    (identifier) @assignment_source)) @assignment

;; ───────────────────────────────────────────────────────────────────
;; Note: Legacy angle-bracket type assertion syntax (<Type>value) not tracked
;; as it conflicts with JSX in TSX files and uses a different node structure
;; ───────────────────────────────────────────────────────────────────
