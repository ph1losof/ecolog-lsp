;; ═════════════════════════════════════════════════════════════════════════
;; C++ Destructure Queries
;; ═════════════════════════════════════════════════════════════════════════
;; C++ has structured bindings (C++17) but they're typically not used for
;; env var access. This file is intentionally minimal.

;; ───────────────────────────────────────────────────────────────────────────
;; auto [a, b] = pair; (structured binding - C++17)
;; ───────────────────────────────────────────────────────────────────────────
(declaration
  declarator: (structured_binding_declarator
    (identifier) @destructure_key)) @destructure
