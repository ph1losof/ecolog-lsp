;; ═════════════════════════════════════════════════════════════════
;; Python Scope Node Queries
;; ═════════════════════════════════════════════════════════════════
;; These patterns identify nodes that create new lexical scopes.
;; Used for building the scope hierarchy in the BindingGraph.

;; ───────────────────────────────────────────────────────────────────
;; Functions (create function scope)
;; ───────────────────────────────────────────────────────────────────
(function_definition) @scope_node

;; ───────────────────────────────────────────────────────────────────
;; Classes (create class scope)
;; ───────────────────────────────────────────────────────────────────
(class_definition) @scope_node

;; ───────────────────────────────────────────────────────────────────
;; Comprehensions (create implicit scope in Python 3)
;; ───────────────────────────────────────────────────────────────────
(list_comprehension) @scope_node
(dictionary_comprehension) @scope_node
(set_comprehension) @scope_node
(generator_expression) @scope_node

;; ───────────────────────────────────────────────────────────────────
;; Lambda (creates function scope)
;; ───────────────────────────────────────────────────────────────────
(lambda) @scope_node

;; ───────────────────────────────────────────────────────────────────
;; Control flow (for tracking variable positions)
;; Note: Python doesn't have block scope like JS, but we track
;; these for position-based lookups
;; ───────────────────────────────────────────────────────────────────
(for_statement) @scope_node
(while_statement) @scope_node
(if_statement) @scope_node
(try_statement) @scope_node
(with_statement) @scope_node
(match_statement) @scope_node
