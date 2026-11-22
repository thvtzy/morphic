// ─────────────────────────────────────────────────────────────
//  TOKENIZER BENCHMARK HARNESS (test-only) — Phase 86
//  (M1-features closing run: the M1 gate's benchmark clause
//  gets its first harness)
//
//  The M1 checklist's benchmark clause reads, verbatim:
//  "Benchmark: must be ≤ 2x slower". It does not name the
//  baseline or the timed quantity in its own words; the
//  checklist's own context (the cross-validation clause
//  directly above it pairs "generated tokenizer vs Rust
//  original") supplies the baseline, and this harness
//  measures the GENERATED-CODE-RUNTIME reading: the wall
//  time of the assembled generated tokenizer tokenizing a
//  shared corpus, against the hand-written Rust mirror
//  tokenizer tokenizing the same corpus, same machine,
//  same harness shape. The alternative reading
//  (synthesis-pipeline time) is recorded as an ambiguity
//  for the gate re-run, not resolved here — a ratio is a
//  measurement, not a verdict; Phase 87 judges the clause.
//
//  THE GENERATED SIDE is the module's actual synthesis
//  output, assembled exactly as Phase 85's Leg 2 assembly
//  assembled it (the composition whose executed agreement
//  Phase 85 measured at 9/10 + 7/8; Phase 101 re-pin:
//  the F2 payoff closed the Err-text deviation, and the
//  composition's executed agreement now stands at
//  10/10 + 8/8 — see the cross-validation crowns):
//   - is_whitespace / is_ident_start / is_ident_continue:
//     the pools' equation seeds — also the crowned winners
//     (Phases 76/81: deterministic Verified crowns).
//   - peek: the complementary_guard seed (its crowned
//     winner, Phase 81). next_char: the record_completion
//     seed (its crowned winner, Phase 81).
//   - tokenize_one / tokenize_all: the tokenize_template
//     seeds (seed family #11, Phase 85 Leg 2).
//   - skip_whitespace: the crowned winner is a Mutation
//     body that satisfies the spec's two stated
//     postconditions without performing the skip the
//     composition contracts on (the Phase 47/80
//     spec-strength fact; mirror agreement 4/12), so —
//     exactly as Leg 2's assembly decided — the
//     composition carries the HAND-SUPPLIED mirror body
//     (provenance Template "phase85_hand_mirror_skip"),
//     labeled here and in the report. No part of the
//     composition is silently hand-written: this is the
//     one hand part, and it is named.
//  Each part is rendered by the production codegen
//  (codegen::generate, rust target) and concatenated
//  after the cross-validation harness's runner prelude.
//
//  THE MIRROR SIDE is tests/tokenizer_proof.rs's
//  hand-written tokenizer — the component's Rust
//  original — transcribed into the same harness shape
//  (the prelude's TokenizerState / Token types), with
//  the one adaptation the cross-validation harness's
//  copies also carry: integer types (the proof uses
//  usize positions and a TokenKind enum; the harness
//  shape uses i64 throughout, kinds as the spec's Int
//  codes). Function bodies are otherwise the proof's.
//
//  THE DRIVER is byte-identical text on both sides:
//   - workload "battery": tokenize_all over the deduped
//     battery sources (the cross-validation harness's
//     four state batteries' sources), BATTERY_REPS passes.
//   - workload "big": a tokenize_one recovery loop over
//     the tokenizer module's own text
//     (examples/functional_tokenizer.morph, read from
//     disk): the state is threaded by value; on Err the
//     driver advances one char past the pre-call position
//     (line/col fixed from the pre-call char) and
//     continues, so the whole text is processed on both
//     sides. The token accumulator is drained per step
//     (the per-step production is checksummed, then the
//     state continues with an empty vec) so the threaded
//     state's clone cost stays the source string's, on
//     both sides equally.
//  Both workloads checksum every produced token (kind,
//  lexeme bytes, line, col) plus a bare error marker —
//  the error MESSAGE text never enters the checksum
//  (the Phase 85 named deviation: template
//  "Unexpected character" vs mirror "Unexpected
//  character '5' at line 1" — Phase 101 re-pin: that
//  deviation is CLOSED, the template now builds the
//  mirror's full interpolated text; the checksum still
//  excludes message text by design, and full-text
//  equality is the cross-validation crowns' evidence,
//  10/10 + 8/8 on both routes). The instrument asserts
//  the two sides' checksums, token counts, error counts
//  and anomaly counts are EQUAL before any timing is
//  reported: the timed work is the same work.
//
//  PROFILE: each side is a single standalone file
//  compiled with `rustc --edition 2021 -C opt-level=3`
//  — release-equivalent optimization (the crate's own
//  release profile is opt-level 3; its LTO /
//  codegen-units=1 settings do not apply to a
//  single-file harness and are not claimed). Timing:
//  OUTER_RUNS runs per side, alternating sides run by
//  run; per workload the report gives each side's
//  median, the ratio of medians, and the min–max spread
//  of the per-run ratios. Machine: this sandbox VM
//  (shared vCPU — spread is reported, not hidden).
//
//  Run with:
//
//    cargo test -- --ignored --nocapture tokenizer_benchmark
//
//  The assembled sources are written to
//  target/tokenizer-benchmark/{generated,mirror}.rs.
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod bench_tok {
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
    /// verbatim (src/tokenizer_cross_validation.rs): the
    /// harness shape both sides are compiled into.
    const RUNNER_PRELUDE: &str = r#"
#[derive(Debug, Clone, PartialEq)]
pub struct Token { pub kind: i64, pub lexeme: String, pub line: i64, pub col: i64 }
#[derive(Debug, Clone, PartialEq)]
pub struct TokenizerState { pub source: String, pub pos: i64, pub line: i64, pub col: i64, pub tokens: Vec<Token> }
pub type StepResult = Result<TokenizerState, String>;
fn show_tokens(ts: &Vec<Token>) -> String { ts.iter().map(|t| format!("{},{:?},{},{}", t.kind, t.lexeme, t.line, t.col)).collect::<Vec<_>>().join(";") }
fn show_state(s: &TokenizerState) -> String { format!("{}|{}|{}|{}|{:?}|{}", s.pos, s.line, s.col, s.source.chars().count(), s.source, show_tokens(&s.tokens)) }
"#;

    /// The Phase 85 Leg 2 assembly, replicated: pool seeds
    /// for every spec the pools carry them for, and the
    /// hand-supplied mirror body for skip_whitespace.
    /// (Copied from phase85_tokenize_template_measurement
    /// in src/tokenizer_cross_validation.rs.)
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
                    // The hand-supplied mirror body: while
                    // the peeked char is whitespace, advance.
                    // Its candidate shell is cloned from
                    // peek's seed (skip has no seed of its
                    // own); body, spec and provenance are
                    // replaced outright below.
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

    /// The hand-written Rust mirror tokenizer —
    /// tests/tokenizer_proof.rs's functions transcribed
    /// into the harness shape (the prelude's types; i64
    /// positions and kind codes in place of the proof's
    /// usize / TokenKind enum — the cross-validation
    /// harness's documented adaptation). Bodies are
    /// otherwise the proof's, including its D4a
    /// START-stamping push_token signature.
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

    /// The driver — byte-identical text appended to both
    /// sides (the corpus functions and BIG_TEXT are
    /// emitted by the instrument, identically for both).
    const DRIVER_SRC: &str = r#"
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

    /// The shared corpus: the deduped sources of the
    /// cross-validation harness's four state batteries
    /// (tokenize_all, tokenize_one, peek/next_char,
    /// skip_whitespace), in battery order.
    fn battery_sources() -> Vec<String> {
        let mut all: Vec<String> = Vec::new();
        for s in [
            // tokenize_all battery
            "", "spec sort", "spec{input:()}", "spec 5", "  \n ", "a", "x{y}:(,)", "a\nb",
            // tokenize_one battery
            "spec", "  spec", ":", "{", "=", "5", " ", "a b", "\n\nx",
            // peek / next_char state battery sources
            "ab", "é😀x", " \t\r\n",
            // skip_whitespace battery sources
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

    const OUTER_RUNS: usize = 5;
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

    /// Run one bench binary once, under the standing
    /// wall-clock discipline (a hung binary is killed and
    /// reported, never hangs the harness).
    fn run_bench(bin_path: &std::path::Path, tag: &str) -> Result<Vec<String>, String> {
        let dir = std::env::temp_dir().join(format!(
            "morphic_bench86_{}_{}",
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

    #[test]
    #[ignore = "benchmark harness: assembles the generated tokenizer (Phase 85 Leg 2 composition) and the hand-written mirror, compiles both at opt-level 3, times them over the shared corpus; run explicitly with --ignored --nocapture"]
    fn tokenizer_benchmark() {
        let root = env!("CARGO_MANIFEST_DIR");
        let out_dir = format!("{}/target/tokenizer-benchmark", root);
        std::fs::create_dir_all(&out_dir).unwrap();
        let (functions, module) = load_tokenizer_module();
        assert_eq!(functions.len(), 8, "tokenizer module spec count");

        // ── Assemble the generated side ──
        let winners = assemble_winners(&functions, &module);
        let provenance: Vec<(&str, &str)> = functions
            .iter()
            .map(|f| {
                let part = match f.name.as_str() {
                    "skip_whitespace" => {
                        "hand-supplied mirror body (Template phase85_hand_mirror_skip) — the composition's one hand part, labeled"
                    }
                    "tokenize_one" | "tokenize_all" => {
                        "synthesis pool seed, family tokenize_template (#11)"
                    }
                    "peek" => "synthesis pool seed complementary_guard — also the crowned winner",
                    "next_char" => {
                        "synthesis pool seed record_completion — also the crowned winner"
                    }
                    _ => "synthesis pool seed equation — also the crowned winner",
                };
                (f.name.as_str(), part)
            })
            .collect();
        for (name, part) in &provenance {
            println!("BENCH-PROV|{}|{}", name, part);
        }
        let mut gen_src = String::new();
        gen_src.push_str(RUNNER_PRELUDE);
        for f in &functions {
            let w = &winners[&f.name];
            match codegen::generate(w, "rust") {
                Ok(code) => {
                    let stripped = code.split("#[cfg(test)]").next().unwrap_or("");
                    gen_src.push_str(stripped);
                    gen_src.push('\n');
                }
                Err(e) => panic!("generated side: codegen failed for {}: {}", f.name, e),
            }
        }

        // ── The shared corpus + driver, identical both sides ──
        let big_text = std::fs::read_to_string(format!(
            "{}/examples/functional_tokenizer.morph",
            root
        ))
        .unwrap();
        let battery = battery_sources();
        let corpus_src = emit_corpus(&battery, &big_text);
        gen_src.push_str(&corpus_src);
        gen_src.push_str(DRIVER_SRC);

        let mut mir_src = String::new();
        mir_src.push_str(RUNNER_PRELUDE);
        mir_src.push_str(MIRROR_SRC);
        mir_src.push_str(&corpus_src);
        mir_src.push_str(DRIVER_SRC);

        let gen_path = std::path::PathBuf::from(format!("{}/generated.rs", out_dir));
        let mir_path = std::path::PathBuf::from(format!("{}/mirror.rs", out_dir));
        std::fs::write(&gen_path, &gen_src).unwrap();
        std::fs::write(&mir_path, &mir_src).unwrap();
        println!(
            "BENCH-INFO|generated_src_bytes={}|mirror_src_bytes={}|battery_sources={}|battery_chars={}|big_chars={}|big_bytes={}",
            gen_src.len(),
            mir_src.len(),
            battery.len(),
            battery.iter().map(|s| s.chars().count()).sum::<usize>(),
            big_text.chars().count(),
            big_text.len()
        );

        // ── Compile both sides ──
        let dir = std::env::temp_dir().join(format!("morphic_bench86_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let gen_bin = dir.join("bench_generated");
        let mir_bin = dir.join("bench_mirror");
        if let Err(e) = compile_bench(&gen_path, &gen_bin) {
            panic!("generated side does not compile: {}", e);
        }
        if let Err(e) = compile_bench(&mir_path, &mir_bin) {
            panic!("mirror side does not compile: {}", e);
        }
        println!("BENCH-INFO|compiled=both|rustc_opt_level=3");

        // ── Run, alternating sides ──
        let mut eq: HashMap<&str, Vec<String>> = HashMap::new();
        let mut times: HashMap<(&str, &str), Vec<u128>> = HashMap::new();
        for run in 0..OUTER_RUNS {
            for (side, bin) in [("generated", &gen_bin), ("mirror", &mir_bin)] {
                let lines = run_bench(bin, &format!("{}_{}", side, run))
                    .unwrap_or_else(|e| panic!("{} run {}: {}", side, run, e));
                for line in &lines {
                    if let Some(rest) = line.strip_prefix("BENCH-EQ|") {
                        eq.entry(side).or_default().push(rest.to_string());
                    } else if let Some(rest) = line.strip_prefix("BENCH-TIME|") {
                        // BENCH-TIME|<workload>|ns=<n>|acc=<a>
                        let mut parts = rest.split('|');
                        let workload: &'static str = match parts.next().unwrap_or("") {
                            "battery" => "battery",
                            "big" => "big",
                            _ => continue,
                        };
                        let ns: u128 = parts
                            .next()
                            .and_then(|p| p.strip_prefix("ns="))
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0);
                        times.entry((side, workload)).or_default().push(ns);
                    }
                }
            }
        }

        // ── The equality gate: the timed work is the same work ──
        let gen_eq = eq.get("generated").cloned().unwrap_or_default();
        let mir_eq = eq.get("mirror").cloned().unwrap_or_default();
        assert_eq!(
            gen_eq.len(),
            2 * OUTER_RUNS,
            "generated side printed {} EQ lines over {} runs",
            gen_eq.len(),
            OUTER_RUNS
        );
        assert_eq!(mir_eq.len(), 2 * OUTER_RUNS, "mirror side EQ lines");
        for i in 0..OUTER_RUNS {
            assert_eq!(
                gen_eq[2 * i],
                mir_eq[2 * i],
                "battery workload: sides disagree (run {})",
                i
            );
            assert_eq!(
                gen_eq[2 * i + 1],
                mir_eq[2 * i + 1],
                "big workload: sides disagree (run {})",
                i
            );
        }
        println!("BENCH-EQ-MATCH|battery=true|big=true");
        println!("BENCH-EQ|generated|{}", gen_eq[0]);
        println!("BENCH-EQ|generated|{}", gen_eq[1]);

        // ── Ratios ──
        for workload in ["battery", "big"] {
            let g = times.get(&("generated", workload)).cloned().unwrap_or_default();
            let m = times.get(&("mirror", workload)).cloned().unwrap_or_default();
            assert_eq!(g.len(), OUTER_RUNS, "generated {} runs", workload);
            assert_eq!(m.len(), OUTER_RUNS, "mirror {} runs", workload);
            let g_med = median(g.clone());
            let m_med = median(m.clone());
            let ratio = g_med as f64 / m_med as f64;
            let per_run: Vec<f64> = g
                .iter()
                .zip(m.iter())
                .map(|(a, b)| *a as f64 / *b as f64)
                .collect();
            let lo = per_run.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = per_run.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            println!(
                "BENCH-RESULT|workload={}|generated_median_ns={}|mirror_median_ns={}|ratio={:.4}|per_run_ratio_min={:.4}|per_run_ratio_max={:.4}|generated_runs_ns={:?}|mirror_runs_ns={:?}",
                workload, g_med, m_med, ratio, lo, hi, g, m
            );
        }
        let g_tot = median(times.get(&("generated", "battery")).cloned().unwrap())
            + median(times.get(&("generated", "big")).cloned().unwrap());
        let m_tot = median(times.get(&("mirror", "battery")).cloned().unwrap())
            + median(times.get(&("mirror", "big")).cloned().unwrap());
        println!(
            "BENCH-RESULT|workload=combined(sum of workload medians)|generated_ns={}|mirror_ns={}|ratio={:.4}",
            g_tot,
            m_tot,
            g_tot as f64 / m_tot as f64
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
