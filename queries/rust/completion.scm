;; std::env::var / std::env::var_os
(call_expression
  function: (scoped_identifier
    path: (scoped_identifier
      path: (identifier) @p1
      name: (identifier) @object)
    name: (identifier) @func)
  (#eq? @p1 "std")
  (#eq? @object "env")
  (#match? @func "^(var|var_os)$")
) @completion_target

;; env::var / env::var_os
(call_expression
  function: (scoped_identifier
    path: (identifier) @object
    name: (identifier) @func)
  (#eq? @object "env")
  (#match? @func "^(var|var_os)$")
) @completion_target

(macro_invocation
  macro: (identifier) @object
  (#eq? @object "option_env")
) @completion_target

(macro_invocation
  macro: (identifier) @object
  (#eq? @object "env")
) @completion_target
