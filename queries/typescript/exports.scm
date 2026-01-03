;; ═══════════════════════════════════════════════════════════════════════════
;; TypeScript Export Queries
;; ═══════════════════════════════════════════════════════════════════════════
;;
;; TypeScript shares export syntax with JavaScript.
;; TypeScript-specific type exports (interface, type, enum) don't export
;; runtime values, so they're not relevant for env var tracking.
;;
;; Captures:
;;   @export_name    - The exported identifier name
;;   @export_value   - The value being exported (optional)
;;   @local_name     - The local name if aliased (optional)
;;   @reexport_source - Module specifier for re-exports (optional)
;;   @wildcard_source - Module specifier for wildcard re-exports (optional)
;;   @export_stmt    - The entire export statement node
;;   @default_export - Marks a default export

;; ───────────────────────────────────────────────────────────────────────────
;; export const foo = value (ES modules named export with declaration)
;; ───────────────────────────────────────────────────────────────────────────
(export_statement
  declaration: (lexical_declaration
    (variable_declarator
      name: (identifier) @export_name
      value: (_) @export_value))) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; export const { foo, bar } = obj (destructured export)
;; ───────────────────────────────────────────────────────────────────────────
(export_statement
  declaration: (lexical_declaration
    (variable_declarator
      name: (object_pattern
        (shorthand_property_identifier_pattern) @export_name)
      value: (_) @export_value))) @export_stmt

;; export const { foo: aliasedFoo } = obj (destructured export with alias)
(export_statement
  declaration: (lexical_declaration
    (variable_declarator
      name: (object_pattern
        (pair_pattern
          key: (property_identifier) @local_name
          value: (identifier) @export_name))
      value: (_) @export_value))) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; export function foo() {} (function export)
;; ───────────────────────────────────────────────────────────────────────────
(export_statement
  declaration: (function_declaration
    name: (identifier) @export_name)) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; export class Foo {} (class export)
;; ───────────────────────────────────────────────────────────────────────────
(export_statement
  declaration: (class_declaration
    name: (type_identifier) @export_name)) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; export { foo } (named export from local scope)
;; ───────────────────────────────────────────────────────────────────────────
(export_statement
  (export_clause
    (export_specifier
      name: (identifier) @export_name))) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; export { foo as bar } (named export with alias)
;; ───────────────────────────────────────────────────────────────────────────
(export_statement
  (export_clause
    (export_specifier
      name: (identifier) @local_name
      alias: (identifier) @export_name))) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; export { foo } from "./module" (re-export named)
;; ───────────────────────────────────────────────────────────────────────────
(export_statement
  (export_clause
    (export_specifier
      name: (identifier) @export_name))
  source: (string
    (string_fragment) @reexport_source)) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; export { foo as bar } from "./module" (re-export with alias)
;; ───────────────────────────────────────────────────────────────────────────
(export_statement
  (export_clause
    (export_specifier
      name: (identifier) @local_name
      alias: (identifier) @export_name))
  source: (string
    (string_fragment) @reexport_source)) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; export * from "./module" (wildcard re-export)
;; ───────────────────────────────────────────────────────────────────────────
(export_statement
  "*"
  source: (string
    (string_fragment) @wildcard_source)) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; export * as ns from "./module" (namespace re-export)
;; ───────────────────────────────────────────────────────────────────────────
(export_statement
  (namespace_export
    (identifier) @export_name)
  source: (string
    (string_fragment) @reexport_source)) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; export default expression (default export)
;; ───────────────────────────────────────────────────────────────────────────
(export_statement
  "default"
  value: (identifier) @export_value) @default_export

(export_statement
  "default"
  value: (_) @export_value) @default_export

;; ───────────────────────────────────────────────────────────────────────────
;; export default function foo() {} (named default function)
;; ───────────────────────────────────────────────────────────────────────────
(export_statement
  "default"
  declaration: (function_declaration
    name: (identifier) @export_name)) @default_export

;; ───────────────────────────────────────────────────────────────────────────
;; export default class Foo {} (named default class)
;; ───────────────────────────────────────────────────────────────────────────
(export_statement
  "default"
  declaration: (class_declaration
    name: (type_identifier) @export_name)) @default_export

;; ───────────────────────────────────────────────────────────────────────────
;; module.exports = value (CommonJS default export)
;; ───────────────────────────────────────────────────────────────────────────
(expression_statement
  (assignment_expression
    left: (member_expression
      object: (identifier) @_obj
      property: (property_identifier) @_prop)
    right: (_) @export_value)
  (#eq? @_obj "module")
  (#eq? @_prop "exports")) @cjs_default_export

;; ───────────────────────────────────────────────────────────────────────────
;; module.exports.foo = value (CommonJS named export)
;; ───────────────────────────────────────────────────────────────────────────
(expression_statement
  (assignment_expression
    left: (member_expression
      object: (member_expression
        object: (identifier) @_obj
        property: (property_identifier) @_prop)
      property: (property_identifier) @export_name)
    right: (_) @export_value)
  (#eq? @_obj "module")
  (#eq? @_prop "exports")) @cjs_named_export

;; ───────────────────────────────────────────────────────────────────────────
;; exports.foo = value (CommonJS shorthand named export)
;; ───────────────────────────────────────────────────────────────────────────
(expression_statement
  (assignment_expression
    left: (member_expression
      object: (identifier) @_obj
      property: (property_identifier) @export_name)
    right: (_) @export_value)
  (#eq? @_obj "exports")) @cjs_named_export
