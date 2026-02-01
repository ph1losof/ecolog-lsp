;; ═════════════════════════════════════════════════════════════════════════
;; Ruby Environment Variable Binding Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; x = ENV['VAR']
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (identifier) @binding_name
  right: (element_reference
    object: (constant) @_obj
    (string
      (string_content) @bound_env_var))
  (#eq? @_obj "ENV")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; x = ENV.fetch('VAR')
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (identifier) @binding_name
  right: (call
    receiver: (constant) @_obj
    method: (identifier) @_method
    arguments: (argument_list
      (string
        (string_content) @bound_env_var)))
  (#eq? @_obj "ENV")
  (#eq? @_method "fetch")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; x = ENV.fetch('VAR', default)
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (identifier) @binding_name
  right: (call
    receiver: (constant) @_obj
    method: (identifier) @_method
    arguments: (argument_list
      (string
        (string_content) @bound_env_var)
      (_)))
  (#eq? @_obj "ENV")
  (#eq? @_method "fetch")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; x = ENV['VAR'] || default
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (identifier) @binding_name
  right: (binary
    left: (element_reference
      object: (constant) @_obj
      (string
        (string_content) @bound_env_var))
    operator: "||")
  (#eq? @_obj "ENV")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; env = ENV (object alias)
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (identifier) @binding_name
  right: (constant) @_obj
  (#eq? @_obj "ENV")) @env_object_binding
