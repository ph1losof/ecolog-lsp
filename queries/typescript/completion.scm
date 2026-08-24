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

; NOTE: `import.meta.env.VAR` and `import.meta.env["VAR"]` are matched by the
; generic patterns below -- `import.meta` is a single `meta_property` node, so
; the object of `import.meta.env` is an ordinary member expression.

; Generic member_expression - captures "env" from "env.X"
(member_expression
  object: (_) @object
) @completion_target

; Generic subscript with string - captures "env" from env["X"]
(subscript_expression
  object: (_) @object
  index: (string)
) @completion_target
