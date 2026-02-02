;; ═════════════════════════════════════════════════════════════════════════
;; Bash/Shell Environment Variable Binding Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; X=$VAR (assignment from variable)
;; ───────────────────────────────────────────────────────────────────────────
(variable_assignment
  name: (variable_name) @binding_name
  value: (simple_expansion
    (variable_name) @bound_env_var)) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; X=${VAR} (assignment from expansion)
;; ───────────────────────────────────────────────────────────────────────────
(variable_assignment
  name: (variable_name) @binding_name
  value: (expansion
    (variable_name) @bound_env_var)) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; X="${VAR}" (assignment from quoted expansion)
;; ───────────────────────────────────────────────────────────────────────────
(variable_assignment
  name: (variable_name) @binding_name
  value: (string
    (simple_expansion
      (variable_name) @bound_env_var))) @env_binding

(variable_assignment
  name: (variable_name) @binding_name
  value: (string
    (expansion
      (variable_name) @bound_env_var))) @env_binding
