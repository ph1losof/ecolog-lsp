;; ═════════════════════════════════════════════════════════════════════════
;; C# Destructure Queries
;; ═════════════════════════════════════════════════════════════════════════
;;
;; Intentionally empty.
;;
;; A destructure is only meaningful here when the source resolves to an
;; environment *object* whose properties are variables, the way
;; `const { PORT } = process.env` works in JavaScript. C# exposes the
;; environment through `Environment.GetEnvironmentVariable(...)` calls and
;; through an `IDictionary` that is read by indexing, not by destructuring.
;; Tuple deconstruction (`var (a, b) = GetTuple();`) is positional and carries
;; no key to match against a variable name, so there is nothing to bind.
