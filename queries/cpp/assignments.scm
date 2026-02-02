;; ═════════════════════════════════════════════════════════════════════════
;; C++ Variable Assignment Queries (for chain tracking)
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; b = a (assignment expression)
;; ───────────────────────────────────────────────────────────────────────────
(assignment_expression
  left: (identifier) @assignment_target
  right: (identifier) @assignment_source) @assignment

;; ───────────────────────────────────────────────────────────────────────────
;; auto b = a (declaration with initialization)
;; ───────────────────────────────────────────────────────────────────────────
(declaration
  declarator: (init_declarator
    declarator: (identifier) @assignment_target
    value: (identifier) @assignment_source)) @assignment

(declaration
  declarator: (init_declarator
    declarator: (pointer_declarator
      declarator: (identifier) @assignment_target)
    value: (identifier) @assignment_source)) @assignment

(declaration
  declarator: (init_declarator
    declarator: (reference_declarator
      (identifier) @assignment_target)
    value: (identifier) @assignment_source)) @assignment
