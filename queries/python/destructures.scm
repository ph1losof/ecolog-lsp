;; ═════════════════════════════════════════════════════════════════════════
;; Python Destructuring Pattern Queries (from env aliases)
;; ═════════════════════════════════════════════════════════════════════════
;; These patterns capture destructuring from arbitrary identifiers,
;; allowing tracking like: env = os.environ; val = env['KEY']
;;
;; Note: Direct destructuring from os.environ is handled in bindings.scm

;; ───────────────────────────────────────────────────────────────────────────
;; val = identifier['KEY'] (subscript access from alias)
;; e.g., val = env['DATABASE_URL'] where env is an alias to os.environ
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (identifier) @destructure_target
  right: (subscript
    value: (identifier) @destructure_source
    subscript: (string
      (string_content) @destructure_key))) @destructure

;; ───────────────────────────────────────────────────────────────────────────
;; val = identifier.get('KEY') (method call from alias)
;; e.g., val = env.get('DATABASE_URL') where env is an alias
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (identifier) @destructure_target
  right: (call
    function: (attribute
      object: (identifier) @destructure_source
      attribute: (identifier) @_method)
    arguments: (argument_list
      (string
        (string_content) @destructure_key)
      (_)?))
  (#any-of? @_method "get" "pop" "setdefault")) @destructure

;; ───────────────────────────────────────────────────────────────────────────
;; Tuple unpacking: db, api = env['DB'], env['API']
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (pattern_list
    (identifier) @destructure_target)
  right: (expression_list
    (subscript
      value: (identifier) @destructure_source
      subscript: (string
        (string_content) @destructure_key)))) @destructure

;; ───────────────────────────────────────────────────────────────────────────
;; Walrus operator: (val := identifier['KEY'])
;; e.g., if (db := env['DATABASE_URL']): ...
;; ───────────────────────────────────────────────────────────────────────────
(named_expression
  name: (identifier) @destructure_target
  value: (subscript
    value: (identifier) @destructure_source
    subscript: (string
      (string_content) @destructure_key))) @destructure

;; ───────────────────────────────────────────────────────────────────────────
;; Walrus operator with get: (val := identifier.get('KEY'))
;; e.g., if (db := env.get('DATABASE_URL')): ...
;; ───────────────────────────────────────────────────────────────────────────
(named_expression
  name: (identifier) @destructure_target
  value: (call
    function: (attribute
      object: (identifier) @destructure_source
      attribute: (identifier) @_method)
    arguments: (argument_list
      (string
        (string_content) @destructure_key)
      (_)?))
  (#any-of? @_method "get" "pop" "setdefault")) @destructure
