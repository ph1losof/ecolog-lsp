;; ═════════════════════════════════════════════════════════════════════════
;; C# Destructure Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; var (a, b) = tuple; (tuple deconstruction)
;; ───────────────────────────────────────────────────────────────────────────
(local_declaration_statement
  (variable_declaration
    (variable_declarator
      (tuple_pattern
        (argument
          (declaration_expression
            (identifier) @destructure_key)))))) @destructure
