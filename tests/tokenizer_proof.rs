// ┌────────────────────────────────────────────────────────────┐
// │  M0 Phase B: Functional Tokenizer — Manual Rust Proof      │
// │  Demonstrates functional-style compiler code that the      │
// │  Morphic IR can synthesize. No self, no &mut, no impl.     │
// └────────────────────────────────────────────────────────────┘

/// Tokenizer state — immutable, passed explicitly to every function
#[derive(Debug, Clone, PartialEq)]
struct TokenizerState {
    source: String,
    pos: usize,
    line: usize,
    col: usize,
    tokens: Vec<Token>,
}

#[derive(Debug, Clone, PartialEq)]
struct Token {
    kind: TokenKind,
    lexeme: String,
    line: usize,
    col: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TokenKind {
    Ident = 0,
    IntLit = 1,
    StringLit = 2,
    LParen = 3,
    RParen = 4,
    LBrace = 5,
    RBrace = 6,
    Colon = 7,
    Semicolon = 8,
    Comma = 9,
    Lt = 10,
    Gt = 11,
    Eq = 12,
}

// ── PURE FUNCTIONS ────────────────────────────────────────
// No self. No &mut. Pass state → return new state.

fn peek(state: &TokenizerState) -> Option<char> {
    state.source.chars().nth(state.pos)
}

fn next_char(state: TokenizerState) -> TokenizerState {
    let ch = state.source.chars().nth(state.pos);
    let is_newline = ch == Some('\n');
    TokenizerState {
        pos: state.pos + 1,
        line: if is_newline { state.line + 1 } else { state.line },
        col: if is_newline { 1 } else { state.col + 1 },
        ..state
    }
}

fn is_whitespace(c: char) -> bool {
    c == ' ' || c == '\t' || c == '\n' || c == '\r'
}

fn is_ident_start(c: char) -> bool {
    (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    is_ident_start(c) || (c >= '0' && c <= '9')
}

fn skip_whitespace(mut state: TokenizerState) -> TokenizerState {
    while peek(&state).map_or(false, |c| is_whitespace(c)) {
        state = next_char(state);
    }
    state
}

fn push_token(state: TokenizerState, kind: TokenKind, lexeme: String) -> TokenizerState {
    let mut tokens = state.tokens;
    tokens.push(Token {
        kind,
        lexeme,
        line: state.line,
        col: state.col,
    });
    TokenizerState { tokens, ..state }
}

fn tokenize_one(state: TokenizerState) -> Result<TokenizerState, String> {
    let state = skip_whitespace(state);
    match peek(&state) {
        None => Ok(state), // EOF
        Some(c) if is_ident_start(c) => {
            // Read entire identifier
            let mut lexeme = String::new();
            let mut s = state;
            while peek(&s).map_or(false, |ch| is_ident_continue(ch)) {
                if let Some(ch) = peek(&s) { lexeme.push(ch); }
                s = next_char(s);
            }
            Ok(push_token(s, TokenKind::Ident, lexeme))
        }
        Some(':') => Ok(push_token(next_char(state), TokenKind::Colon, ":".into())),
        Some('{') => Ok(push_token(next_char(state), TokenKind::LBrace, "{".into())),
        Some('}') => Ok(push_token(next_char(state), TokenKind::RBrace, "}".into())),
        Some('(') => Ok(push_token(next_char(state), TokenKind::LParen, "(".into())),
        Some(')') => Ok(push_token(next_char(state), TokenKind::RParen, ")".into())),
        Some(',') => Ok(push_token(next_char(state), TokenKind::Comma, ",".into())),
        Some('<') => Ok(push_token(next_char(state), TokenKind::Lt, "<".into())),
        Some('>') => Ok(push_token(next_char(state), TokenKind::Gt, ">".into())),
        Some('=') => Ok(push_token(next_char(state), TokenKind::Eq, "=".into())),
        Some(other) => Err(format!("Unexpected character '{}' at line {}", other, state.line)),
    }
}

fn tokenize_all(mut state: TokenizerState) -> Result<Vec<Token>, String> {
    while peek(&state).is_some() {
        state = tokenize_one(state)?;
    }
    Ok(state.tokens)
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn init_state(source: &str) -> TokenizerState {
        TokenizerState {
            source: source.to_string(),
            pos: 0,
            line: 1,
            col: 1,
            tokens: vec![],
        }
    }

    #[test]
    fn test_tokenize_simple_spec() {
        let state = init_state("spec sort");
        let result = tokenize_all(state).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].lexeme, "spec");
        assert_eq!(result[0].kind, TokenKind::Ident);
        assert_eq!(result[1].lexeme, "sort");
        assert_eq!(result[1].kind, TokenKind::Ident);
    }

    #[test]
    fn test_tokenize_empty() {
        let state = init_state("");
        let result = tokenize_all(state).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_tokenize_with_punctuation() {
        let state = init_state("spec{input:()}");
        let result = tokenize_all(state).unwrap();
        // spec, {, input, :, (, ), }
        assert_eq!(result.len(), 7);
        assert_eq!(result[0].lexeme, "spec");
        assert_eq!(result[1].lexeme, "{");
        assert_eq!(result[2].lexeme, "input");
        assert_eq!(result[3].lexeme, ":");
        assert_eq!(result[4].lexeme, "(");
        assert_eq!(result[5].lexeme, ")");
        assert_eq!(result[6].lexeme, "}");
    }

    #[test]
    fn test_skip_whitespace() {
        let state = init_state("   hello");
        let result = skip_whitespace(state);
        assert_eq!(result.pos, 3);
        assert_eq!(result.col, 4);
    }

    #[test]
    fn test_functional_roundtrip() {
        // State is never mutated — each function returns a NEW state
        let s1 = init_state("a b");
        let s2 = skip_whitespace(s1);
        assert_eq!(s2.pos, 0); // No whitespace to skip

        let c = peek(&s2).unwrap();
        assert_eq!(c, 'a');
        let s3 = next_char(s2.clone());
        assert_eq!(s3.pos, 1);
        // s2 is still at pos 0 — immutable!
        assert_eq!(s2.pos, 0);
    }
}

// ── S4 MIRRORS (Phase 47) ─────────────────────────────────
// Executable mirrors of the eight specs in
// examples/functional_tokenizer.morph (as re-authored), one
// function per spec, transcribed from the specs' stated
// behavior. Where a spec under-determines its output — the
// state fields its constraints never pin, and
// skip_whitespace / tokenize_one / tokenize_all, whose
// functional content lived in their tests — the mirror
// completes it the way the original file's tests intend:
// untouched fields carry over, and the tokenizing behavior
// is the hand-written tokenizer's above, with ONE recorded
// divergence: a token is stamped with the line/col where
// it STARTS (the spec's tests), not where it ends (the
// hand-written push_token reads the already-advanced
// state). String positions are char indices, as in the
// hand-written tokenizer (chars().nth(pos)); the spec's
// len() / indexing read the same way over the batteries
// below. The M0 code above is untouched; these mirrors sit
// beside it so the cross-check is a same-file assertion.

fn mirror_peek(state: &TokenizerState) -> Option<char> {
    if state.pos >= state.source.chars().count() {
        None
    } else {
        state.source.chars().nth(state.pos)
    }
}

fn mirror_next_char(state: TokenizerState) -> TokenizerState {
    if mirror_peek(&state) == Some('\n') {
        TokenizerState {
            pos: state.pos + 1,
            line: state.line + 1,
            col: 1,
            ..state
        }
    } else {
        TokenizerState {
            pos: state.pos + 1,
            col: state.col + 1,
            ..state
        }
    }
}

fn mirror_is_whitespace(c: char) -> bool {
    c == ' ' || c == '\t' || c == '\n' || c == '\r'
}

fn mirror_is_ident_start(c: char) -> bool {
    (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_'
}

fn mirror_is_ident_continue(c: char) -> bool {
    mirror_is_ident_start(c) || (c >= '0' && c <= '9')
}

fn mirror_skip_whitespace(state: TokenizerState) -> TokenizerState {
    let mut state = state;
    while mirror_peek(&state).map_or(false, mirror_is_whitespace) {
        state = mirror_next_char(state);
    }
    state
}

/// Append a token stamped with the START state's line/col
/// (the spec's tests) to the END state's token list; the
/// rest of the result is the end state. This is the one
/// place the mirrors deliberately differ from the
/// hand-written push_token above, which stamps the token
/// from the single, already-advanced state it is given.
fn mirror_push_token(
    start: &TokenizerState,
    end: TokenizerState,
    kind: TokenKind,
    lexeme: String,
) -> TokenizerState {
    let mut tokens = end.tokens.clone();
    tokens.push(Token {
        kind,
        lexeme,
        line: start.line,
        col: start.col,
    });
    TokenizerState { tokens, ..end }
}

/// One single-character token: advance once, stamp at the
/// position the character occupied.
fn mirror_single(
    state: &TokenizerState,
    kind: TokenKind,
    lexeme: &str,
) -> Result<TokenizerState, String> {
    let end = mirror_next_char(state.clone());
    Ok(mirror_push_token(state, end, kind, lexeme.to_string()))
}

fn mirror_tokenize_one(state: TokenizerState) -> Result<TokenizerState, String> {
    let state = mirror_skip_whitespace(state);
    match mirror_peek(&state) {
        None => Ok(state), // EOF
        Some(c) if mirror_is_ident_start(c) => {
            // Read entire identifier; the token is stamped
            // where the identifier starts.
            let mut lexeme = String::new();
            let mut s = state.clone();
            while mirror_peek(&s).map_or(false, mirror_is_ident_continue) {
                if let Some(ch) = mirror_peek(&s) {
                    lexeme.push(ch);
                }
                s = mirror_next_char(s);
            }
            Ok(mirror_push_token(&state, s, TokenKind::Ident, lexeme))
        }
        Some(':') => mirror_single(&state, TokenKind::Colon, ":"),
        Some('{') => mirror_single(&state, TokenKind::LBrace, "{"),
        Some('}') => mirror_single(&state, TokenKind::RBrace, "}"),
        Some('(') => mirror_single(&state, TokenKind::LParen, "("),
        Some(')') => mirror_single(&state, TokenKind::RParen, ")"),
        Some(',') => mirror_single(&state, TokenKind::Comma, ","),
        Some('<') => mirror_single(&state, TokenKind::Lt, "<"),
        Some('>') => mirror_single(&state, TokenKind::Gt, ">"),
        Some('=') => mirror_single(&state, TokenKind::Eq, "="),
        Some(other) => Err(format!(
            "Unexpected character '{}' at line {}",
            other, state.line
        )),
    }
}

fn mirror_tokenize_all(state: TokenizerState) -> Result<Vec<Token>, String> {
    let mut state = state;
    while mirror_peek(&state).is_some() {
        state = mirror_tokenize_one(state)?;
    }
    Ok(state.tokens)
}

// ── S4 golden + cross-check tests ─────────────────────────

#[cfg(test)]
mod s4_mirror_tests {
    use super::*;

    fn st(source: &str, pos: usize, line: usize, col: usize) -> TokenizerState {
        TokenizerState {
            source: source.to_string(),
            pos,
            line,
            col,
            tokens: vec![],
        }
    }

    fn tok(kind: TokenKind, lexeme: &str, line: usize, col: usize) -> Token {
        Token {
            kind,
            lexeme: lexeme.to_string(),
            line,
            col,
        }
    }

    // The nine in-spec tests the re-authored file could not
    // carry (record literals), transcribed verbatim — peek.

    #[test]
    fn s4_peek_golden() {
        // test "non-empty"
        assert_eq!(mirror_peek(&st("hi", 0, 1, 1)), Some('h'));
        // test "at-end"
        assert_eq!(mirror_peek(&st("", 0, 1, 1)), None);
    }

    #[test]
    fn s4_next_char_golden() {
        // test "normal"
        assert_eq!(mirror_next_char(st("ab", 0, 1, 1)), st("ab", 1, 1, 2));
        // The spec's newline rows, as a case: consuming '\n'
        // bumps the line and resets the column.
        assert_eq!(mirror_next_char(st("a\nb", 1, 1, 2)), st("a\nb", 2, 2, 1));
    }

    // The nine classifier tests the re-authored file DID
    // carry in-spec (char literals, F4) — the same cases,
    // asserted against the mirrors.

    #[test]
    fn s4_classifier_golden() {
        // is_whitespace: "space", "letter", "newline"
        assert!(mirror_is_whitespace(' '));
        assert!(!mirror_is_whitespace('a'));
        assert!(mirror_is_whitespace('\n'));
        // is_ident_start: "lower", "digit", "underscore"
        assert!(mirror_is_ident_start('a'));
        assert!(!mirror_is_ident_start('5'));
        assert!(mirror_is_ident_start('_'));
        // is_ident_continue: "letter", "digit-ok", "special"
        assert!(mirror_is_ident_continue('x'));
        assert!(mirror_is_ident_continue('7'));
        assert!(!mirror_is_ident_continue('{'));
    }

    // The classifiers against the hand-written tokenizer
    // over the full ASCII range plus non-ASCII probes: the
    // mirror bodies ARE the spec equations, so this asserts
    // spec == ground truth on every char, not mirror ==
    // mirror.

    #[test]
    fn s4_classifier_battery_vs_proof() {
        let mut chars: Vec<char> = (0u32..=0x7F).filter_map(char::from_u32).collect();
        chars.extend(['é', 'ß', 'Ω', '\u{85}', '\u{A0}', '😀']);
        for c in chars {
            assert_eq!(mirror_is_whitespace(c), is_whitespace(c), "ws {:?}", c);
            assert_eq!(mirror_is_ident_start(c), is_ident_start(c), "start {:?}", c);
            assert_eq!(
                mirror_is_ident_continue(c),
                is_ident_continue(c),
                "cont {:?}",
                c
            );
        }
    }

    // peek / next_char against the hand-written tokenizer
    // over source strings (incl. multi-byte chars and every
    // position, one past the end included): full-state
    // equality — these two mirrors agree with the ground
    // truth exactly.

    #[test]
    fn s4_peek_next_char_crosscheck() {
        for src in ["", "ab", "a\nb", "é😀x", " \t\r\n", "spec sort"] {
            let n = src.chars().count();
            for pos in 0..=n + 1 {
                let s = st(src, pos, 3, 7);
                assert_eq!(mirror_peek(&s), peek(&s), "peek {:?} @ {}", src, pos);
                assert_eq!(
                    mirror_next_char(s.clone()),
                    next_char(s),
                    "next {:?} @ {}",
                    src,
                    pos
                );
            }
        }
    }

    // skip_whitespace: the two moved tests, the M0 proof's
    // own case, and a battery cross-check that also asserts
    // the spec's two stated postconditions and the stopping
    // intent on every case.

    #[test]
    fn s4_skip_whitespace_golden() {
        // test "no-whitespace"
        assert_eq!(mirror_skip_whitespace(st("hi", 0, 1, 1)), st("hi", 0, 1, 1));
        // test "skip-spaces"
        assert_eq!(
            mirror_skip_whitespace(st("  hi", 0, 1, 1)),
            st("  hi", 2, 1, 3)
        );
        // The M0 proof's case (test_skip_whitespace above).
        assert_eq!(
            mirror_skip_whitespace(st("   hello", 0, 1, 1)),
            st("   hello", 3, 1, 4)
        );
    }

    #[test]
    fn s4_skip_whitespace_crosscheck() {
        for src in ["", " ", "  \t\n x", "no-ws", "\n\nab", " \u{A0}x", "a b", "\r\n\t"] {
            let input = st(src, 0, 1, 1);
            let out = mirror_skip_whitespace(input.clone());
            // Agrees with the hand-written tokenizer exactly.
            assert_eq!(out, skip_whitespace(input.clone()), "skip {:?}", src);
            // The spec's two stated postconditions.
            assert!(out.pos >= input.pos, "pos regressed on {:?}", src);
            assert_eq!(out.source.chars().count(), input.source.chars().count());
            // The intent: stops at the first non-whitespace
            // char (or end), having skipped only whitespace.
            // (NBSP is not in the spec's whitespace set, so
            // " \u{A0}x" stops at pos 1.)
            assert!(mirror_peek(&out).map_or(true, |c| !mirror_is_whitespace(c)));
            assert!(src.chars().take(out.pos).all(mirror_is_whitespace));
        }
    }

    // tokenize_one / tokenize_all: the four moved tests,
    // verbatim.

    #[test]
    fn s4_tokenize_one_golden() {
        // test "keyword-spec": the token is stamped where it
        // starts (line 1, col 1) — see the divergence test
        // below for the hand-written tokenizer's col 5.
        let out = mirror_tokenize_one(st("spec", 0, 1, 1)).unwrap();
        assert_eq!(
            out,
            TokenizerState {
                source: "spec".to_string(),
                pos: 4,
                line: 1,
                col: 5,
                tokens: vec![tok(TokenKind::Ident, "spec", 1, 1)],
            }
        );
        // test "end-of-input"
        assert_eq!(mirror_tokenize_one(st("", 0, 1, 1)).unwrap(), st("", 0, 1, 1));
    }

    #[test]
    fn s4_tokenize_all_golden() {
        // test "simple-spec"
        assert_eq!(
            mirror_tokenize_all(st("spec sort", 0, 1, 1)).unwrap(),
            vec![
                tok(TokenKind::Ident, "spec", 1, 1),
                tok(TokenKind::Ident, "sort", 1, 6),
            ]
        );
        // test "empty"
        assert_eq!(
            mirror_tokenize_all(st("", 0, 1, 1)).unwrap(),
            Vec::<Token>::new()
        );
    }

    // The cross-check against the hand-written tokenizer,
    // seeded by its own punctuation case — and the recorded
    // stamping divergence, pinned on both sides so any
    // future change to either artifact is loud.

    #[test]
    fn s4_tokenize_crosscheck_and_stamping_divergence() {
        let src = "spec{input:()}";
        let mirror_tokens = mirror_tokenize_all(st(src, 0, 1, 1)).unwrap();
        let proof_tokens = tokenize_all(st(src, 0, 1, 1)).unwrap();
        assert_eq!(mirror_tokens.len(), 7);
        assert_eq!(proof_tokens.len(), 7);
        // Lexemes, kinds, and lines agree everywhere.
        for (m, p) in mirror_tokens.iter().zip(&proof_tokens) {
            assert_eq!(m.lexeme, p.lexeme);
            assert_eq!(m.kind, p.kind);
            assert_eq!(m.line, p.line);
        }
        // Columns diverge exactly as recorded: the spec
        // stamps token starts, the hand-written tokenizer
        // stamps token ends.
        let mirror_cols: Vec<usize> = mirror_tokens.iter().map(|t| t.col).collect();
        let proof_cols: Vec<usize> = proof_tokens.iter().map(|t| t.col).collect();
        assert_eq!(mirror_cols, vec![1, 5, 6, 11, 12, 13, 14]);
        assert_eq!(proof_cols, vec![5, 6, 11, 12, 13, 14, 15]);

        // The same divergence on the spec's own case.
        let proof_simple = tokenize_all(st("spec sort", 0, 1, 1)).unwrap();
        assert_eq!(
            proof_simple,
            vec![
                tok(TokenKind::Ident, "spec", 1, 5),
                tok(TokenKind::Ident, "sort", 1, 10),
            ]
        );
    }

    #[test]
    fn s4_tokenize_error_and_edge_crosscheck() {
        // Digits have no arm (IntLit is declared in the kind
        // table but never produced): both sides Err with the
        // same message.
        let expected = Err("Unexpected character '5' at line 1".to_string());
        assert_eq!(mirror_tokenize_all(st("spec 5", 0, 1, 1)), expected);
        assert_eq!(tokenize_all(st("spec 5", 0, 1, 1)), expected);
        // Whitespace-only input tokenizes to no tokens (the
        // loop terminates through skip-only steps), both sides.
        assert_eq!(
            mirror_tokenize_all(st("  \n ", 0, 1, 1)).unwrap(),
            Vec::<Token>::new()
        );
        assert_eq!(
            tokenize_all(st("  \n ", 0, 1, 1)).unwrap(),
            Vec::<Token>::new()
        );
        // A leading-whitespace identifier: the mirror stamps
        // the token where it starts, after the skip.
        let out = mirror_tokenize_one(st("  spec", 0, 1, 1)).unwrap();
        assert_eq!(out.tokens, vec![tok(TokenKind::Ident, "spec", 1, 3)]);
        assert_eq!((out.pos, out.line, out.col), (6, 1, 7));
    }
}

