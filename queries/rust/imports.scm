;; ═════════════════════════════════════════════════════════════════════════
;; Rust Import Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; use std::env;
;; use std::env as env;
;; use std::env::var;
;; ───────────────────────────────────────────────────────────────────────────
;; use std::env as e;
;; use std::env as e;
;; use std::env as e;
(use_declaration
  argument: (use_as_clause
    path: (_) @import_path
    alias: (identifier) @alias_name
  )
) @import_stmt

;; use std::env;
(use_declaration
  argument: (scoped_identifier
    path: (identifier)
    name: (identifier) @import_path)
) @import
