;; ═════════════════════════════════════════════════════════════════════════
;; PHP Import Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; PHP uses 'use' statements for importing classes/namespaces.

;; ───────────────────────────────────────────────────────────────────────────
;; use App\Config;
;; ───────────────────────────────────────────────────────────────────────────
(namespace_use_declaration
  (namespace_use_clause
    (qualified_name) @import_path)) @import_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; use App\Config as Cfg;
;; ───────────────────────────────────────────────────────────────────────────
(namespace_use_declaration
  (namespace_use_clause
    (qualified_name) @import_path
    alias: (name) @alias_name)) @import_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; require 'file.php'; require_once 'file.php';
;; ───────────────────────────────────────────────────────────────────────────
(include_expression
  (string
    (string_content) @import_path)) @import_stmt

(include_once_expression
  (string
    (string_content) @import_path)) @import_stmt

(require_expression
  (string
    (string_content) @import_path)) @import_stmt

(require_once_expression
  (string
    (string_content) @import_path)) @import_stmt
