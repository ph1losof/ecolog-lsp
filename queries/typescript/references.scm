;; ═════════════════════════════════════════════════════════════════════════
;; TypeScript Environment Variable Reference Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; process.env.VAR_NAME (dot notation)
;; ───────────────────────────────────────────────────────────────────────────
(member_expression
  object: (member_expression
    object: (identifier) @_object
    property: (property_identifier) @_property)
  property: (property_identifier) @env_var_name
  (#eq? @_object "process")
  (#eq? @_property "env")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; process.env["VAR_NAME"] (bracket notation)
;; ───────────────────────────────────────────────────────────────────────────
(subscript_expression
  object: (member_expression
    object: (identifier) @_object
    property: (property_identifier) @_property)
  index: (string
    (string_fragment) @env_var_name)
  (#eq? @_object "process")
  (#eq? @_property "env")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; import.meta.env.VAR_NAME (Vite/ESM)
;; ───────────────────────────────────────────────────────────────────────────
(member_expression
  object: (member_expression
    object: (member_expression
      object: (import)
      property: (property_identifier) @_meta)
    property: (property_identifier) @_env)
  property: (property_identifier) @env_var_name
  (#eq? @_meta "meta")
  (#eq? @_env "env")) @env_access
