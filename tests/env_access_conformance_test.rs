//! Cross-language conformance for environment variable access forms.
//!
//! Two things are pinned here:
//!
//! 1. Every access form a language's queries claim to support actually resolves.
//! 2. `AnalysisMode::Index` sees exactly what `AnalysisMode::Full` sees. Workspace
//!    indexing runs in `Index` mode and skips work that interactive requests need;
//!    any divergence means a file is indexed as referencing the wrong set of
//!    variables, which silently breaks cross-file references and rename.

use ecolog_lsp::analysis::{AnalysisMode, AnalysisPipeline, BindingResolver, QueryEngine};
use ecolog_lsp::languages::{LanguageRegistry, LanguageSupport};
use std::sync::Arc;

fn language(id: &str) -> Arc<dyn LanguageSupport> {
    LanguageRegistry::with_all_languages()
        .get_by_language_id(id)
        .unwrap_or_else(|| panic!("no language registered for {}", id))
}

/// Resolves `source` in both analysis modes and returns the sorted env vars.
///
/// Panics if the two modes disagree.
async fn env_vars(id: &str, source: &str) -> Vec<String> {
    let lang = language(id);
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang.grammar()).expect("set grammar");
    let tree = parser.parse(source, None).expect("parse");
    let engine = QueryEngine::new();

    let mut per_mode = Vec::new();
    for mode in [AnalysisMode::Full, AnalysisMode::Index] {
        let graph = AnalysisPipeline::analyze_with_mode(
            &engine,
            lang.as_ref(),
            &tree,
            source.as_bytes(),
            &Default::default(),
            mode,
        )
        .await;
        let mut vars: Vec<String> = BindingResolver::new(&graph)
            .all_env_vars()
            .iter()
            .map(|v| v.to_string())
            .collect();
        vars.sort();
        per_mode.push(vars);
    }

    assert_eq!(
        per_mode[0], per_mode[1],
        "Full and Index analysis disagree for {} on:\n{}",
        id, source
    );
    per_mode.remove(0)
}

async fn assert_finds(id: &str, source: &str, expected: &str) {
    let vars = env_vars(id, source).await;
    assert!(
        vars.iter().any(|v| v == expected),
        "{}: expected to find {:?} in {:?}\nsource:\n{}",
        id,
        expected,
        vars,
        source
    );
}

// ───────────────────────────────────────────────────────────────────────────
// Vite / ESM `import.meta.env`
//
// `import.meta` is a single `meta_property` node. The queries previously
// expected a member expression over an `(import)` node, so none of these forms
// resolved at all.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_import_meta_env_all_forms() {
    for id in ["javascript", "typescript"] {
        assert_finds(id, "const a = import.meta.env.VITE_K;", "VITE_K").await;
        assert_finds(id, "const a = import.meta.env[\"VITE_K\"];", "VITE_K").await;
        assert_finds(id, "console.log(import.meta.env.VITE_K);", "VITE_K").await;
        assert_finds(id, "const { VITE_K } = import.meta.env;", "VITE_K").await;
        assert_finds(id, "const { VITE_K: a } = import.meta.env;", "VITE_K").await;
        assert_finds(id, "const { VITE_K: a = 'd' } = import.meta.env;", "VITE_K").await;
        assert_finds(
            id,
            "const e = import.meta.env;\nconsole.log(e.VITE_K);",
            "VITE_K",
        )
        .await;
    }
}

#[tokio::test]
async fn test_import_meta_env_typescript_wrappers() {
    assert_finds("typescript", "const a = import.meta.env.VITE_K as string;", "VITE_K").await;
    assert_finds("typescript", "const a = import.meta.env.VITE_K!;", "VITE_K").await;
    assert_finds(
        "typescript",
        "const a = import.meta.env[\"VITE_K\"] as string;",
        "VITE_K",
    )
    .await;
}

// ───────────────────────────────────────────────────────────────────────────
// Reads through an env-object alias.
//
// These are recorded as usages rather than symbols, so they were invisible to
// the workspace index -- and the collection only understood JavaScript node
// kinds, so no other language resolved them at all.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_env_object_alias_reads() {
    assert_finds(
        "javascript",
        "const e = process.env;\nconsole.log(e.PORT);",
        "PORT",
    )
    .await;
    assert_finds(
        "javascript",
        "const e = process.env;\nconsole.log(e[\"PORT\"]);",
        "PORT",
    )
    .await;
    assert_finds(
        "typescript",
        "const e = process.env;\nconsole.log(e.PORT);",
        "PORT",
    )
    .await;
    assert_finds(
        "python",
        "import os\ne = os.environ\nprint(e['PORT'])",
        "PORT",
    )
    .await;
    assert_finds(
        "python",
        "import os\ne = os.environ\nprint(e.get('PORT'))",
        "PORT",
    )
    .await;
    assert_finds(
        "python",
        "import os\ne = os.environ.copy()\nprint(e['PORT'])",
        "PORT",
    )
    .await;
    assert_finds("ruby", "e = ENV\nputs e['PORT']", "PORT").await;
    assert_finds("ruby", "e = ENV\nputs e.fetch('PORT')", "PORT").await;
    assert_finds("php", "<?php $e = $_ENV; echo $e['PORT'];", "PORT").await;
    assert_finds("php", "<?php $s = $_SERVER; echo $s['PORT'];", "PORT").await;
}

// ───────────────────────────────────────────────────────────────────────────
// Bindings that references already understood.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_qualified_and_alternate_binding_forms() {
    assert_finds(
        "csharp",
        "class C { void M() { var a = System.Environment.GetEnvironmentVariable(\"K\"); } }",
        "K",
    )
    .await;
    assert_finds(
        "csharp",
        "class C { string _a = System.Environment.GetEnvironmentVariable(\"K\"); }",
        "K",
    )
    .await;
    assert_finds(
        "java",
        "class C { void m() { String a = System.getProperty(\"K\"); } }",
        "K",
    )
    .await;
}

// ───────────────────────────────────────────────────────────────────────────
// Reassignment invalidation.
//
// In languages with no separate declaration syntax the binding's own
// declaration matches the reassignment query, and used to invalidate itself.
// ───────────────────────────────────────────────────────────────────────────

async fn symbol_validity(id: &str, source: &str) -> Vec<(String, bool)> {
    let lang = language(id);
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang.grammar()).expect("set grammar");
    let tree = parser.parse(source, None).expect("parse");
    let graph = AnalysisPipeline::analyze(
        &QueryEngine::new(),
        lang.as_ref(),
        &tree,
        source.as_bytes(),
        &Default::default(),
    )
    .await;
    graph
        .symbols()
        .iter()
        .map(|s| (s.name.to_string(), s.is_valid))
        .collect()
}

#[tokio::test]
async fn test_declaration_does_not_invalidate_itself() {
    for (id, source) in [
        ("php", "<?php $a = getenv('K');"),
        ("ruby", "a = ENV['K']"),
        ("javascript", "let a = process.env.K;"),
    ] {
        let syms = symbol_validity(id, source).await;
        assert!(
            syms.iter().any(|(_, valid)| *valid),
            "{}: declaration invalidated itself: {:?}",
            id,
            syms
        );
    }
}

#[tokio::test]
async fn test_later_reassignment_still_invalidates() {
    for (id, source) in [
        ("php", "<?php $a = getenv('K'); $a = 'other';"),
        ("ruby", "a = ENV['K']\na = 'other'"),
        ("javascript", "let a = process.env.K;\na = 'other';"),
    ] {
        let syms = symbol_validity(id, source).await;
        assert!(
            syms.iter().all(|(_, valid)| !*valid),
            "{}: reassignment did not invalidate: {:?}",
            id,
            syms
        );
    }
}

#[tokio::test]
async fn test_assignment_before_declaration_does_not_invalidate() {
    let syms = symbol_validity("javascript", "let a = 'x';\nlet b = process.env.K;").await;
    assert!(
        syms.iter().any(|(name, valid)| name == "b" && *valid),
        "a prior assignment invalidated a later binding: {:?}",
        syms
    );
}

// ───────────────────────────────────────────────────────────────────────────
// Baseline forms, guarding against regressions in the common paths.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_direct_access_baselines() {
    assert_finds("javascript", "const a = process.env.K;", "K").await;
    assert_finds("javascript", "const { K } = process.env;", "K").await;
    assert_finds("python", "import os\nx = os.environ['K']", "K").await;
    assert_finds("python", "import os\nx = os.getenv('K')", "K").await;
    assert_finds("go", "package m\nimport \"os\"\nfunc f(){ os.Getenv(\"K\") }", "K").await;
    assert_finds("rust", "fn f(){ std::env::var(\"K\").unwrap(); }", "K").await;
    assert_finds("ruby", "x = ENV['K']", "K").await;
    assert_finds("php", "<?php echo $_ENV['K'];", "K").await;
    assert_finds("java", "class C { void m(){ System.getenv(\"K\"); } }", "K").await;
    assert_finds("lua", "local x = os.getenv('K')", "K").await;
    assert_finds("c", "int main(){ getenv(\"K\"); }", "K").await;
    assert_finds("cpp", "int main(){ std::getenv(\"K\"); }", "K").await;
    assert_finds("kotlin", "fun m(){ System.getenv(\"K\") }", "K").await;
    assert_finds("elixir", "System.get_env(\"K\")", "K").await;
    assert_finds("bash", "echo $K", "K").await;
}

// ───────────────────────────────────────────────────────────────────────────
// Strict-mode completion gating.
//
// `strict.completion` only offers variables when the cursor sits on an env
// object, so this gate decides whether completion appears at all.
// ───────────────────────────────────────────────────────────────────────────

async fn completion_gate(
    file: &str,
    language_id: &str,
    content: &str,
    line: u32,
    character: u32,
) -> (bool, Option<String>) {
    use ecolog_lsp::analysis::document::DocumentManager;
    use tower_lsp::lsp_types::{Position, Url};

    let manager = DocumentManager::new(
        Arc::new(QueryEngine::new()),
        Arc::new(LanguageRegistry::with_all_languages()),
    );
    let uri = Url::parse(&format!("file:///{}", file)).expect("uri");
    manager
        .open(uri.clone(), language_id.into(), content.to_string(), 1)
        .await;

    let position = Position::new(line, character);
    (
        manager.check_completion(&uri, position).await,
        manager
            .check_completion_context(&uri, position)
            .await
            .map(|s| s.to_string()),
    )
}

#[tokio::test]
async fn test_completion_offered_on_env_objects() {
    let cases: &[(&str, &str, &str, &str, u32, u32)] = &[
        ("process.env.", "t.js", "javascript", "console.log(process.env.)", 0, 24),
        (
            "import.meta.env.",
            "t.js",
            "javascript",
            "console.log(import.meta.env.)",
            0,
            28,
        ),
        (
            "process.env[\"\"]",
            "t.js",
            "javascript",
            "console.log(process.env[\"\"])",
            0,
            25,
        ),
        (
            "import.meta.env[\"\"]",
            "t.js",
            "javascript",
            "console.log(import.meta.env[\"\"])",
            0,
            29,
        ),
        (
            "alias of process.env",
            "t.js",
            "javascript",
            "const e = process.env;\ne.",
            1,
            2,
        ),
        (
            "alias of import.meta.env",
            "t.js",
            "javascript",
            "const e = import.meta.env;\ne.",
            1,
            2,
        ),
        (
            "typescript import.meta.env.",
            "t.ts",
            "typescript",
            "console.log(import.meta.env.)",
            0,
            28,
        ),
        (
            "os.environ[\"\"]",
            "t.py",
            "python",
            "print(os.environ[\"\"])",
            0,
            18,
        ),
    ];

    for (label, file, id, content, line, character) in cases {
        let (gate, ctx) = completion_gate(file, id, content, *line, *character).await;
        assert!(gate, "completion not offered for {} (context {:?})", label, ctx);
    }
}
