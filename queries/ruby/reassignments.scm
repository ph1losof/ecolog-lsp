;; ═════════════════════════════════════════════════════════════════════════
;; Ruby Reassignment Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; These patterns detect when variables are reassigned, which can
;; invalidate binding chains.

;; ───────────────────────────────────────────────────────────────────────────
;; x = new_value (assignment to existing variable)
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (identifier) @reassigned_name) @reassignment

;; ───────────────────────────────────────────────────────────────────────────
;; @instance_var = new_value
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (instance_variable) @reassigned_name) @reassignment
