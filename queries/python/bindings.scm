;; ═════════════════════════════════════════════════════════════════════════
;; Python Environment Variable Binding Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; x = os.environ["VAR"]
;; x = os.environ.get("VAR")
;; x = os.getenv("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (identifier) @binding_name
  right: [
    ;; os.environ["VAR"]
    (subscript
      value: (attribute
        object: (identifier) @_module
        attribute: (identifier) @_object)
      subscript: (string
        (string_content) @bound_env_var)
      (#eq? @_module "os")
      (#eq? @_object "environ"))
    ;; os.environ.get("VAR")
    (call
      function: (attribute
        object: (attribute
          object: (identifier) @_module
          attribute: (identifier) @_object)
        attribute: (identifier) @_method)
      arguments: (argument_list
        (string
          (string_content) @bound_env_var)
        (_)?)
      (#eq? @_module "os")
      (#eq? @_object "environ")
      (#any-of? @_method "get" "pop" "setdefault"))
    ;; os.getenv("VAR")
    (call
      function: (attribute
        object: (identifier) @_module
        attribute: (identifier) @_method)
      arguments: (argument_list
        (string
          (string_content) @bound_env_var)
        (_)?)
      (#eq? @_module "os")
      (#eq? @_method "getenv"))
    
    ;; env = os.environ
    (attribute
      object: (identifier) @_module
      attribute: (identifier) @_object
      (#eq? @_module "os")
      (#eq? @_object "environ"))

    ;; env = os.environ.copy()
    (call
      function: (attribute
        object: (attribute
          object: (identifier) @_module
          attribute: (identifier) @_object)
        attribute: (identifier) @_method)
      arguments: (argument_list)
      (#eq? @_module "os")
      (#eq? @_object "environ")
      (#eq? @_method "copy"))

  ]) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; env = os.environ (object alias)
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (identifier) @binding_name
  right: [
    ;; os.environ
    (attribute
      object: (identifier) @_module
      attribute: (identifier) @_object
      (#eq? @_module "os")
      (#eq? @_object "environ"))

    ;; os.environ.copy()
    (call
      function: (attribute
        object: (attribute
          object: (identifier) @_module
          attribute: (identifier) @_object)
        attribute: (identifier) @_method)
      arguments: (argument_list)
      (#eq? @_module "os")
      (#eq? @_object "environ")
      (#eq? @_method "copy"))
  ]
) @env_object_binding

;; ═════════════════════════════════════════════════════════════════════════
;; Walrus Operator (:=) / Named Expression Patterns
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; (x := os.environ["VAR"])
;; ───────────────────────────────────────────────────────────────────────────
(named_expression
  name: (identifier) @binding_name
  value: (subscript
    value: (attribute
      object: (identifier) @_module
      attribute: (identifier) @_object)
    subscript: (string
      (string_content) @bound_env_var))
  (#eq? @_module "os")
  (#eq? @_object "environ")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; (x := os.environ.get("VAR"))
;; ───────────────────────────────────────────────────────────────────────────
(named_expression
  name: (identifier) @binding_name
  value: (call
    function: (attribute
      object: (attribute
        object: (identifier) @_module
        attribute: (identifier) @_object)
      attribute: (identifier) @_method)
    arguments: (argument_list
      (string
        (string_content) @bound_env_var)
      (_)?))
  (#eq? @_module "os")
  (#eq? @_object "environ")
  (#any-of? @_method "get" "pop" "setdefault")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; (x := os.getenv("VAR"))
;; ───────────────────────────────────────────────────────────────────────────
(named_expression
  name: (identifier) @binding_name
  value: (call
    function: (attribute
      object: (identifier) @_module
      attribute: (identifier) @_method)
    arguments: (argument_list
      (string
        (string_content) @bound_env_var)
      (_)?))
  (#eq? @_module "os")
  (#eq? @_method "getenv")) @env_binding
