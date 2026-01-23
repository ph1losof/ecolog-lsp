;; ───────────────────────────────────────────────────────────────────────────
;; os.environ.get("VAR") / o.environ.get("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(call
  function: (attribute
    object: (attribute
      object: (identifier) @module
      attribute: (identifier) @_object)
    attribute: (identifier) @_method)
  arguments: (argument_list
    (string
      (string_content) @env_var_name)
    (_)?)
  (#eq? @_object "environ")
  (#any-of? @_method "get" "pop" "setdefault")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; os.environ["VAR"] / o.environ["VAR"]
;; ───────────────────────────────────────────────────────────────────────────
(subscript
  value: (attribute
    object: (identifier) @module
    attribute: (identifier) @_object)
  subscript: (string
    (string_content) @env_var_name)
  (#eq? @_object "environ")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; os.getenv("VAR") / o.getenv("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(call
  function: (attribute
    object: (identifier) @module
    attribute: (identifier) @_method)
  arguments: (argument_list
    (string
      (string_content) @env_var_name)
    (_)?)
  (#eq? @_method "getenv")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; environ["VAR"] (from os import environ) / e["VAR"] (as e)
;; ───────────────────────────────────────────────────────────────────────────
(subscript
  value: (identifier) @object
  subscript: (string
    (string_content) @env_var_name)
  ;; No built-in way to know if this identifier is environ without ImportContext check
  ;; But we capture it as @object, and extract_references validates it.
) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; environ.get("VAR") / e.get("VAR")
;; ───────────────────────────────────────────────────────────────────────────
(call
  function: (attribute
    object: (identifier) @object
    attribute: (identifier) @_method)
  arguments: (argument_list
    (string
      (string_content) @env_var_name)
    (_)?)
  (#any-of? @_method "get" "pop" "setdefault")
) @env_access

;; ═════════════════════════════════════════════════════════════════════════
;; python-dotenv / decouple patterns
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; dotenv.get_key('.env', 'VAR')
;; ───────────────────────────────────────────────────────────────────────────
(call
  function: (attribute
    object: (identifier) @_module
    attribute: (identifier) @_method)
  arguments: (argument_list
    (_)
    (string
      (string_content) @env_var_name))
  (#eq? @_module "dotenv")
  (#eq? @_method "get_key")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; dotenv_values('.env')['VAR']
;; ───────────────────────────────────────────────────────────────────────────
(subscript
  value: (call
    function: (identifier) @_func
    arguments: (argument_list
      (_)?))
  subscript: (string
    (string_content) @env_var_name)
  (#eq? @_func "dotenv_values")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; decouple config('VAR') - python-decouple library
;; ───────────────────────────────────────────────────────────────────────────
(call
  function: (identifier) @_func
  arguments: (argument_list
    (string
      (string_content) @env_var_name)
    (_)?)
  (#eq? @_func "config")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; decouple.config('VAR')
;; ───────────────────────────────────────────────────────────────────────────
(call
  function: (attribute
    object: (identifier) @_module
    attribute: (identifier) @_func)
  arguments: (argument_list
    (string
      (string_content) @env_var_name)
    (_)?)
  (#eq? @_module "decouple")
  (#eq? @_func "config")) @env_access

;; ═════════════════════════════════════════════════════════════════════════
;; Function parameter default patterns
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; def connect(db=os.getenv('DB')): ...
;; ───────────────────────────────────────────────────────────────────────────
(default_parameter
  value: (call
    function: (attribute
      object: (identifier) @_module
      attribute: (identifier) @_method)
    arguments: (argument_list
      (string
        (string_content) @env_var_name)
      (_)?))
  (#eq? @_module "os")
  (#eq? @_method "getenv")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; def connect(db=os.environ.get('DB')): ...
;; ───────────────────────────────────────────────────────────────────────────
(default_parameter
  value: (call
    function: (attribute
      object: (attribute
        object: (identifier) @_module
        attribute: (identifier) @_object)
      attribute: (identifier) @_method)
    arguments: (argument_list
      (string
        (string_content) @env_var_name)
      (_)?))
  (#eq? @_module "os")
  (#eq? @_object "environ")
  (#any-of? @_method "get" "pop" "setdefault")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; def connect(db=os.environ['DB']): ...
;; ───────────────────────────────────────────────────────────────────────────
(default_parameter
  value: (subscript
    value: (attribute
      object: (identifier) @_module
      attribute: (identifier) @_object)
    subscript: (string
      (string_content) @env_var_name))
  (#eq? @_module "os")
  (#eq? @_object "environ")) @env_access

;; ═════════════════════════════════════════════════════════════════════════
;; Dictionary literal patterns
;; ═════════════════════════════════════════════════════════════════════════

;; ───────────────────────────────────────────────────────────────────────────
;; config = {'db': os.environ['DB']}
;; ───────────────────────────────────────────────────────────────────────────
(pair
  value: (subscript
    value: (attribute
      object: (identifier) @_module
      attribute: (identifier) @_attr)
    subscript: (string
      (string_content) @env_var_name))
  (#eq? @_module "os")
  (#eq? @_attr "environ")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; config = {'db': os.getenv('DB')}
;; ───────────────────────────────────────────────────────────────────────────
(pair
  value: (call
    function: (attribute
      object: (identifier) @_module
      attribute: (identifier) @_method)
    arguments: (argument_list
      (string
        (string_content) @env_var_name)
      (_)?))
  (#eq? @_module "os")
  (#eq? @_method "getenv")) @env_access

;; ───────────────────────────────────────────────────────────────────────────
;; config = {'db': os.environ.get('DB')}
;; ───────────────────────────────────────────────────────────────────────────
(pair
  value: (call
    function: (attribute
      object: (attribute
        object: (identifier) @_module
        attribute: (identifier) @_object)
      attribute: (identifier) @_method)
    arguments: (argument_list
      (string
        (string_content) @env_var_name)
      (_)?))
  (#eq? @_module "os")
  (#eq? @_object "environ")
  (#any-of? @_method "get" "pop" "setdefault")) @env_access
