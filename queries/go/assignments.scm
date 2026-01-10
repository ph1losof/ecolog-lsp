;; ═════════════════════════════════════════════════════════════════
;; Go Variable Assignment Queries (for chain tracking)
;; ═════════════════════════════════════════════════════════════════
;; These patterns capture variable-to-variable assignments that may
;; form chains back to environment variables.
;;
;; Example chain: env := os.Getenv("VAR"); val := env

;; ───────────────────────────────────────────────────────────────────
;; b := a (short variable declaration from identifier)
;; ───────────────────────────────────────────────────────────────────
(short_var_declaration
  left: (expression_list
    (identifier) @assignment_target)
  right: (expression_list
    (identifier) @assignment_source)) @assignment

;; ───────────────────────────────────────────────────────────────────
;; var b = a
;; ───────────────────────────────────────────────────────────────────
(var_declaration
  (var_spec
    name: (identifier) @assignment_target
    value: (expression_list
      (identifier) @assignment_source))) @assignment

;; ───────────────────────────────────────────────────────────────────
;; b = a (assignment statement)
;; ───────────────────────────────────────────────────────────────────
(assignment_statement
  left: (expression_list
    (identifier) @assignment_target)
  right: (expression_list
    (identifier) @assignment_source)) @assignment

;; ───────────────────────────────────────────────────────────────────
;; var b Type = a (with type)
;; ───────────────────────────────────────────────────────────────────
(var_declaration
  (var_spec
    name: (identifier) @assignment_target
    type: (_)
    value: (expression_list
      (identifier) @assignment_source))) @assignment
