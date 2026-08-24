;; ═════════════════════════════════════════════════════════════════════════
;; Kotlin Import Queries
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; import java.lang.System
;; import com.example.EnvConfig
;; import com.example.Thing as T
;; ───────────────────────────────────────────────────────────────────────────
(import
  (qualified_identifier) @import_path
  (identifier)? @alias_name) @import_stmt
