use ecolog_lsp::server::semantic_tokens::SemanticTokenProvider;
use korni::ParseOptions;

#[test]
fn test_semantic_tokens_generation() {
    let input = "DB_HOST=localhost\n# comment\nexport PORT=5432";
    let entries = korni::parse_with_options(input, ParseOptions::full());

    let rope = ropey::Rope::from_str(input);
    let tokens = SemanticTokenProvider::extract_tokens(&rope, input, &entries);

    // We expect:
    // 1. DB_HOST (Property)
    // 2. = (Operator)
    // 3. localhost (String)
    // 4. # comment (Comment)
    // 5. export (Keyword)
    // 6. PORT (Property)
    // 7. = (Operator)
    // 8. 5432 (Number)

    assert!(!tokens.is_empty());

    // Verify first token (DB_HOST)
    let t0 = &tokens[0];
    assert_eq!(t0.token_type, 0); // Property

    // Verify number detection (5432)
    let last = tokens.last().unwrap();
    assert_eq!(last.token_type, 3); // Number
}
