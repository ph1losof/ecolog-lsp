(call_expression
  function: (selector_expression
    operand: (identifier) @object
    field: (field_identifier) @func)
  (#eq? @object "os")
  (#match? @func "^(Getenv|LookupEnv|Setenv|Unsetenv)$")
) @completion_target
