;; ═════════════════════════════════════════════════════════════════════════
;; Ruby Destructure Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; Ruby supports parallel assignment (destructuring).

;; ───────────────────────────────────────────────────────────────────────────
;; a, b = arr (parallel assignment / multiple assignment)
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (left_assignment_list
    (identifier) @destructure_target)
  right: (identifier) @destructure_source) @destructure

;; ───────────────────────────────────────────────────────────────────────────
;; a, b = *arr (splat assignment)
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (left_assignment_list
    (identifier) @destructure_target)
  right: (splat_argument
    (identifier) @destructure_source)) @destructure

;; ───────────────────────────────────────────────────────────────────────────
;; Hash access as destructure-like pattern: x = hash[:key]
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (identifier) @destructure_target
  right: (element_reference
    object: (identifier) @destructure_source
    (simple_symbol) @destructure_key)) @destructure

;; ───────────────────────────────────────────────────────────────────────────
;; Hash access with string key: x = hash['key']
;; ───────────────────────────────────────────────────────────────────────────
(assignment
  left: (identifier) @destructure_target
  right: (element_reference
    object: (identifier) @destructure_source
    (string
      (string_content) @destructure_key))) @destructure
