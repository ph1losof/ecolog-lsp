;; ═════════════════════════════════════════════════════════════════════════
;; Ruby Import Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; Ruby uses require and require_relative for imports.

;; ───────────────────────────────────────────────────────────────────────────
;; require 'file'
;; ───────────────────────────────────────────────────────────────────────────
(call
  method: (identifier) @_func
  arguments: (argument_list
    (string
      (string_content) @import_path))
  (#eq? @_func "require")) @import_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; require_relative 'file'
;; ───────────────────────────────────────────────────────────────────────────
(call
  method: (identifier) @_func
  arguments: (argument_list
    (string
      (string_content) @import_path))
  (#eq? @_func "require_relative")) @import_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; load 'file'
;; ───────────────────────────────────────────────────────────────────────────
(call
  method: (identifier) @_func
  arguments: (argument_list
    (string
      (string_content) @import_path))
  (#eq? @_func "load")) @import_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; Bundler.require
;; ───────────────────────────────────────────────────────────────────────────
(call
  receiver: (constant) @_receiver
  method: (identifier) @_func
  (#eq? @_receiver "Bundler")
  (#eq? @_func "require")) @import_stmt
