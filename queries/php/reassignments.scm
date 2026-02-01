;; ═════════════════════════════════════════════════════════════════════════
;; PHP Reassignment Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; These patterns detect when variables are reassigned, which can
;; invalidate binding chains.

;; ───────────────────────────────────────────────────────────────────────────
;; $x = new_value (assignment to existing variable)
;; ───────────────────────────────────────────────────────────────────────────
(assignment_expression
  left: (variable_name) @reassigned_name) @reassignment
