; ERROR node patterns (for incomplete syntax)
(ERROR
  (identifier) @object
) @completion_target

(ERROR
  (member_expression) @object
) @completion_target

; Nested member_expression - captures "process.env" from "process.env.X"
(member_expression
  object: (member_expression) @object
) @completion_target

; Nested subscript with string - captures "process.env" from process.env["X"]
(subscript_expression
  object: (member_expression) @object
  index: (string)
) @completion_target

; import.meta.env.VAR - dot access on import.meta.env
(member_expression
  object: (member_expression
    object: (member_expression
      object: (import)
      property: (property_identifier) @meta)
    property: (property_identifier) @env)
  (#eq? @meta "meta")
  (#eq? @env "env")
) @completion_target

; import.meta.env["VAR"] - subscript on import.meta.env
(subscript_expression
  object: (member_expression
    object: (member_expression
      object: (import)
      property: (property_identifier) @meta)
    property: (property_identifier) @env)
  index: (string)
  (#eq? @meta "meta")
  (#eq? @env "env")
) @completion_target

; Generic member_expression - captures "env" from "env.X"
(member_expression
  object: (_) @object
) @completion_target

; Generic subscript with string - captures "env" from env["X"]
(subscript_expression
  object: (_) @object
  index: (string)
) @completion_target
