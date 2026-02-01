;; ═════════════════════════════════════════════════════════════════════════
;; Ruby Environment Variable Completion Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; ENV['[cursor]'] - cursor inside string
;; ───────────────────────────────────────────────────────────────────────────
(element_reference
  object: (constant) @object
  (string) @completion_target
  (#eq? @object "ENV"))

;; ───────────────────────────────────────────────────────────────────────────
;; ENV.fetch('[cursor]') - method call with string
;; ───────────────────────────────────────────────────────────────────────────
(call
  receiver: (constant) @object
  method: (identifier) @_method
  arguments: (argument_list
    (string) @completion_target)
  (#eq? @object "ENV")
  (#eq? @_method "fetch"))

;; ───────────────────────────────────────────────────────────────────────────
;; ENV[ - incomplete, no string yet
;; ───────────────────────────────────────────────────────────────────────────
(element_reference
  object: (constant) @object
  (#eq? @object "ENV")) @completion_target

;; ───────────────────────────────────────────────────────────────────────────
;; ERROR nodes - incomplete input during typing
;; ───────────────────────────────────────────────────────────────────────────
(ERROR) @completion_target
