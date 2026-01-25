;; ═════════════════════════════════════════════════════════════════════════
;; Lua Environment Variable Reference Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; Primary pattern: os.getenv("VAR")
;; Lua only has os.getenv() for environment variable access - no dict-style
;; or property-style access patterns.

;; ───────────────────────────────────────────────────────────────────────────
;; os.getenv("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(function_call
  name: (dot_index_expression
    table: (identifier) @object
    field: (identifier) @_func)
  arguments: (arguments
    (string
      content: (string_content) @env_var_name))
  (#eq? @object "os")
  (#eq? @_func "getenv")) @env_access

