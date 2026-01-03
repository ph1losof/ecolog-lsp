;; ═════════════════════════════════════════════════════════════════
;; Rust Variable Assignment Queries (for chain tracking)
;; ═════════════════════════════════════════════════════════════════
;; These patterns capture variable-to-variable assignments that may
;; form chains back to environment variables.
;;
;; Example chain: let env = std::env::var("VAR")?; let val = env;

;; ───────────────────────────────────────────────────────────────────
;; let b = a (simple let binding from identifier)
;; ───────────────────────────────────────────────────────────────────
(let_declaration
  pattern: (identifier) @assignment_target
  value: (identifier) @assignment_source) @assignment

;; ───────────────────────────────────────────────────────────────────
;; let mut b = a
;; ───────────────────────────────────────────────────────────────────
(let_declaration
  pattern: (mut_pattern
    (identifier) @assignment_target)
  value: (identifier) @assignment_source) @assignment

;; ───────────────────────────────────────────────────────────────────
;; let b: Type = a (with type annotation)
;; ───────────────────────────────────────────────────────────────────
(let_declaration
  pattern: (identifier) @assignment_target
  type: (_)
  value: (identifier) @assignment_source) @assignment

;; ───────────────────────────────────────────────────────────────────
;; b = a (assignment expression)
;; ───────────────────────────────────────────────────────────────────
(assignment_expression
  left: (identifier) @assignment_target
  right: (identifier) @assignment_source) @assignment

;; ───────────────────────────────────────────────────────────────────
;; let b = a.clone() - track if a is an env-related variable
;; ───────────────────────────────────────────────────────────────────
(let_declaration
  pattern: (identifier) @assignment_target
  value: (call_expression
    function: (field_expression
      value: (identifier) @assignment_source
      field: (field_identifier) @_method)
    (#eq? @_method "clone"))) @assignment
