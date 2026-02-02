;; ═════════════════════════════════════════════════════════════════════════
;; Elixir Import Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; import Module
;; alias Module
;; require Module
;; use Module
;; ───────────────────────────────────────────────────────────────────────────
(call
  target: (identifier) @_directive
  (arguments
    (alias) @import_path)
  (#any-of? @_directive "import" "alias" "require" "use")) @import_stmt
