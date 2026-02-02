;; ═════════════════════════════════════════════════════════════════════════
;; Elixir Variable Assignment Queries (for chain tracking)
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; b = a (match/assignment)
;; ───────────────────────────────────────────────────────────────────────────
(binary_operator
  left: (identifier) @assignment_target
  operator: "="
  right: (identifier) @assignment_source) @assignment
