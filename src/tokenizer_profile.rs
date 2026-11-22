// ─────────────────────────────────────────────────────────────
//  TOKENIZER PROFILE INSTRUMENT (test-only) — Phase 87
//  (M1-features closing run, road B workstream 1: profile
//  BEFORE attacking the Phase 86 benchmark ratio)
//
//  Phase 86 measured the assembled generated tokenizer at
//  ≈3.0–3.4× the hand-written mirror and recorded one
//  hedged mechanism candidate (by-value state passing +
//  clones at call sites). This instrument ATTRIBUTES the
//  gap before any production surface is touched: it
//  assembles the same composition as the Phase 86 harness
//  (src/tokenizer_benchmark.rs — its assembly, prelude,
//  mirror text, driver and corpus are copied here under
//  the standing copy precedent) and then builds ABLATION
//  VARIANTS of the generated side. Each variant replaces
//  individual generated functions with hand-written bodies
//  in the same harness shape, one change at a time,
//  cumulatively:
//
//   P0  production composition, unmodified (sanity: its
//       ratio must reproduce Phase 86's ≈3.0–3.4×)
//   P1  peek → `source.chars().nth(pos)` directly. The
//       production peek is `if pos < chars().count()
//       { Some(nth(pos)) } else { None }` — the nth form
//       is the SAME function (nth is Some exactly when
//       pos < char count, with the same char), so P1
//       prices the `chars().count()` scan in every peek
//       CALL (next_char's inlined copies are inside its
//       own body and are NOT touched here).
//   P2  P1 + next_char computes its peeked char ONCE
//       (production inlines the peek expression twice —
//       once for the line row, once for the col row) but
//       still builds its record with .clone() field
//       values, exactly as the production RecordLit
//       renders. Prices the double peek evaluation.
//   P3  P2's single computation + the record built by
//       struct update (fields MOVE out of the parameter;
//       no source/tokens clones). Prices the RecordLit
//       field clones in next_char.
//   P4  P3 + tokenize_one restructured at the template
//       builder's own surfaces: the ident loop peeks
//       ONCE per char (production: condition peek + body
//       peek, each with a full state clone) and the
//       punctuation arms move the post-skip state into
//       next_char (line/col bound first) instead of
//       cloning it. Prices the template's clone points.
//   P5  P4 + peek by REFERENCE at every call site (the
//       global by-value call convention itself). NOT
//       producible by any in-scope production surface —
//       this variant exists to price the convention wall
//       the Phase 87 brief names, nothing more.
//
//  Every variant must pass the harness's EQUALITY GATE
//  (its checksums equal the mirror's on both workloads)
//  before its times are reported: the ablations are
//  semantics-preserving, so a variant that disagrees is
//  an instrument bug, not a result.
//
//  CLONE ACCOUNTING: a second pair of binaries (generated
//  P0 and mirror) is assembled under a counting prelude —
//  TokenizerState/Token carry manual Clone impls that
//  count invocations and cloned source bytes — and run
//  for ONE pass of each workload. The counts corroborate
//  (or refute) the ablation attribution.
//
//  POST-ATTACK NOTE (Phase 87, same phase): the attack
//  landed (see ROAD Phase 87) — the template is peek-free
//  (Index-read chars), record_completion shares its
//  guard through a Let, and the Rust RecordLit arm has
//  a struct-update variant rendering. P0 assembles
//  CURRENT production, so on the post-attack tree it
//  reads the new ratio (≈1.25–1.49 on this instrument's
//  timing loop; the Phase 86 harness itself reads
//  1.39–1.58); P1–P5 remain the pre-attack ablation
//  ladder, kept as the attribution record — their hand
//  bodies no longer stack on the current composition,
//  and their post-attack numbers are not readings of
//  anything but themselves.
//
//  This instrument changes NO production code. Run with:
//
//    cargo test -- --ignored --nocapture tokenizer_profile
//
//  Assembled variant sources are written to
//  target/tokenizer-profile/.
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod prof_tok {
    use crate::codegen;
    use crate::spec;
    use crate::spec::ast::FunctionSpec;
    use crate::synthesis::engine::{self, CandidateImplementation, IRNode, IRPattern, Provenance};
    use std::collections::HashMap;
    use std::process::Command;
    use std::time::{Duration, Instant};

    fn load_tokenizer_module() -> (Vec<FunctionSpec>, spec::ast::Spec) {
        let root = env!("CARGO_MANIFEST_DIR");
        let path = format!("{}/examples/functional_tokenizer.morph", root);
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = spec::parser::parse(&source).expect("parse must succeed");
        let checked = spec::typeck::check(parsed).expect("typeck must succeed");
        (checked.spec.functions.clone(), checked.spec)
    }

    /// The cross-validation harness's runner prelude,
    /// verbatim (as copied in src/tokenizer_benchmark.rs).
    const RUNNER_PRELUDE: &str = r#"
#[derive(Debug, Clone, PartialEq)]
pub struct Token { pub kind: i64, pub lexeme: String, pub line: i64, pub col: i64 }
#[derive(Debug, Clone, PartialEq)]
pub struct TokenizerState { pub source: String, pub pos: i64, pub line: i64, pub col: i64, pub tokens: Vec<Token> }
pub type StepResult = Result<TokenizerState, String>;
fn show_tokens(ts: &Vec<Token>) -> String { ts.iter().map(|t| format!("{},{:?},{},{}", t.kind, t.lexeme, t.line, t.col)).collect::<Vec<_>>().join(";") }
fn show_state(s: &TokenizerState) -> String { format!("{}|{}|{}|{}|{:?}|{}", s.pos, s.line, s.col, s.source.chars().count(), s.source, show_tokens(&s.tokens)) }
"#;

    /// The counting prelude: identical types, but Clone is
    /// manual and accounted (state clones, token clones,
    /// cloned source bytes). Used only by the accounting
    /// binaries.
    const COUNTING_PRELUDE: &str = r#"
pub static STATE_CLONES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static TOKEN_CLONES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static STATE_CLONE_SRC_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
#[derive(Debug, PartialEq)]
pub struct Token { pub kind: i64, pub lexeme: String, pub line: i64, pub col: i64 }
impl Clone for Token {
    fn clone(&self) -> Self {
        TOKEN_CLONES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Token { kind: self.kind, lexeme: self.lexeme.clone(), line: self.line, col: self.col }
    }
}
#[derive(Debug, PartialEq)]
pub struct TokenizerState { pub source: String, pub pos: i64, pub line: i64, pub col: i64, pub tokens: Vec<Token> }
impl Clone for TokenizerState {
    fn clone(&self) -> Self {
        STATE_CLONES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        STATE_CLONE_SRC_BYTES.fetch_add(self.source.len() as u64, std::sync::atomic::Ordering::Relaxed);
        TokenizerState { source: self.source.clone(), pos: self.pos, line: self.line, col: self.col, tokens: self.tokens.clone() }
    }
}
pub type StepResult = Result<TokenizerState, String>;
fn show_tokens(ts: &Vec<Token>) -> String { ts.iter().map(|t| format!("{},{:?},{},{}", t.kind, t.lexeme, t.line, t.col)).collect::<Vec<_>>().join(";") }
fn show_state(s: &TokenizerState) -> String { format!("{}|{}|{}|{}|{:?}|{}", s.pos, s.line, s.col, s.source.chars().count(), s.source, show_tokens(&s.tokens)) }
"#;

    /// The Phase 86 assembly, copied from
    /// src/tokenizer_benchmark.rs (assemble_winners).
    fn assemble_winners(
        functions: &[FunctionSpec],
        module: &spec::ast::Spec,
    ) -> HashMap<String, CandidateImplementation> {
        let pool_seed = |f: &FunctionSpec, kinds: &[&str]| {
            let candidates = engine::synthesize_with_module_spec(f, 1, module).unwrap();
            kinds.iter().find_map(|k| {
                candidates.iter().find(|c| {
                    matches!(&c.provenance, Provenance::ConstraintInduced { kind } if kind == k)
                }).cloned()
            })
        };
        let mut winners: HashMap<String, CandidateImplementation> = HashMap::new();
        for f in functions {
            let pick = match f.name.as_str() {
                "skip_whitespace" => {
                    let mut hand = pool_seed(
                        functions.iter().find(|g| g.name == "peek").unwrap(),
                        &["complementary_guard"],
                    )
                    .unwrap();
                    hand.spec_name = "skip_whitespace".to_string();
                    hand.spec = f.clone();
                    hand.provenance = Provenance::Template {
                        template_name: "phase85_hand_mirror_skip".to_string(),
                    };
                    hand.body = {
                        let v = |n: &str| IRNode::Var(n.to_string());
                        let call = |fu: &str, args: Vec<IRNode>| IRNode::Call {
                            function: fu.to_string(),
                            args,
                        };
                        IRNode::Block(vec![
                            IRNode::Alloc {
                                name: "cur".to_string(),
                                ty: crate::spec::ast::TypeRef::Named(
                                    "TokenizerState".to_string(),
                                ),
                                initial: Box::new(v("state")),
                            },
                            IRNode::While {
                                condition: Box::new(IRNode::Match {
                                    scrutinee: Box::new(call(
                                        "peek",
                                        vec![IRNode::UnaryOp(
                                            crate::synthesis::engine::IRUnaryOp::Clone,
                                            Box::new(v("cur")),
                                        )],
                                    )),
                                    cases: vec![
                                        (
                                            IRPattern::Constructor(
                                                "None".to_string(),
                                                vec![],
                                            ),
                                            IRNode::Literal(
                                                crate::synthesis::engine::IRLiteral::Bool(false),
                                            ),
                                        ),
                                        (
                                            IRPattern::Constructor(
                                                "Some".to_string(),
                                                vec![IRPattern::Variable(
                                                    "ch".to_string(),
                                                )],
                                            ),
                                            call("is_whitespace", vec![v("ch")]),
                                        ),
                                    ],
                                }),
                                body: Box::new(IRNode::Block(vec![IRNode::Assign {
                                    target: Box::new(v("cur")),
                                    value: Box::new(call(
                                        "next_char",
                                        vec![v("cur")],
                                    )),
                                }])),
                                invariant: None,
                            },
                            IRNode::Return(Box::new(v("cur"))),
                        ])
                    };
                    hand
                }
                "tokenize_one" | "tokenize_all" => pool_seed(f, &["tokenize_template"])
                    .unwrap_or_else(|| panic!("{}: template seed missing", f.name)),
                "peek" => pool_seed(f, &["complementary_guard"])
                    .unwrap_or_else(|| panic!("peek: seed missing")),
                "next_char" => pool_seed(f, &["record_completion"])
                    .unwrap_or_else(|| panic!("next_char: seed missing")),
                _ => pool_seed(f, &["equation", "char_equation"])
                    .unwrap_or_else(|| panic!("{}: constraint seed missing", f.name)),
            };
            winners.insert(f.name.clone(), pick);
        }
        winners
    }

    /// The hand-written mirror, copied verbatim from
    /// src/tokenizer_benchmark.rs (MIRROR_SRC).
    const MIRROR_SRC: &str = r#"
const KIND_IDENT: i64 = 0;
const KIND_LPAREN: i64 = 3;
const KIND_RPAREN: i64 = 4;
const KIND_LBRACE: i64 = 5;
const KIND_RBRACE: i64 = 6;
const KIND_COLON: i64 = 7;
const KIND_COMMA: i64 = 9;
const KIND_LT: i64 = 10;
const KIND_GT: i64 = 11;
const KIND_EQ: i64 = 12;

fn peek(state: &TokenizerState) -> Option<char> {
    state.source.chars().nth(state.pos as usize)
}

fn next_char(state: TokenizerState) -> TokenizerState {
    let ch = state.source.chars().nth(state.pos as usize);
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

fn push_token(
    start: &TokenizerState,
    state: TokenizerState,
    kind: i64,
    lexeme: String,
) -> TokenizerState {
    let mut tokens = state.tokens;
    tokens.push(Token {
        kind,
        lexeme,
        line: start.line,
        col: start.col,
    });
    TokenizerState { tokens, ..state }
}

fn tokenize_one(state: TokenizerState) -> Result<TokenizerState, String> {
    let state = skip_whitespace(state);
    match peek(&state) {
        None => Ok(state),
        Some(c) if is_ident_start(c) => {
            let mut lexeme = String::new();
            let mut s = state.clone();
            while peek(&s).map_or(false, |ch| is_ident_continue(ch)) {
                if let Some(ch) = peek(&s) {
                    lexeme.push(ch);
                }
                s = next_char(s);
            }
            Ok(push_token(&state, s, KIND_IDENT, lexeme))
        }
        Some(':') => Ok(push_token(&state, next_char(state.clone()), KIND_COLON, ":".into())),
        Some('{') => Ok(push_token(&state, next_char(state.clone()), KIND_LBRACE, "{".into())),
        Some('}') => Ok(push_token(&state, next_char(state.clone()), KIND_RBRACE, "}".into())),
        Some('(') => Ok(push_token(&state, next_char(state.clone()), KIND_LPAREN, "(".into())),
        Some(')') => Ok(push_token(&state, next_char(state.clone()), KIND_RPAREN, ")".into())),
        Some(',') => Ok(push_token(&state, next_char(state.clone()), KIND_COMMA, ",".into())),
        Some('<') => Ok(push_token(&state, next_char(state.clone()), KIND_LT, "<".into())),
        Some('>') => Ok(push_token(&state, next_char(state.clone()), KIND_GT, ">".into())),
        Some('=') => Ok(push_token(&state, next_char(state.clone()), KIND_EQ, "=".into())),
        Some(other) => Err(format!(
            "Unexpected character '{}' at line {}",
            other, state.line
        )),
    }
}

fn tokenize_all(mut state: TokenizerState) -> Result<Vec<Token>, String> {
    while peek(&state).is_some() {
        state = tokenize_one(state)?;
    }
    Ok(state.tokens)
}
"#;

    /// The workload functions, copied from the Phase 86
    /// driver (src/tokenizer_benchmark.rs DRIVER_SRC) minus
    /// its main; the profile supplies its own mains.
    const WORKLOADS_SRC: &str = r#"
const BATTERY_REPS: usize = 20000;
const BIG_REPS: usize = 3;

fn checksum_token(acc: &mut u64, t: &Token) {
    *acc = acc.wrapping_mul(31).wrapping_add(t.kind as u64);
    for b in t.lexeme.as_bytes() {
        *acc = acc.wrapping_mul(31).wrapping_add(*b as u64);
    }
    *acc = acc.wrapping_mul(31).wrapping_add(t.line as u64);
    *acc = acc.wrapping_mul(31).wrapping_add(t.col as u64);
}

fn workload_battery(inputs: &[String], reps: usize) -> (u64, u64, u64) {
    let (mut acc, mut ntok, mut nerr) = (0u64, 0u64, 0u64);
    for _ in 0..reps {
        for src in inputs {
            let st = TokenizerState {
                source: src.clone(),
                pos: 0,
                line: 1,
                col: 1,
                tokens: vec![],
            };
            match tokenize_all(st) {
                Ok(ts) => {
                    for t in &ts {
                        checksum_token(&mut acc, t);
                    }
                    ntok += ts.len() as u64;
                }
                Err(_) => {
                    nerr += 1;
                    acc = acc.wrapping_mul(31).wrapping_add(0xE22);
                }
            }
        }
    }
    (acc, ntok, nerr)
}

fn workload_big(text: &str, reps: usize) -> (u64, u64, u64, u64) {
    let (mut acc, mut ntok, mut nerr, mut anomalies) = (0u64, 0u64, 0u64, 0u64);
    let n = text.chars().count() as i64;
    for _ in 0..reps {
        let mut state = TokenizerState {
            source: text.to_string(),
            pos: 0,
            line: 1,
            col: 1,
            tokens: vec![],
        };
        let mut steps: i64 = 0;
        while state.pos < n {
            steps += 1;
            if steps > 20 * n + 100 {
                anomalies += 1;
                break;
            }
            let backup = state.clone();
            let before = backup.pos;
            match tokenize_one(state) {
                Ok(out) => {
                    if out.pos <= before {
                        anomalies += 1;
                        state = backup;
                        break;
                    }
                    for t in &out.tokens {
                        checksum_token(&mut acc, t);
                    }
                    ntok += out.tokens.len() as u64;
                    state = TokenizerState { tokens: vec![], ..out };
                }
                Err(_) => {
                    nerr += 1;
                    acc = acc.wrapping_mul(31).wrapping_add(0xE22);
                    let mut recovered = backup;
                    recovered.pos += 1;
                    if recovered.source.chars().nth(before as usize) == Some('\n') {
                        recovered.line += 1;
                        recovered.col = 1;
                    } else {
                        recovered.col += 1;
                    }
                    recovered.tokens = vec![];
                    state = recovered;
                }
            }
        }
        acc = acc.wrapping_mul(31).wrapping_add(state.pos as u64);
    }
    (acc, ntok, nerr, anomalies)
}
"#;

    /// The Phase 86 timing main, verbatim in shape.
    const TIMING_MAIN: &str = r#"
fn main() {
    let battery = corpus_battery();
    let (a1, t1, e1) = workload_battery(&battery, 1);
    let (a2, t2, e2, an2) = workload_big(BIG_TEXT, 1);
    println!("BENCH-EQ|battery|acc={}|tokens={}|errs={}", a1, t1, e1);
    println!("BENCH-EQ|big|acc={}|tokens={}|errs={}|anomalies={}", a2, t2, e2, an2);
    let started = std::time::Instant::now();
    let r = workload_battery(&battery, BATTERY_REPS);
    println!("BENCH-TIME|battery|ns={}|acc={}", started.elapsed().as_nanos(), r.0);
    let started = std::time::Instant::now();
    let r = workload_big(BIG_TEXT, BIG_REPS);
    println!("BENCH-TIME|big|ns={}|acc={}", started.elapsed().as_nanos(), r.0);
}
"#;

    /// The accounting main: one pass per workload, then
    /// the clone counters.
    const COUNTING_MAIN: &str = r#"
fn snapshot(tag: &str) {
    println!(
        "CLONES|{}|state_clones={}|token_clones={}|state_clone_src_bytes={}",
        tag,
        STATE_CLONES.load(std::sync::atomic::Ordering::Relaxed),
        TOKEN_CLONES.load(std::sync::atomic::Ordering::Relaxed),
        STATE_CLONE_SRC_BYTES.load(std::sync::atomic::Ordering::Relaxed)
    );
}
fn main() {
    let battery = corpus_battery();
    let (a1, t1, e1) = workload_battery(&battery, 1);
    println!("BENCH-EQ|battery|acc={}|tokens={}|errs={}", a1, t1, e1);
    snapshot("after_battery_1pass");
    let (a2, t2, e2, an2) = workload_big(BIG_TEXT, 1);
    println!("BENCH-EQ|big|acc={}|tokens={}|errs={}|anomalies={}", a2, t2, e2, an2);
    snapshot("after_big_1pass");
}
"#;

    // ── Ablation replacement bodies (harness shape) ──

    /// P1: peek without the count scan — the same
    /// function, stated through nth alone.
    const PEEK_NTH_SRC: &str = r#"
pub fn peek(state: TokenizerState) -> Option<char> {
    state.source.chars().nth(state.pos as usize)
}
"#;

    /// P2: next_char with the peeked char computed once;
    /// the record still clones its carried fields, as
    /// the production RecordLit renders them.
    const NEXT_CHAR_ONCE_CLONED_SRC: &str = r#"
pub fn next_char(state: TokenizerState) -> TokenizerState {
    let nl = state.source.chars().nth(state.pos as usize) == Some('\n');
    TokenizerState { source: state.source.clone(), pos: (state.pos + 1), line: if nl { (state.line + 1) } else { state.line }, col: if nl { 1 } else { (state.col + 1) }, tokens: state.tokens.clone() }
}
"#;

    /// P3: next_char, char computed once, record built
    /// by struct update — carried fields move.
    const NEXT_CHAR_ONCE_MOVED_SRC: &str = r#"
pub fn next_char(state: TokenizerState) -> TokenizerState {
    let is_newline = state.source.chars().nth(state.pos as usize) == Some('\n');
    TokenizerState {
        pos: state.pos + 1,
        line: if is_newline { state.line + 1 } else { state.line },
        col: if is_newline { 1 } else { state.col + 1 },
        ..state
    }
}
"#;

    /// P4: tokenize_one with the template's own clone
    /// points removed — the ident loop peeks once per
    /// char; the punctuation arms bind line/col and
    /// move the post-skip state into next_char. The
    /// closing records keep the production shape
    /// (carried source cloned), so the delta prices
    /// only the loop/dispatch clone points.
    const TOK_ONE_SINGLE_PEEK_SRC: &str = r#"
pub fn tokenize_one(state: TokenizerState) -> StepResult {
    let mut s: TokenizerState = skip_whitespace(state);
    return match peek(s.clone()) { None => Ok(s), Some(c) => if is_ident_start(c) { { let mut lexeme: String = "".to_string();
let mut cur: TokenizerState = s.clone();
loop {
    match peek(cur.clone()) {
        None => break,
        Some(ch) => {
            if !is_ident_continue(ch) { break; }
            lexeme.push(ch);
            cur = next_char(cur);
        },
    }
}
let mut toks: Vec<Token> = cur.tokens;
toks.push(Token { kind: 0, lexeme: lexeme, line: s.line, col: s.col });
return Ok(TokenizerState { source: cur.source.clone(), pos: cur.pos, line: cur.line, col: cur.col, tokens: toks }); } } else { if (c == '(') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 3, lexeme: "(".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { if (c == ')') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 4, lexeme: ")".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { if (c == '{') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 5, lexeme: "{".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { if (c == '}') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 6, lexeme: "}".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { if (c == ':') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 7, lexeme: ":".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { if (c == ',') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 9, lexeme: ",".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { if (c == '<') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 10, lexeme: "<".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { if (c == '>') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 11, lexeme: ">".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { if (c == '=') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 12, lexeme: "=".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { { let mut msg: String = "Unexpected character".to_string();
return Err(msg); } } } } } } } } } } } };
}
"#;

    /// P5 (convention ceiling): peek by reference.
    const PEEK_REF_SRC: &str = r#"
pub fn peek(state: &TokenizerState) -> Option<char> {
    state.source.chars().nth(state.pos as usize)
}
"#;

    const SKIP_REF_SRC: &str = r#"
pub fn skip_whitespace(state: TokenizerState) -> TokenizerState {
    let mut cur: TokenizerState = state;
    while (match peek(&cur) { None => false, Some(ch) => is_whitespace(ch) }) {
        cur = next_char(cur);
    }
    return cur;
}
"#;

    const TOK_ONE_REF_SRC: &str = r#"
pub fn tokenize_one(state: TokenizerState) -> StepResult {
    let mut s: TokenizerState = skip_whitespace(state);
    return match peek(&s) { None => Ok(s), Some(c) => if is_ident_start(c) { { let mut lexeme: String = "".to_string();
let mut cur: TokenizerState = s.clone();
loop {
    match peek(&cur) {
        None => break,
        Some(ch) => {
            if !is_ident_continue(ch) { break; }
            lexeme.push(ch);
            cur = next_char(cur);
        },
    }
}
let mut toks: Vec<Token> = cur.tokens;
toks.push(Token { kind: 0, lexeme: lexeme, line: s.line, col: s.col });
return Ok(TokenizerState { source: cur.source.clone(), pos: cur.pos, line: cur.line, col: cur.col, tokens: toks }); } } else { if (c == '(') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 3, lexeme: "(".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { if (c == ')') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 4, lexeme: ")".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { if (c == '{') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 5, lexeme: "{".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { if (c == '}') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 6, lexeme: "}".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { if (c == ':') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 7, lexeme: ":".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { if (c == ',') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 9, lexeme: ",".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { if (c == '<') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 10, lexeme: "<".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { if (c == '>') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 11, lexeme: ">".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { if (c == '=') { { let (stamp_line, stamp_col) = (s.line, s.col);
let mut end: TokenizerState = next_char(s);
let mut toks: Vec<Token> = end.tokens;
toks.push(Token { kind: 12, lexeme: "=".to_string(), line: stamp_line, col: stamp_col });
return Ok(TokenizerState { source: end.source.clone(), pos: end.pos, line: end.line, col: end.col, tokens: toks }); } } else { { let mut msg: String = "Unexpected character".to_string();
return Err(msg); } } } } } } } } } } } };
}
"#;

    const TOK_ALL_REF_SRC: &str = r#"
pub fn tokenize_all(state: TokenizerState) -> Result<Vec<Token>, String> {
    let mut s: TokenizerState = state;
    while (peek(&s) != None) {
        let mut step: TokenizerState = (tokenize_one(s))?;
        s = step;
    }
    return Ok(s.tokens);
}
"#;

    /// The shared corpus sources, copied from the
    /// Phase 86 harness.
    fn battery_sources() -> Vec<String> {
        let mut all: Vec<String> = Vec::new();
        for s in [
            "", "spec sort", "spec{input:()}", "spec 5", "  \n ", "a", "x{y}:(,)", "a\nb",
            "spec", "  spec", ":", "{", "=", "5", " ", "a b", "\n\nx",
            "ab", "é😀x", " \t\r\n",
            "  \t\n x", "no-ws", "\n\nab", " \u{A0}x", "\r\n\t",
        ] {
            if !all.iter().any(|x| x == s) {
                all.push(s.to_string());
            }
        }
        all
    }

    fn emit_corpus(battery: &[String], big: &str) -> String {
        let lits: Vec<String> = battery
            .iter()
            .map(|s| format!("{:?}.to_string()", s))
            .collect();
        format!(
            "\nfn corpus_battery() -> Vec<String> {{ vec![{}] }}\nconst BIG_TEXT: &str = {:?};\n",
            lits.join(", "),
            big
        )
    }

    const RUN_TIMEOUT_SECS: u64 = 180;

    fn compile_bench(src_path: &std::path::Path, bin_path: &std::path::Path) -> Result<(), String> {
        let compile = Command::new("rustc")
            .arg("--edition")
            .arg("2021")
            .arg("-C")
            .arg("opt-level=3")
            .arg(src_path)
            .arg("-o")
            .arg(bin_path)
            .output()
            .map_err(|e| format!("spawn rustc: {}", e))?;
        if !compile.status.success() {
            let stderr = String::from_utf8_lossy(&compile.stderr);
            let errors: Vec<&str> = stderr
                .lines()
                .filter(|l| l.starts_with("error"))
                .take(4)
                .collect();
            return Err(format!("rustc failed: {}", errors.join(" | ")));
        }
        Ok(())
    }

    fn run_bench(bin_path: &std::path::Path, tag: &str) -> Result<Vec<String>, String> {
        let dir = std::env::temp_dir().join(format!(
            "morphic_prof87_{}_{}",
            std::process::id(),
            tag
        ));
        std::fs::create_dir_all(&dir).map_err(|e| format!("temp dir: {}", e))?;
        let stdout_path = dir.join("stdout.txt");
        let stdout_file =
            std::fs::File::create(&stdout_path).map_err(|e| format!("stdout file: {}", e))?;
        let mut child = Command::new(bin_path)
            .stdout(std::process::Stdio::from(stdout_file))
            .spawn()
            .map_err(|e| format!("spawn bench: {}", e))?;
        let deadline = Instant::now() + Duration::from_secs(RUN_TIMEOUT_SECS);
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        let _ = std::fs::remove_dir_all(&dir);
                        return Err(format!(
                            "bench timed out after {}s",
                            RUN_TIMEOUT_SECS
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => return Err(format!("wait bench: {}", e)),
            }
        };
        if !status.success() {
            return Err(format!("bench exited {}", status));
        }
        let stdout =
            std::fs::read_to_string(&stdout_path).map_err(|_| "bench output is not UTF-8")?;
        let _ = std::fs::remove_dir_all(&dir);
        Ok(stdout.lines().map(|l| l.to_string()).collect())
    }

    fn median(mut v: Vec<u128>) -> u128 {
        v.sort_unstable();
        v[v.len() / 2]
    }

    const OUTER_RUNS: usize = 3;

    #[test]
    #[ignore = "profile instrument: assembles the Phase 86 composition plus ablation variants, equality-gates each, times them; run explicitly with --ignored --nocapture"]
    fn tokenizer_profile() {
        let root = env!("CARGO_MANIFEST_DIR");
        let out_dir = format!("{}/target/tokenizer-profile", root);
        std::fs::create_dir_all(&out_dir).unwrap();
        let (functions, module) = load_tokenizer_module();
        assert_eq!(functions.len(), 8, "tokenizer module spec count");

        // Production-generated code per function (the
        // Phase 86 assembly, per part).
        let winners = assemble_winners(&functions, &module);
        let mut production: HashMap<String, String> = HashMap::new();
        for f in &functions {
            let w = &winners[&f.name];
            let code = codegen::generate(w, "rust")
                .unwrap_or_else(|e| panic!("codegen failed for {}: {}", f.name, e));
            let stripped = code.split("#[cfg(test)]").next().unwrap_or("").to_string();
            production.insert(f.name.clone(), stripped);
        }

        let big_text = std::fs::read_to_string(format!(
            "{}/examples/functional_tokenizer.morph",
            root
        ))
        .unwrap();
        let battery = battery_sources();
        let corpus_src = emit_corpus(&battery, &big_text);

        // Variant table: name → (function → replacement).
        let variants: Vec<(&str, Vec<(&str, &str)>)> = vec![
            ("P0_production", vec![]),
            ("P1_peek_nth", vec![("peek", PEEK_NTH_SRC)]),
            (
                "P2_nextchar_once_cloned",
                vec![
                    ("peek", PEEK_NTH_SRC),
                    ("next_char", NEXT_CHAR_ONCE_CLONED_SRC),
                ],
            ),
            (
                "P3_nextchar_once_moved",
                vec![
                    ("peek", PEEK_NTH_SRC),
                    ("next_char", NEXT_CHAR_ONCE_MOVED_SRC),
                ],
            ),
            (
                "P4_template_single_peek",
                vec![
                    ("peek", PEEK_NTH_SRC),
                    ("next_char", NEXT_CHAR_ONCE_MOVED_SRC),
                    ("tokenize_one", TOK_ONE_SINGLE_PEEK_SRC),
                ],
            ),
            (
                "P5_peek_by_ref_CEILING",
                vec![
                    ("peek", PEEK_REF_SRC),
                    ("next_char", NEXT_CHAR_ONCE_MOVED_SRC),
                    ("skip_whitespace", SKIP_REF_SRC),
                    ("tokenize_one", TOK_ONE_REF_SRC),
                    ("tokenize_all", TOK_ALL_REF_SRC),
                ],
            ),
        ];

        let dir = std::env::temp_dir().join(format!("morphic_prof87_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        // Assemble + compile every variant and the mirror.
        let mut bins: Vec<(String, std::path::PathBuf)> = Vec::new();
        for (vname, replacements) in &variants {
            let mut src = String::new();
            src.push_str(RUNNER_PRELUDE);
            for f in &functions {
                if let Some((_, rep)) = replacements.iter().find(|(n, _)| n == &f.name) {
                    src.push_str(rep);
                } else {
                    src.push_str(&production[&f.name]);
                }
                src.push('\n');
            }
            src.push_str(&corpus_src);
            src.push_str(WORKLOADS_SRC);
            src.push_str(TIMING_MAIN);
            let path = std::path::PathBuf::from(format!("{}/{}.rs", out_dir, vname));
            std::fs::write(&path, &src).unwrap();
            let bin = dir.join(format!("bench_{}", vname));
            if let Err(e) = compile_bench(&path, &bin) {
                panic!("variant {} does not compile: {}", vname, e);
            }
            bins.push((vname.to_string(), bin));
        }
        let mir_path = std::path::PathBuf::from(format!("{}/mirror.rs", out_dir));
        let mut mir_src = String::new();
        mir_src.push_str(RUNNER_PRELUDE);
        mir_src.push_str(MIRROR_SRC);
        mir_src.push_str(&corpus_src);
        mir_src.push_str(WORKLOADS_SRC);
        mir_src.push_str(TIMING_MAIN);
        std::fs::write(&mir_path, &mir_src).unwrap();
        let mir_bin = dir.join("bench_mirror");
        if let Err(e) = compile_bench(&mir_path, &mir_bin) {
            panic!("mirror does not compile: {}", e);
        }
        bins.push(("mirror".to_string(), mir_bin));

        // Run all sides, alternating, OUTER_RUNS times.
        let mut eq: HashMap<String, Vec<String>> = HashMap::new();
        let mut times: HashMap<(String, String), Vec<u128>> = HashMap::new();
        for run in 0..OUTER_RUNS {
            for (side, bin) in &bins {
                let lines = run_bench(bin, &format!("{}_{}", side, run))
                    .unwrap_or_else(|e| panic!("{} run {}: {}", side, run, e));
                for line in &lines {
                    if let Some(rest) = line.strip_prefix("BENCH-EQ|") {
                        eq.entry(side.clone()).or_default().push(rest.to_string());
                    } else if let Some(rest) = line.strip_prefix("BENCH-TIME|") {
                        let mut parts = rest.split('|');
                        let workload = parts.next().unwrap_or("").to_string();
                        let ns: u128 = parts
                            .next()
                            .and_then(|p| p.strip_prefix("ns="))
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0);
                        times
                            .entry((side.clone(), workload))
                            .or_default()
                            .push(ns);
                    }
                }
            }
        }

        // Equality gate: every variant must agree with
        // the mirror on both workloads, every run.
        let mir_eq = eq.get("mirror").cloned().unwrap_or_default();
        assert_eq!(mir_eq.len(), 2 * OUTER_RUNS, "mirror EQ lines");
        for (vname, _) in &bins {
            let v_eq = eq.get(vname).cloned().unwrap_or_default();
            assert_eq!(
                v_eq.len(),
                2 * OUTER_RUNS,
                "{} printed {} EQ lines",
                vname,
                v_eq.len()
            );
            for i in 0..OUTER_RUNS {
                assert_eq!(
                    v_eq[2 * i],
                    mir_eq[2 * i],
                    "{} disagrees with mirror on battery (run {})",
                    vname,
                    i
                );
                assert_eq!(
                    v_eq[2 * i + 1],
                    mir_eq[2 * i + 1],
                    "{} disagrees with mirror on big (run {})",
                    vname,
                    i
                );
            }
        }
        println!("PROFILE-EQ-MATCH|all_variants=true");

        // Report: medians + ratio vs mirror per variant.
        for workload in ["battery", "big"] {
            let m = median(
                times
                    .get(&("mirror".to_string(), workload.to_string()))
                    .cloned()
                    .unwrap(),
            );
            for (vname, _) in &bins {
                let g = median(
                    times
                        .get(&(vname.clone(), workload.to_string()))
                        .cloned()
                        .unwrap(),
                );
                println!(
                    "PROFILE-RESULT|workload={}|variant={}|median_ns={}|mirror_median_ns={}|ratio_vs_mirror={:.4}",
                    workload,
                    vname,
                    g,
                    m,
                    g as f64 / m as f64
                );
            }
        }

        // ── Clone accounting: P0 generated + mirror under
        // the counting prelude, one pass per workload. ──
        for (side, parts) in [
            ("generated", None),
            ("mirror", Some(MIRROR_SRC)),
        ] {
            let mut src = String::new();
            src.push_str(COUNTING_PRELUDE);
            match parts {
                None => {
                    for f in &functions {
                        src.push_str(&production[&f.name]);
                        src.push('\n');
                    }
                }
                Some(m) => src.push_str(m),
            }
            src.push_str(&corpus_src);
            src.push_str(WORKLOADS_SRC);
            src.push_str(COUNTING_MAIN);
            let path = std::path::PathBuf::from(format!("{}/counting_{}.rs", out_dir, side));
            std::fs::write(&path, &src).unwrap();
            let bin = dir.join(format!("counting_{}", side));
            if let Err(e) = compile_bench(&path, &bin) {
                panic!("counting {} does not compile: {}", side, e);
            }
            let lines = run_bench(&bin, &format!("counting_{}", side))
                .unwrap_or_else(|e| panic!("counting {}: {}", side, e));
            for line in &lines {
                println!("PROFILE-COUNT|side={}|{}", side, line);
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
