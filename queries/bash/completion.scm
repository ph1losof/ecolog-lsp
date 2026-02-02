;; ═════════════════════════════════════════════════════════════════════════
;; Bash/Shell Completion Context Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; $VAR - trigger completion after $
;; ${VAR} - trigger completion after ${
;; ───────────────────────────────────────────────────────────────────────────
(simple_expansion) @object @completion_target

(expansion) @object @completion_target
