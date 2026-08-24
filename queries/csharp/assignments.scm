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
;;
;; `variable_declarator` holds the initializer directly; there is no
;; `equals_value_clause` node in this grammar.
;; ───────────────────────────────────────────────────────────────────────────
(local_declaration_statement
  (variable_declaration
    (variable_declarator
      name: (identifier) @assignment_target
      (identifier) @assignment_source))) @assignment
