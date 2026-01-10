;; ═════════════════════════════════════════════════════════════════
;; Go Scope Node Queries
;; ═════════════════════════════════════════════════════════════════
;; These patterns identify nodes that create new lexical scopes.
;; Used for building the scope hierarchy in the BindingGraph.

;; ───────────────────────────────────────────────────────────────────
;; Functions
;; ───────────────────────────────────────────────────────────────────
(function_declaration) @scope_node
(method_declaration) @scope_node
(func_literal) @scope_node

;; ───────────────────────────────────────────────────────────────────
;; Blocks
;; ───────────────────────────────────────────────────────────────────
(block) @scope_node

;; ───────────────────────────────────────────────────────────────────
;; Loops
;; ───────────────────────────────────────────────────────────────────
(for_statement) @scope_node

;; ───────────────────────────────────────────────────────────────────
;; Conditionals
;; ───────────────────────────────────────────────────────────────────
(if_statement) @scope_node
(switch_statement) @scope_node
(expression_switch_statement) @scope_node
(type_switch_statement) @scope_node

;; ───────────────────────────────────────────────────────────────────
;; Select (channel operations)
;; ───────────────────────────────────────────────────────────────────
(select_statement) @scope_node
(communication_case) @scope_node
(expression_case) @scope_node
(default_case) @scope_node
(type_case) @scope_node
