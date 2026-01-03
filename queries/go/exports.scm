;; ═══════════════════════════════════════════════════════════════════════════
;; Go Export Queries
;; ═══════════════════════════════════════════════════════════════════════════
;;
;; Go uses capitalization for exports - any identifier starting with an
;; uppercase letter is automatically exported from the package.
;;
;; We capture all package-level declarations. The capitalization check
;; is performed in post-processing (in Rust code) since tree-sitter
;; doesn't support regex in predicates.
;;
;; Captures:
;;   @export_name    - The exported identifier name (capitalization checked later)
;;   @export_value   - The value being exported (optional)
;;   @export_stmt    - The entire declaration

;; ───────────────────────────────────────────────────────────────────────────
;; Package-level const declarations
;; const Foo = "value" or const Foo string = "value"
;; ───────────────────────────────────────────────────────────────────────────
(const_declaration
  (const_spec
    name: (identifier) @export_name
    value: (expression_list
      (_) @export_value)?)) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; Package-level var declarations
;; var Foo = "value" or var Foo string = "value"
;; ───────────────────────────────────────────────────────────────────────────
(var_declaration
  (var_spec
    name: (identifier) @export_name
    value: (expression_list
      (_) @export_value)?)) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; Package-level function declarations
;; func Foo() {}
;; ───────────────────────────────────────────────────────────────────────────
(function_declaration
  name: (identifier) @export_name) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; Method declarations (exported if receiver type and method are capitalized)
;; func (r *Receiver) Method() {}
;; ───────────────────────────────────────────────────────────────────────────
(method_declaration
  name: (field_identifier) @export_name) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; Type declarations
;; type Foo struct {} or type Foo = OtherType
;; ───────────────────────────────────────────────────────────────────────────
(type_declaration
  (type_spec
    name: (type_identifier) @export_name)) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; Type alias declarations
;; type Foo = Bar
;; ───────────────────────────────────────────────────────────────────────────
(type_declaration
  (type_alias
    name: (type_identifier) @export_name)) @export_stmt
