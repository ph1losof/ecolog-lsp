;; ═════════════════════════════════════════════════════════════════════════
;; Lua Table Field Access Queries (for chain tracking)
;; ═════════════════════════════════════════════════════════════════════════
;;
;; Lua doesn't have JavaScript-style destructuring, but it does have
;; table field access which can be used in chains.
;;
;; Example: local x = config.database_url

;; ───────────────────────────────────────────────────────────────────────────
;; local x = table.field (dot notation)
;; ───────────────────────────────────────────────────────────────────────────
(variable_declaration
  (assignment_statement
    (variable_list
      name: (identifier) @destructure_target)
    (expression_list
      value: (dot_index_expression
        table: (identifier) @destructure_source
        field: (identifier) @destructure_key)))) @destructure

;; ───────────────────────────────────────────────────────────────────────────
;; x = table.field (global assignment, dot notation)
;; ───────────────────────────────────────────────────────────────────────────
(assignment_statement
  (variable_list
    name: (identifier) @destructure_target)
  (expression_list
    value: (dot_index_expression
      table: (identifier) @destructure_source
      field: (identifier) @destructure_key))) @destructure

;; ───────────────────────────────────────────────────────────────────────────
;; local x = table["key"] (bracket notation with string)
;; ───────────────────────────────────────────────────────────────────────────
(variable_declaration
  (assignment_statement
    (variable_list
      name: (identifier) @destructure_target)
    (expression_list
      value: (bracket_index_expression
        table: (identifier) @destructure_source
        field: (string
          content: (string_content) @destructure_key))))) @destructure

;; ───────────────────────────────────────────────────────────────────────────
;; x = table["key"] (global assignment, bracket notation)
;; ───────────────────────────────────────────────────────────────────────────
(assignment_statement
  (variable_list
    name: (identifier) @destructure_target)
  (expression_list
    value: (bracket_index_expression
      table: (identifier) @destructure_source
      field: (string
        content: (string_content) @destructure_key)))) @destructure
