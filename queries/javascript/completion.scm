(member_expression
  object: (_) @object
) @completion_target

(subscript_expression
  object: (_) @object
) @completion_target

(ERROR
  (identifier) @object
) @completion_target

(ERROR
  (member_expression) @object
) @completion_target
