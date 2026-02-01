;; ═════════════════════════════════════════════════════════════════════════
;; PHP Destructure Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; PHP uses list() or [] shorthand for array destructuring.
;; Both create list_literal nodes.

;; ───────────────────────────────────────────────────────────────────────────
;; list($a, $b) = $arr or [$a, $b] = $arr
;; ───────────────────────────────────────────────────────────────────────────
(assignment_expression
  left: (list_literal
    (variable_name) @destructure_target)
  right: (variable_name) @destructure_source) @destructure

;; ───────────────────────────────────────────────────────────────────────────
;; Array access as destructure-like pattern: $x = $arr['key']
;; ───────────────────────────────────────────────────────────────────────────
(assignment_expression
  left: (variable_name) @destructure_target
  right: (subscript_expression
    (variable_name) @destructure_source
    (string
      (string_content) @destructure_key))) @destructure
