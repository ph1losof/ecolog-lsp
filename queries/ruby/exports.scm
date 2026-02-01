;; ═════════════════════════════════════════════════════════════════════════
;; Ruby Export Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; Ruby doesn't have explicit exports like ES modules.
;; Classes and modules are implicitly available when required.

;; ───────────────────────────────────────────────────────────────────────────
;; Class definitions (implicitly exported)
;; ───────────────────────────────────────────────────────────────────────────
(class
  name: (constant) @export_name) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; Module definitions (implicitly exported)
;; ───────────────────────────────────────────────────────────────────────────
(module
  name: (constant) @export_name) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; Method definitions at top level
;; ───────────────────────────────────────────────────────────────────────────
(program
  (method
    name: (identifier) @export_name)) @export_stmt
