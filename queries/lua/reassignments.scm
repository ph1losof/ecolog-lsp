;; ═════════════════════════════════════════════════════════════════════════
;; Lua Reassignment Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; These patterns detect when variables are reassigned, which can
;; invalidate binding chains.

;; ───────────────────────────────────────────────────────────────────────────
;; x = new_value (non-declaration assignment)
;; ───────────────────────────────────────────────────────────────────────────
(assignment_statement
  (variable_list
    name: (identifier) @reassigned_name)) @reassignment
