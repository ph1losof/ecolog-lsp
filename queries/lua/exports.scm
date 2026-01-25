;; ═════════════════════════════════════════════════════════════════════════
;; Lua Export Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; Lua modules typically export via:
;; 1. return { ... } at module level
;; 2. Module-level variable assignments (global or returned)

;; ───────────────────────────────────────────────────────────────────────────
;; return { ... } - table constructor as module export
;; ───────────────────────────────────────────────────────────────────────────
(return_statement
  (expression_list
    (table_constructor) @export_value)) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; return x - identifier as module export
;; ───────────────────────────────────────────────────────────────────────────
(return_statement
  (expression_list
    (identifier) @export_name)) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; Module-level variable declarations (can be exported)
;; ───────────────────────────────────────────────────────────────────────────
(chunk
  (variable_declaration
    (assignment_statement
      (variable_list
        name: (identifier) @export_name)))) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; Module-level function declarations
;; ───────────────────────────────────────────────────────────────────────────
(chunk
  (function_declaration
    name: (identifier) @export_name)) @export_stmt
