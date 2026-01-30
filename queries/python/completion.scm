;; Subscript with string: os.environ["VAR"] or os.environ['VAR']
(subscript
  value: (_) @object
  subscript: (string)
) @completion_target

;; os.getenv("VAR") function call
(call
  function: (attribute
    object: (identifier) @object
    attribute: (identifier) @func)
  (#eq? @object "os")
  (#eq? @func "getenv")
) @completion_target

;; environ.get/pop/setdefault("VAR") method calls
(call
  function: (attribute
    object: (_) @object
    attribute: (identifier) @func)
  (#match? @func "^(get|pop|setdefault)$")
) @completion_target

(attribute
  object: (attribute) @object
) @completion_target

;; Handle incomplete syntax: "env." parses as ERROR
;; Exclude "os" since it's a module that CONTAINS env objects, not one itself
(ERROR
  (identifier) @object
  (#not-eq? @object "os")
) @completion_target

(ERROR
  (attribute) @object
) @completion_target

;; Handle incomplete subscript: os.environ[' or os.environ["
;; Tree-sitter creates ERROR nodes when string is incomplete
(ERROR
  (subscript
    value: (_) @object)
) @completion_target
