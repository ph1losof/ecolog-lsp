;; ═════════════════════════════════════════════════════════════════════════
;; Elixir Export Queries
;; ═════════════════════════════════════════════════════════════════════════
;; Elixir modules define exports via def/defp (public/private functions)

;; ───────────────────────────────────────────────────────────────────────────
;; def function_name(args) do ... end (public function)
;; ───────────────────────────────────────────────────────────────────────────
(call
  target: (identifier) @_def
  (arguments
    (call
      target: (identifier) @export_name))
  (#eq? @_def "def")) @export_stmt

;; ───────────────────────────────────────────────────────────────────────────
;; defmodule Module do ... end
;; ───────────────────────────────────────────────────────────────────────────
(call
  target: (identifier) @_defmodule
  (arguments
    (alias) @export_name)
  (#eq? @_defmodule "defmodule")) @export_stmt
