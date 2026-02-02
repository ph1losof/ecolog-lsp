;; ═════════════════════════════════════════════════════════════════════════
;; C# Variable Assignment Queries (for chain tracking)
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; b = a (assignment expression)
;; ───────────────────────────────────────────────────────────────────────────
(assignment_expression
  left: (identifier) @assignment_target
  right: (identifier) @assignment_source) @assignment

;; ───────────────────────────────────────────────────────────────────────────
;; var b = a (local declaration)
;; ───────────────────────────────────────────────────────────────────────────
(local_declaration_statement
  (variable_declaration
    (variable_declarator
      (identifier) @assignment_target
      (equals_value_clause
        (identifier) @assignment_source)))) @assignment
