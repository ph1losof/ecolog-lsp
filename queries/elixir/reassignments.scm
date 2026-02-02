;; ═════════════════════════════════════════════════════════════════════════
;; Elixir Reassignment Queries
;; ═════════════════════════════════════════════════════════════════════════
;; Note: Elixir uses pattern matching, variables can be rebound.

;; ───────────────────────────────────────────────────────────────────────────
;; x = something_else (rebinding)
;; ───────────────────────────────────────────────────────────────────────────
(binary_operator
  left: (identifier) @reassigned_name
  operator: "="
  right: (_) @new_value) @reassignment
