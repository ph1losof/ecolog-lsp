;; ═════════════════════════════════════════════════════════════════════════
;; C Export Queries
;; ═════════════════════════════════════════════════════════════════════════
;; C uses header files for exports. Functions and variables at file scope
;; without static are implicitly exported.

;; ───────────────────────────────────────────────────────────────────────────
;; Function definitions (non-static functions are exported)
;; ───────────────────────────────────────────────────────────────────────────
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @export_name)) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; Global variable declarations
;; ───────────────────────────────────────────────────────────────────────────
(declaration
  declarator: (init_declarator
    declarator: (identifier) @export_name)) @export_stmt
