;; ═════════════════════════════════════════════════════════════════════════
;; Kotlin Destructure Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; val (a, b) = pair (destructuring declaration)
;; ───────────────────────────────────────────────────────────────────────────
(property_declaration
  (multi_variable_declaration
    (variable_declaration
      (identifier) @destructure_key))) @destructure
