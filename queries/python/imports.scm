;; ═════════════════════════════════════════════════════════════════════════
;; Python Import Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; import os
;; ───────────────────────────────────────────────────────────────────────────
(import_statement
  name: (dotted_name) @import_path @original_name) @import_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; import os as operating_system
;; ───────────────────────────────────────────────────────────────────────────
(import_statement
  name: (aliased_import
    name: (dotted_name) @import_path @original_name
    alias: (identifier) @alias_name)) @import_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; from os import environ
;; ───────────────────────────────────────────────────────────────────────────
(import_from_statement
  module_name: (dotted_name) @import_path
  name: (dotted_name) @original_name) @import_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; from os import environ as env
;; ───────────────────────────────────────────────────────────────────────────
(import_from_statement
  module_name: (dotted_name) @import_path
  name: (aliased_import
    name: (dotted_name) @original_name
    alias: (identifier) @alias_name)) @import_stmt
