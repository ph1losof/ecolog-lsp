;; ═════════════════════════════════════════════════════════════════════════
;; PHP Export Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; PHP doesn't have explicit exports like ES modules.
;; Classes, functions, and constants are implicitly available when included.

;; ───────────────────────────────────────────────────────────────────────────
;; Class declarations (implicitly exported)
;; ───────────────────────────────────────────────────────────────────────────
(class_declaration
  name: (name) @export_name) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; Function definitions at top level (implicitly exported)
;; ───────────────────────────────────────────────────────────────────────────
(function_definition
  name: (name) @export_name) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; Constant definitions (implicitly exported)
;; Note: const_element has a direct name child, not a named field
;; ───────────────────────────────────────────────────────────────────────────
(const_declaration
  (const_element
    (name) @export_name)) @export_stmt
