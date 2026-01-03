;; ───────────────────────────────────────────────────────────────────────────
;; os.environ.get("VAR") / o.environ.get("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(call
  function: (attribute
    object: (attribute
      object: (identifier) @module
      attribute: (identifier) @_object)
    attribute: (identifier) @_method)
  arguments: (argument_list
    (string
      (string_content) @env_var_name)
    (_)?)
  (#eq? @_object "environ")
  (#any-of? @_method "get" "pop" "setdefault")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; os.environ["VAR"] / o.environ["VAR"]
;; ───────────────────────────────────────────────────────────────────────────
(subscript
  value: (attribute
    object: (identifier) @module
    attribute: (identifier) @_object)
  subscript: (string
    (string_content) @env_var_name)
  (#eq? @_object "environ")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; os.getenv("VAR") / o.getenv("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(call
  function: (attribute
    object: (identifier) @module
    attribute: (identifier) @_method)
  arguments: (argument_list
    (string
      (string_content) @env_var_name)
    (_)?)
  (#eq? @_method "getenv")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; environ["VAR"] (from os import environ) / e["VAR"] (as e)
;; ───────────────────────────────────────────────────────────────────────────
(subscript
  value: (identifier) @object
  subscript: (string
    (string_content) @env_var_name)
  ;; No built-in way to know if this identifier is environ without ImportContext check
  ;; But we capture it as @object, and extract_references validates it.
) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; environ.get("VAR") / e.get("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(call
  function: (attribute
    object: (identifier) @object
    attribute: (identifier) @_method)
  arguments: (argument_list
    (string
      (string_content) @env_var_name)
    (_)?)
  (#any-of? @_method "get" "pop" "setdefault")
) @env_access
