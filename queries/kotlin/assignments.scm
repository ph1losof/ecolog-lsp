;; ═════════════════════════════════════════════════════════════════════════
;; Kotlin Variable Assignment Queries (for chain tracking)
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; b = a (assignment)
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  (identifier) @assignment_target
  (identifier) @assignment_source) @assignment

;; ───────────────────────────────────────────────────────────────────────────
;; val b = a (property declaration)
;; ───────────────────────────────────────────────────────────────────────────
(property_declaration
  (variable_declaration
    (identifier) @assignment_target)
  (identifier) @assignment_source) @assignment
