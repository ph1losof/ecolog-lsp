use korni::{Entry, QuoteType, Span};
use ropey::Rope;
use tower_lsp::lsp_types::{SemanticToken, SemanticTokenModifier, SemanticTokenType};

pub struct SemanticTokenProvider;

impl SemanticTokenProvider {
    pub const LEGEND_TYPES: &'static [SemanticTokenType] = &[
        SemanticTokenType::PROPERTY, // 0: keys (property names)
        SemanticTokenType::KEYWORD,  //1: export
        SemanticTokenType::STRING,   // 2: values
        SemanticTokenType::NUMBER,   // 3: numeric values
        SemanticTokenType::COMMENT,  // 4: comments
        SemanticTokenType::OPERATOR, // 5: =
    ];

    pub const LEGEND_MODIFIERS: &'static [SemanticTokenModifier] = &[
        SemanticTokenModifier::DECLARATION, // 0: definition of variable
        SemanticTokenModifier::STATIC,      // 1: constant/static
        SemanticTokenModifier::ABSTRACT,    // 2: used for CONCEAL/MASKING if needed
    ];

    pub fn extract_tokens(rope: &Rope, content: &str, entries: &[Entry]) -> Vec<SemanticToken> {
        let mut tokens = Vec::new();
        let mut pre_line = 0;
        let mut pre_start = 0;

        for entry in entries {
            match entry {
                Entry::Pair(pair) => {
                    // 1. Export keyword
                    if let Some(span) = pair.export_span {
                        Self::push_token(
                            &mut tokens,
                            rope,
                            content,
                            &mut pre_line,
                            &mut pre_start,
                            span,
                            1,
                            0,
                        ); // 1=Keyword
                    }

                    // 2. Key (Property)
                    if let Some(span) = pair.key_span {
                        Self::push_token(
                            &mut tokens,
                            rope,
                            content,
                            &mut pre_line,
                            &mut pre_start,
                            span,
                            0,
                            0,
                        ); // 0=Property, no modifier
                    }

                    // 3. Operator (=)
                    if let Some(pos) = pair.equals_pos {
                        let span = Span::new(pos, korni::Position::from_offset(pos.offset + 1));
                        Self::push_token(
                            &mut tokens,
                            rope,
                            content,
                            &mut pre_line,
                            &mut pre_start,
                            span,
                            5,
                            0,
                        ); // 5=Operator
                    }

                    // 4. Value
                    if let Some(span) = pair.value_span {
                        // Decide type: Number or String?
                        let token_type = if pair.quote == QuoteType::None && is_numeric(&pair.value)
                        {
                            3 // Number
                        } else {
                            2 // String
                        };

                        // Modifier: 0
                        Self::push_token(
                            &mut tokens,
                            rope,
                            content,
                            &mut pre_line,
                            &mut pre_start,
                            span,
                            token_type,
                            0,
                        );
                    }
                }
                Entry::Comment(span) => {
                    Self::push_token(
                        &mut tokens,
                        rope,
                        content,
                        &mut pre_line,
                        &mut pre_start,
                        *span,
                        4,
                        0,
                    ); // 4=Comment
                }
                _ => {}
            }
        }

        tokens
    }

    #[allow(clippy::too_many_arguments)]
    fn push_token(
        tokens: &mut Vec<SemanticToken>,
        rope: &Rope,
        content: &str,
        pre_line: &mut u32,
        pre_start: &mut u32,
        span: Span,
        token_type: u32,
        token_modifiers_bitset: u32,
    ) {
        let start_line = rope.byte_to_line(span.start.offset);
        let line_start_byte = rope.line_to_byte(start_line);
        let start_col = rope
            .byte_slice(line_start_byte..span.start.offset)
            .len_chars() as u32;

        let span_text = &content[span.start.offset..span.end.offset.min(content.len())];
        let contains_newline = span_text.contains('\n');

        if contains_newline {
            // Split multi-line spans into per-line tokens
            let mut current_offset = span.start.offset;
            let mut first_token = true;

            for line_text in span_text.lines() {
                if line_text.is_empty() {
                    // Skip empty lines
                    current_offset += 1; // newline character
                    continue;
                }

                let token_line = rope.byte_to_line(current_offset);
                let token_line_start_byte = rope.line_to_byte(token_line);
                let token_col = rope
                    .byte_slice(token_line_start_byte..current_offset)
                    .len_chars() as u32;
                let token_length = line_text.chars().count() as u32;

                if first_token {
                    let delta_line = token_line as u32 - *pre_line;
                    let delta_start = if delta_line == 0 {
                        token_col - *pre_start
                    } else {
                        token_col
                    };

                    tokens.push(SemanticToken {
                        delta_line,
                        delta_start,
                        length: token_length,
                        token_type,
                        token_modifiers_bitset,
                    });

                    *pre_line = token_line as u32;
                    *pre_start = token_col;
                    first_token = false;
                } else {
                    let delta_line = token_line as u32 - *pre_line;
                    tokens.push(SemanticToken {
                        delta_line,
                        delta_start: 0,
                        length: token_length,
                        token_type,
                        token_modifiers_bitset,
                    });

                    *pre_line = token_line as u32;
                    *pre_start = 0;
                }

                // Move to next line (include newline character)
                current_offset += line_text.len() + 1;
            }
        } else {
            // Single-line token - use original logic
            let length = span.len() as u32;

            if length == 0 {
                return;
            }

            let delta_line = start_line as u32 - *pre_line;
            let delta_start = if delta_line == 0 {
                start_col - *pre_start
            } else {
                start_col
            };

            tokens.push(SemanticToken {
                delta_line,
                delta_start,
                length,
                token_type,
                token_modifiers_bitset,
            });

            *pre_line = start_line as u32;
            *pre_start = start_col;
        }
    }
}

fn is_numeric(s: &str) -> bool {
    s.parse::<f64>().is_ok() // Simple check
}
