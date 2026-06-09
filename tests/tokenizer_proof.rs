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
