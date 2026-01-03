;; ═════════════════════════════════════════════════════════════════
;; JavaScript Destructuring Pattern Queries (from identifiers)
;; ═════════════════════════════════════════════════════════════════
;; These patterns capture destructuring from arbitrary identifiers,
;; allowing tracking like: const env = process.env; const { VAR } = env;
;;
;; Note: Direct destructuring from process.env is handled in bindings.scm

;; ───────────────────────────────────────────────────────────────────
;; const { VAR } = identifier (shorthand destructuring from alias)
;; ───────────────────────────────────────────────────────────────────
(variable_declarator
  name: (object_pattern
    (shorthand_property_identifier_pattern) @destructure_target @destructure_key)
  value: (identifier) @destructure_source) @destructure

;; ───────────────────────────────────────────────────────────────────
;; const { KEY: alias } = identifier (renamed destructuring from alias)
;; ───────────────────────────────────────────────────────────────────
(variable_declarator
  name: (object_pattern
    (pair_pattern
      key: (property_identifier) @destructure_key
      value: (identifier) @destructure_target))
  value: (identifier) @destructure_source) @destructure

;; ───────────────────────────────────────────────────────────────────
;; const { KEY: alias = default } = identifier (with default value)
;; ───────────────────────────────────────────────────────────────────
(variable_declarator
  name: (object_pattern
    (pair_pattern
      key: (property_identifier) @destructure_key
      value: (assignment_pattern
        left: (identifier) @destructure_target
        right: (_))))
  value: (identifier) @destructure_source) @destructure

;; ───────────────────────────────────────────────────────────────────
;; const { VAR = default } = identifier (shorthand with default)
;; ───────────────────────────────────────────────────────────────────
(variable_declarator
  name: (object_pattern
    (object_assignment_pattern
      (shorthand_property_identifier_pattern) @destructure_target @destructure_key
      (_)))
  value: (identifier) @destructure_source) @destructure

;; ───────────────────────────────────────────────────────────────────
;; let { VAR } = identifier (also for let/var declarations)
;; ───────────────────────────────────────────────────────────────────
;; Note: The above patterns already match const/let/var because
;; variable_declarator is used regardless of the declaration keyword.
