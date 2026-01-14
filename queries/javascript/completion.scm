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

; Nested subscript - captures "process.env" from "process.env["X"]"
(subscript_expression
  object: (member_expression) @object
) @completion_target

; Generic member_expression - captures "env" from "env.X"
(member_expression
  object: (_) @object
) @completion_target

; Generic subscript - captures "env" from "env["X"]"
(subscript_expression
  object: (_) @object
) @completion_target
