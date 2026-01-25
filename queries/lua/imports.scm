;; ═════════════════════════════════════════════════════════════════════════
;; Lua Import Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; Lua uses require() for module imports.

;; ───────────────────────────────────────────────────────────────────────────
;; local x = require("module")
;; ───────────────────────────────────────────────────────────────────────────
(variable_declaration
  (assignment_statement
    (variable_list
      name: (identifier) @alias_name)
    (expression_list
      value: (function_call
        name: (identifier) @_func
        arguments: (arguments
          (string
            content: (string_content) @import_path)))))
  (#eq? @_func "require")) @import_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; x = require("module") (global)
;; ───────────────────────────────────────────────────────────────────────────
(assignment_statement
  (variable_list
    name: (identifier) @alias_name)
  (expression_list
    value: (function_call
      name: (identifier) @_func
      arguments: (arguments
        (string
          content: (string_content) @import_path))))
  (#eq? @_func "require")) @import_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; require("module") (without assignment)
;; ───────────────────────────────────────────────────────────────────────────
(function_call
  name: (identifier) @_func
  arguments: (arguments
    (string
      content: (string_content) @import_path))
  (#eq? @_func "require")) @import_stmt
