;; ═════════════════════════════════════════════════════════════════════════
;; Java Variable Assignment Queries (for chain tracking)
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; b = a (assignment expression)
;; ───────────────────────────────────────────────────────────────────────────
(assignment_expression
  left: (identifier) @assignment_target
  right: (identifier) @assignment_source) @assignment

;; ───────────────────────────────────────────────────────────────────────────
;; String b = a (local variable declaration)
;; ───────────────────────────────────────────────────────────────────────────
(local_variable_declaration
  declarator: (variable_declarator
    name: (identifier) @assignment_target
    value: (identifier) @assignment_source)) @assignment
