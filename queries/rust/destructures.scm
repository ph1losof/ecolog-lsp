;; ═════════════════════════════════════════════════════════════════════════
;; Rust Destructuring Pattern Queries (from env-holding structs/vars)
;; ═════════════════════════════════════════════════════════════════════════
;; Rust tracks struct destructuring and field access from config objects

;; ───────────────────────────────────────────────────────────────────────────
;; let Config { db, api } = config; (struct destructuring - shorthand)
;; ───────────────────────────────────────────────────────────────────────────
(let_declaration
  pattern: (struct_pattern
    type: (_) @destructure_source
    (field_pattern
      name: (shorthand_field_identifier) @destructure_target))) @destructure

;; ───────────────────────────────────────────────────────────────────────────
;; let Config { db: database_url, .. } = config; (renamed field)
;; ───────────────────────────────────────────────────────────────────────────
(let_declaration
  pattern: (struct_pattern
    type: (_) @destructure_source
    (field_pattern
      name: (field_identifier) @destructure_key
      pattern: (identifier) @destructure_target))) @destructure

;; ───────────────────────────────────────────────────────────────────────────
;; let val = config.db; (field access from struct)
;; ───────────────────────────────────────────────────────────────────────────
(let_declaration
  pattern: (identifier) @destructure_target
  value: (field_expression
    value: (identifier) @destructure_source
    field: (field_identifier) @destructure_key)) @destructure
