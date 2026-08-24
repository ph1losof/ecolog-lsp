;; ═════════════════════════════════════════════════════════════════════════
;; Zig Export Queries
;; ═════════════════════════════════════════════════════════════════════════
;; Zig uses the `pub` keyword for exports. `pub` is an anonymous token in this
;; grammar, so it is matched as a string rather than as a named node.

;; ───────────────────────────────────────────────────────────────────────────
;; pub fn foo() {}
;; ───────────────────────────────────────────────────────────────────────────
(function_declaration
  "pub"
  name: (identifier) @export_name) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; pub const x = ...;
;; ───────────────────────────────────────────────────────────────────────────
(variable_declaration
  "pub"
  (identifier) @export_name) @export_stmt
