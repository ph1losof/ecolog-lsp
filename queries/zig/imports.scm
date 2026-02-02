;; ═════════════════════════════════════════════════════════════════════════
;; Zig Import Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; const std = @import("std");
;; ───────────────────────────────────────────────────────────────────────────
(variable_declaration
  (identifier) @alias_name
  (builtin_function
    (builtin_identifier) @_builtin
    (arguments
      (string
        (string_content) @import_path)))
  (#eq? @_builtin "@import")) @import_stmt
