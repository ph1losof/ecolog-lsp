;; ═════════════════════════════════════════════════════════════════════════
;; Ruby Environment Variable Reference Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; Primary patterns: ENV['VAR'], ENV.fetch('VAR')

;; ───────────────────────────────────────────────────────────────────────────
;; ENV['VAR']
;; ───────────────────────────────────────────────────────────────────────────
(element_reference
  object: (constant) @_obj
  (string
    (string_content) @env_var_name)
  (#eq? @_obj "ENV")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; ENV.fetch('VAR')
;; ───────────────────────────────────────────────────────────────────────────
(call
  receiver: (constant) @_obj
  method: (identifier) @_method
  arguments: (argument_list
    (string
      (string_content) @env_var_name))
  (#eq? @_obj "ENV")
  (#eq? @_method "fetch")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; ENV.fetch('VAR', default)
;; ───────────────────────────────────────────────────────────────────────────
(call
  receiver: (constant) @_obj
  method: (identifier) @_method
  arguments: (argument_list
    (string
      (string_content) @env_var_name)
    (_))
  (#eq? @_obj "ENV")
  (#eq? @_method "fetch")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; ENV.key?('VAR')
;; ───────────────────────────────────────────────────────────────────────────
(call
  receiver: (constant) @_obj
  method: (identifier) @_method
  arguments: (argument_list
    (string
      (string_content) @env_var_name))
  (#eq? @_obj "ENV")
  (#any-of? @_method "key?" "has_key?" "include?")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; ENV['VAR'] || default
;; ───────────────────────────────────────────────────────────────────────────
(binary
  left: (element_reference
    object: (constant) @_obj
    (string
      (string_content) @env_var_name))
  operator: "||"
  (#eq? @_obj "ENV")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; ENV.values_at('VAR1', 'VAR2')
;; ───────────────────────────────────────────────────────────────────────────
(call
  receiver: (constant) @_obj
  method: (identifier) @_method
  arguments: (argument_list
    (string
      (string_content) @env_var_name))
  (#eq? @_obj "ENV")
  (#eq? @_method "values_at")) @env_access
