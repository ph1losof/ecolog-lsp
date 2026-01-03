;; ═══════════════════════════════════════════════════════════════════════════
;; Python Export Queries
;; ═══════════════════════════════════════════════════════════════════════════
;;
;; Python doesn't have explicit export syntax - all module-level names are
;; implicitly exported. We capture module-level assignments to track potential
;; env var re-exports.
;;
;; The __all__ list can be used for explicit exports but is optional.
;;
;; Captures:
;;   @export_name    - The exported identifier name
;;   @export_value   - The value being exported (optional)
;;   @export_stmt    - The assignment/definition statement
;;   @all_list       - The __all__ list if defined

;; ───────────────────────────────────────────────────────────────────────────
;; Module-level variable assignments: foo = value
;; These are implicit exports in Python
;; ───────────────────────────────────────────────────────────────────────────
(module
  (expression_statement
    (assignment
      left: (identifier) @export_name
      right: (_) @export_value)) @export_stmt)

;; ───────────────────────────────────────────────────────────────────────────
;; Module-level annotated assignments: foo: Type = value
;; ───────────────────────────────────────────────────────────────────────────
(module
  (expression_statement
    (assignment
      left: (identifier) @export_name
      type: (_)
      right: (_) @export_value)) @export_stmt)

;; ───────────────────────────────────────────────────────────────────────────
;; Module-level function definitions: def foo(): ...
;; ───────────────────────────────────────────────────────────────────────────
(module
  (function_definition
    name: (identifier) @export_name) @export_stmt)

;; ───────────────────────────────────────────────────────────────────────────
;; Module-level class definitions: class Foo: ...
;; ───────────────────────────────────────────────────────────────────────────
(module
  (class_definition
    name: (identifier) @export_name) @export_stmt)

;; ───────────────────────────────────────────────────────────────────────────
;; __all__ = ["foo", "bar"] - explicit export list
;; This defines which names are exported when using `from module import *`
;; ───────────────────────────────────────────────────────────────────────────
(module
  (expression_statement
    (assignment
      left: (identifier) @_all_name
      right: (list) @all_list)
    (#eq? @_all_name "__all__"))) @all_definition

;; ───────────────────────────────────────────────────────────────────────────
;; Re-export via from x import y pattern at module level
;; from .config import DATABASE_URL  # re-exports DATABASE_URL
;; ───────────────────────────────────────────────────────────────────────────
(module
  (import_from_statement
    module_name: (_) @reexport_source
    name: (dotted_name
      (identifier) @export_name))) @reexport_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; Re-export via from x import y as z pattern
;; from .config import DB_URL as DATABASE_URL
;; ───────────────────────────────────────────────────────────────────────────
(module
  (import_from_statement
    module_name: (_) @reexport_source
    name: (aliased_import
      name: (dotted_name
        (identifier) @local_name)
      alias: (identifier) @export_name))) @reexport_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; Wildcard re-export: from .config import *
;; ───────────────────────────────────────────────────────────────────────────
(module
  (import_from_statement
    module_name: (_) @wildcard_source
    (wildcard_import))) @wildcard_reexport
