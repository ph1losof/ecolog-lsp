;; ═══════════════════════════════════════════════════════════════════════════
;; Rust Export Queries
;; ═══════════════════════════════════════════════════════════════════════════
;;
;; Rust exports are items with `pub` visibility at module level.
;; We track pub const, pub static, pub fn, and pub use re-exports.
;;
;; Captures:
;;   @export_name    - The exported identifier name
;;   @export_value   - The value being exported (optional)
;;   @export_stmt    - The entire declaration
;;   @visibility     - The visibility modifier (pub, pub(crate), etc.)
;;   @reexport_path  - Path for pub use re-exports

;; ───────────────────────────────────────────────────────────────────────────
;; pub const FOO: Type = value;
;; ───────────────────────────────────────────────────────────────────────────
(const_item
  (visibility_modifier) @visibility
  name: (identifier) @export_name
  value: (_) @export_value) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; pub static FOO: Type = value;
;; ───────────────────────────────────────────────────────────────────────────
(static_item
  (visibility_modifier) @visibility
  name: (identifier) @export_name
  value: (_) @export_value) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; pub fn foo() {} (function export)
;; ───────────────────────────────────────────────────────────────────────────
(function_item
  (visibility_modifier) @visibility
  name: (identifier) @export_name) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; pub struct Foo {} (struct export)
;; ───────────────────────────────────────────────────────────────────────────
(struct_item
  (visibility_modifier) @visibility
  name: (type_identifier) @export_name) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; pub enum Foo {} (enum export)
;; ───────────────────────────────────────────────────────────────────────────
(enum_item
  (visibility_modifier) @visibility
  name: (type_identifier) @export_name) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; pub trait Foo {} (trait export)
;; ───────────────────────────────────────────────────────────────────────────
(trait_item
  (visibility_modifier) @visibility
  name: (type_identifier) @export_name) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; pub type Foo = Bar; (type alias export)
;; ───────────────────────────────────────────────────────────────────────────
(type_item
  (visibility_modifier) @visibility
  name: (type_identifier) @export_name) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; pub use crate::config::DATABASE_URL; (re-export from path)
;; ───────────────────────────────────────────────────────────────────────────
(use_declaration
  (visibility_modifier) @visibility
  argument: (scoped_identifier
    path: (_) @reexport_path
    name: (identifier) @export_name)) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; pub use crate::config::DB_URL as DATABASE_URL; (re-export with alias)
;; ───────────────────────────────────────────────────────────────────────────
(use_declaration
  (visibility_modifier) @visibility
  argument: (use_as_clause
    path: (scoped_identifier
      path: (_) @reexport_path
      name: (identifier) @local_name)
    alias: (identifier) @export_name)) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; pub use crate::config::*; (wildcard re-export)
;; ───────────────────────────────────────────────────────────────────────────
(use_declaration
  (visibility_modifier) @visibility
  argument: (use_wildcard
    (scoped_identifier) @wildcard_source)) @wildcard_export

;; ───────────────────────────────────────────────────────────────────────────
;; pub use crate::config::{Foo, Bar}; (group re-export)
;; ───────────────────────────────────────────────────────────────────────────
(use_declaration
  (visibility_modifier) @visibility
  argument: (scoped_use_list
    path: (_) @reexport_path
    list: (use_list
      (identifier) @export_name))) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; pub mod foo; (module re-export - makes submodule publicly accessible)
;; ───────────────────────────────────────────────────────────────────────────
(mod_item
  (visibility_modifier) @visibility
  name: (identifier) @export_name) @mod_export
