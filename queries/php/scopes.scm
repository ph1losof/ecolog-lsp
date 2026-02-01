;; ═════════════════════════════════════════════════════════════════════════
;; PHP Scope Node Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; These patterns identify nodes that create new lexical scopes.

;; ───────────────────────────────────────────────────────────────────────────
;; Functions
;; ───────────────────────────────────────────────────────────────────────────
(function_definition) @scope_node
(method_declaration) @scope_node
(anonymous_function) @scope_node
(arrow_function) @scope_node

;; ───────────────────────────────────────────────────────────────────────────
;; Classes
;; ───────────────────────────────────────────────────────────────────────────
(class_declaration) @scope_node

;; ───────────────────────────────────────────────────────────────────────────
;; Loops
;; ───────────────────────────────────────────────────────────────────────────
(for_statement) @scope_node
(foreach_statement) @scope_node
(while_statement) @scope_node
(do_statement) @scope_node

;; ───────────────────────────────────────────────────────────────────────────
;; Conditionals
;; ───────────────────────────────────────────────────────────────────────────
(if_statement) @scope_node
(switch_statement) @scope_node
(try_statement) @scope_node
