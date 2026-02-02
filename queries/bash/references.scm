;; ═════════════════════════════════════════════════════════════════════════
;; Bash/Shell Environment Variable Reference Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; $VAR (simple expansion)
;; ───────────────────────────────────────────────────────────────────────────
(simple_expansion
  (variable_name) @env_var_name) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; ${VAR} (expansion with braces)
;; ${VAR:-default} (expansion with default)
;; ${VAR:=default} (expansion with assignment)
;; ${VAR:+alternative} (expansion with alternative)
;; ${VAR:?error} (expansion with error)
;; ───────────────────────────────────────────────────────────────────────────
(expansion
  (variable_name) @env_var_name) @env_access
