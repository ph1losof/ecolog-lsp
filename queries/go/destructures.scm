;; ═════════════════════════════════════════════════════════════════════════
;; Go Destructuring Pattern Queries (from env aliases/maps)
;; ═════════════════════════════════════════════════════════════════════════
;; Go doesn't have JS-style destructuring, but we track map/subscript access
;; from variables that hold environment data (e.g., envMap["KEY"])

;; ───────────────────────────────────────────────────────────────────────────
;; val := envMap["KEY"] (short var declaration with index expression)
;; ───────────────────────────────────────────────────────────────────────────
(short_var_declaration
  left: (expression_list
    (identifier) @destructure_target)
  right: (expression_list
    (index_expression
      operand: (identifier) @destructure_source
      index: (interpreted_string_literal) @destructure_key))) @destructure

;; ───────────────────────────────────────────────────────────────────────────
;; var val = envMap["KEY"] (var declaration with index expression)
;; ───────────────────────────────────────────────────────────────────────────
(var_declaration
  (var_spec
    name: (identifier) @destructure_target
    value: (expression_list
      (index_expression
        operand: (identifier) @destructure_source
        index: (interpreted_string_literal) @destructure_key)))) @destructure

;; ───────────────────────────────────────────────────────────────────────────
;; val, ok := envMap["KEY"] (with ok check)
;; ───────────────────────────────────────────────────────────────────────────
(short_var_declaration
  left: (expression_list
    (identifier) @destructure_target
    (identifier))
  right: (expression_list
    (index_expression
      operand: (identifier) @destructure_source
      index: (interpreted_string_literal) @destructure_key))) @destructure

;; ───────────────────────────────────────────────────────────────────────────
;; val = envMap["KEY"] (assignment to existing variable)
;; ───────────────────────────────────────────────────────────────────────────
(assignment_statement
  left: (expression_list
    (identifier) @destructure_target)
  right: (expression_list
    (index_expression
      operand: (identifier) @destructure_source
      index: (interpreted_string_literal) @destructure_key))) @destructure
