;; ═════════════════════════════════════════════════════════════════════════
;; Java Export Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; public class Foo {}
;; ───────────────────────────────────────────────────────────────────────────
(class_declaration
  (modifiers) @_mods
  name: (identifier) @export_name
  (#match? @_mods "public")) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; public interface Foo {}
;; ───────────────────────────────────────────────────────────────────────────
(interface_declaration
  (modifiers) @_mods
  name: (identifier) @export_name
  (#match? @_mods "public")) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; public void method() {}
;; ───────────────────────────────────────────────────────────────────────────
(method_declaration
  (modifiers) @_mods
  name: (identifier) @export_name
  (#match? @_mods "public")) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; public static final String CONSTANT = "value";
;; ───────────────────────────────────────────────────────────────────────────
(field_declaration
  (modifiers) @_mods
  declarator: (variable_declarator
    name: (identifier) @export_name)
  (#match? @_mods "public")) @export_stmt
