(subscript
  value: (_) @object
) @completion_target

(call
  function: (attribute
    object: (_) @object
    attribute: (identifier) @func)
  (#eq? @func "get")
) @completion_target

(attribute
  object: (_) @object
) @completion_target

;; Handle incomplete syntax: "env." parses as ERROR
(ERROR
  (identifier) @object
) @completion_target

(ERROR
  (attribute) @object
) @completion_target
