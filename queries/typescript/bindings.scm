;; ═════════════════════════════════════════════════════════════════════════
;; TypeScript Environment Variable Binding Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; const/let/var x = process.env.VAR
;; ───────────────────────────────────────────────────────────────────────────
(variable_declarator
  name: (identifier) @binding_name
  value: (member_expression
    object: (member_expression
      object: (identifier) @_object
      property: (property_identifier) @_property)
    property: (property_identifier) @bound_env_var)
  (#eq? @_object "process")
  (#eq? @_property "env")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; const/let/var x = process.env["VAR"]
;; ───────────────────────────────────────────────────────────────────────────
(variable_declarator
  name: (identifier) @binding_name
  value: (subscript_expression
    object: (member_expression
      object: (identifier) @_object
      property: (property_identifier) @_property)
    index: (string
      (string_fragment) @bound_env_var))
  (#eq? @_object "process")
  (#eq? @_property "env")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; const { VAR } = process.env (destructuring)
;; ───────────────────────────────────────────────────────────────────────────
(variable_declarator
  name: (object_pattern
    (shorthand_property_identifier_pattern) @binding_name @bound_env_var)
  value: (member_expression
    object: (identifier) @_object
    property: (property_identifier) @_property)
  (#eq? @_object "process")
  (#eq? @_property "env")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; const/let/var x = process.env.VAR as string
;; ───────────────────────────────────────────────────────────────────────────
(variable_declarator
  name: (identifier) @binding_name
  value: (as_expression
    (member_expression
      object: (member_expression
        object: (identifier) @_object
        property: (property_identifier) @_property)
      property: (property_identifier) @bound_env_var))
  (#eq? @_object "process")
  (#eq? @_property "env")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; const/let/var x = process.env["VAR"] as string
;; ───────────────────────────────────────────────────────────────────────────
(variable_declarator
  name: (identifier) @binding_name
  value: (as_expression
    (subscript_expression
      object: (member_expression
        object: (identifier) @_object
        property: (property_identifier) @_property)
      index: (string
        (string_fragment) @bound_env_var)))
  (#eq? @_object "process")
  (#eq? @_property "env")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; const/let/var x = process.env.VAR! (non-null assertion)
;; ───────────────────────────────────────────────────────────────────────────
(variable_declarator
  name: (identifier) @binding_name
  value: (non_null_expression
    (member_expression
      object: (member_expression
        object: (identifier) @_object
        property: (property_identifier) @_property)
      property: (property_identifier) @bound_env_var))
  (#eq? @_object "process")
  (#eq? @_property "env")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; const/let/var x = process.env["VAR"]!
;; ───────────────────────────────────────────────────────────────────────────
(variable_declarator
  name: (identifier) @binding_name
  value: (non_null_expression
    (subscript_expression
      object: (member_expression
        object: (identifier) @_object
        property: (property_identifier) @_property)
      index: (string
        (string_fragment) @bound_env_var)))
  (#eq? @_object "process")
  (#eq? @_property "env")) @env_binding

;; ───────────────────────────────────────────────────────────────────────────
;; const env = process.env (object alias)
;; ───────────────────────────────────────────────────────────────────────────
(variable_declarator
  name: (identifier) @binding_name
  value: (member_expression
    object: (identifier) @_object
    property: (property_identifier) @_property)
  (#eq? @_object "process")
  (#eq? @_property "env")) @env_object_binding

;; ───────────────────────────────────────────────────────────────────
;; const { VAR: myVar } = process.env (renamed destructuring)
;; ───────────────────────────────────────────────────────────────────
(variable_declarator
  name: (object_pattern
    (pair_pattern
      key: (property_identifier) @bound_env_var
      value: (identifier) @binding_name))
  value: (member_expression
    object: (identifier) @_object
    property: (property_identifier) @_property)
  (#eq? @_object "process")
  (#eq? @_property "env")) @env_binding

;; ───────────────────────────────────────────────────────────────────
;; const { VAR: myVar = "default" } = process.env (with default)
;; ───────────────────────────────────────────────────────────────────
(variable_declarator
  name: (object_pattern
    (pair_pattern
      key: (property_identifier) @bound_env_var
      value: (assignment_pattern
        left: (identifier) @binding_name
        right: (_))))
  value: (member_expression
    object: (identifier) @_object
    property: (property_identifier) @_property)
  (#eq? @_object "process")
  (#eq? @_property "env")) @env_binding
