;; ═══════════════════════════════════════════════════════════════════════════
;; JavaScript Import Queries
;; ═══════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; import "module" (side-effect import)
;; ───────────────────────────────────────────────────────────────────────────
;; (import_statement
;;   source: (string
;;     (string_fragment) @import_path)
;;   !import_clause) @import_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; import x from "module" (default import)
;; ───────────────────────────────────────────────────────────────────────────
(import_statement
  (import_clause
    (identifier) @alias_name)
  source: (string
    (string_fragment) @import_path)) @import_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; import * as x from "module" (namespace import)
;; ───────────────────────────────────────────────────────────────────────────
(import_statement
  (import_clause
    (namespace_import
      (identifier) @alias_name))
  source: (string
    (string_fragment) @import_path)) @import_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; import { x, y as z } from "module" (named imports)
;; ───────────────────────────────────────────────────────────────────────────
(import_statement
  (import_clause
    (named_imports
      (import_specifier
        name: (identifier) @original_name
        alias: (identifier)? @alias_name)))
  source: (string
    (string_fragment) @import_path)) @import_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; const x = require("module")
;; ───────────────────────────────────────────────────────────────────────────
(variable_declarator
  name: (identifier) @alias_name
  value: (call_expression
    function: (identifier) @function_name
    arguments: (arguments
      (string
        (string_fragment) @import_path)))
  (#eq? @function_name "require")) @import_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; const { x } = require("module")
;; ───────────────────────────────────────────────────────────────────────────
(variable_declarator
  name: (object_pattern
    (shorthand_property_identifier_pattern) @alias_name)
  value: (call_expression
    function: (identifier) @function_name
    arguments: (arguments
      (string
        (string_fragment) @import_path)))
  (#eq? @function_name "require")) @import_stmt

(variable_declarator
  name: (object_pattern
    (pair_pattern
      key: (property_identifier) @original_name
      value: (identifier) @alias_name))
  value: (call_expression
    function: (identifier) @function_name
    arguments: (arguments
      (string
        (string_fragment) @import_path)))
  (#eq? @function_name "require")) @import_stmt
