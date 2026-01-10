;; ═════════════════════════════════════════════════════════════════
;; TypeScript Scope Node Queries
;; ═════════════════════════════════════════════════════════════════
;; These patterns identify nodes that create new lexical scopes.
;; Used for building the scope hierarchy in the BindingGraph.

;; ───────────────────────────────────────────────────────────────────
;; Functions (create function scope)
;; ───────────────────────────────────────────────────────────────────
(function_declaration) @scope_node
(function_expression) @scope_node
(arrow_function) @scope_node
(method_definition) @scope_node
(generator_function_declaration) @scope_node
(generator_function) @scope_node

;; ───────────────────────────────────────────────────────────────────
;; Classes (create class scope)
;; ───────────────────────────────────────────────────────────────────
(class_declaration) @scope_node
(class) @scope_node
(class_body) @scope_node

;; ───────────────────────────────────────────────────────────────────
;; Blocks (create block scope for let/const)
;; ───────────────────────────────────────────────────────────────────
(statement_block) @scope_node

;; ───────────────────────────────────────────────────────────────────
;; Loops (create loop scope)
;; ───────────────────────────────────────────────────────────────────
(for_statement) @scope_node
;; Note: for-in and for-of are included in for_statement in tree-sitter-typescript

;; ───────────────────────────────────────────────────────────────────
;; Conditionals (create conditional scope)
;; ───────────────────────────────────────────────────────────────────
(if_statement) @scope_node
(switch_statement) @scope_node
(switch_case) @scope_node

;; ───────────────────────────────────────────────────────────────────
;; Exception handling (create exception scope)
;; ───────────────────────────────────────────────────────────────────
(try_statement) @scope_node
(catch_clause) @scope_node
(finally_clause) @scope_node

;; ───────────────────────────────────────────────────────────────────
;; TypeScript-specific scopes
;; ───────────────────────────────────────────────────────────────────
(interface_declaration) @scope_node
(module) @scope_node
(type_alias_declaration) @scope_node
(enum_declaration) @scope_node
