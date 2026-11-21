// ─────────────────────────────────────────────────────────────
//  TOKENIZER CROSS-VALIDATION HARNESS (test-only) — Phase 48
//  (M4-S4 part 2: measurement of the re-authored tokenizer)
//
//  Generalizes the M2/M3 cross-validation harnesses
//  (src/m2_cross_validation.rs, src/m3_cross_validation.rs)
//  to the FunctionalTokenizer module
//  (examples/functional_tokenizer.morph, 8 specs, re-authored
//  in Phase 47 and completed in Phase 79). For every spec
//  this harness runs the real pipeline at opt level 1 —
//  parse → typecheck → synthesize_with_module → Z3
//  verify_all (module-aware: specs + type declarations
//  since Phase 76) → select_best — then EXECUTES the
//  winning implementation
//  and compares its behavior against the Phase 47 mirrors
//  (copied below from tests/tokenizer_proof.rs, the source
//  of truth; if the two ever diverge, update this copy).
//  The mirrors follow the SPEC — including the token
//  START-column stamping. Through Phase 78 the
//  hand-written tokenizer diverged there (its push_token
//  stamped token ENDs); D4a made START normative and
//  Phase 79 corrected push_token, so the two artifacts
//  agree now (pinned in tests/tokenizer_proof.rs).
//  Winners are judged against the mirrors, as before.
//
//  EXECUTION ROUTES, per spec family — the two routes the
//  earlier harnesses established, chosen by what the value
//  domain admits:
//   - The three Char classifiers (Char → Bool) execute on
//     the IR interpreter (src/synthesis/eval.rs), the
//     Phase 45 route: the winner body is call-resolved over
//     the winner index (a body may call a sibling spec —
//     is_ident_continue calls is_ident_start) and evaluated
//     per char. Phase 79 note: the Phase 48 framing of this
//     route is retired — since D5a (Phase 76) a Char
//     encodes in Z3 as its Unicode scalar value, the
//     equation family builds the classifiers' seeds, and
//     the crowned seeds are Z3-Verified, so crowning now
//     has the Z3 backstop the is_digit precedent lacked.
//     The harness's classifier standard nonetheless stays
//     what it was: total executed agreement over the full
//     774-char battery (the Phase 45 battery: U+0000..=U+02FF
//     plus Arabic-Indic / Extended Arabic-Indic /
//     superscript / fullwidth digit probes, an emoji, and
//     U+10FFFF) — the D5a cross-check, re-measured every
//     run, never carried. The harness additionally scans
//     each classifier's RAW pool (pre-verification) and
//     reports whether an exact body exists in it at all.
//   - The five record specs (TokenizerState in/out, Option /
//     Result outputs) execute through the M2/M3 compiled
//     runner. Phase 79 note: the original reason for this
//     route is gone — since Phase 75 the interpreter's
//     Value HAS record, Option, and Result forms and the
//     record specs' tests bind in-process (Phase 75's
//     evidence, on hand-written bodies). The compiled
//     route is kept because this harness's object is the
//     pipeline's GENERATED code, executed end-to-end: the
//     winner's generated Rust (plus the generated Rust of
//     every module winner it transitively calls) is
//     compiled with rustc into a runner whose main drives
//     the battery states through the winner and prints one
//     canonical line per input, under the standing 10 s
//     wall-clock limit (a hung winner reads as total
//     disagreement, never hangs the harness — the Phase 16
//     lesson). Each call is panic-isolated (catch_unwind),
//     so a panicking winner yields a per-input PANIC
//     outcome instead of killing the run. A winner whose
//     generated code fails rustc is an execution failure
//     with the rustc excerpt recorded: no COMPILE_EDGE
//     bucket is kept here, because the M3 edge families
//     were rendering defects of template winners whose IR
//     was independently measured correct — here the IR
//     bodies themselves are the object of measurement and
//     none is known correct a priori.
//
//  PINNED vs SPEC-SILENT (the Phase 23 rule): a case is
//  pinned exactly where the spec's constraints decide the
//  output for that input.
//   - Classifiers: the constraint is a total equation —
//     every char is pinned. The 9 in-spec tests per
//     classifier are its golden cases, read off the same
//     single execution (asserted to lie inside the battery).
//   - peek: the two guard rows decide every state
//     (pos < len → Some(source[pos]); pos >= len → None) —
//     the whole battery is pinned.
//   - next_char: pos is pinned on every case and col is
//     pinned on every case (1 on a newline, col+1 else);
//     line is pinned only on newline cases (line+1) and is
//     spec-silent otherwise (the mirror carries it over).
//     Full-state agreement and pinned-field agreement are
//     reported separately.
//   - skip_whitespace: the spec states only two
//     postconditions (output.pos >= state.pos;
//     len(output.source) == len(state.source)). Those are
//     the pinned content; the stopping behavior is the
//     mirror's completion. Postcondition satisfaction and
//     full mirror agreement are reported separately, and a
//     shortfall against the mirror is a property of the
//     spec as stated, not a machinery failure.
//   - tokenize_one / tokenize_all: their constraints are
//     RESTORED (Phase 79; the D1a method calls), and Z3
//     judges bodies against them — tokenize_one's rows
//     refute both partial bodies and tokenize_all's row
//     verifies the Ok(state.tokens) body (Phase 76). The
//     rows pin PARTIAL content, not full outputs:
//     tokenize_one pins the Ok/Err split, exact token-count
//     growth on the Ok side, and end-of-input on the Err
//     side; tokenize_all pins the split plus a length
//     floor. So executed agreement against the mirrors
//     remains these two specs' verdict content (the
//     mirror states the full behavior), now paired with
//     the winners' real Z3 statuses — no longer smoke: a
//     crowned body can be REFUTED by the constraints,
//     which before the restoration it could not.
//
//  VERDICTS (computed per spec across the 3 runs, stated in
//  the report; the Phase 48 verdicts are the historical
//  baseline, the Phase 79 record states the current ones):
//   - EXACT: the winner agrees with the mirror on every
//     battery case in every run (classifiers: 774/774;
//     peek: full battery; next_char: full-state;
//     tokenize_one / tokenize_all: full battery).
//   - PARTIAL: anything less, with the exact region stated
//     (for next_char, pinned-fields-exact is distinguished
//     from pinned-field disagreement; for skip_whitespace,
//     the postcondition / mirror-agreement pair is the
//     verdict content).
//  No M1 checkbox is ticked by this harness — the gate
//  run is Phase 80's. What changed since Phase 48: Z3 CAN
//  now verify Char-keyed and record-shaped bodies
//  (Phases 73–76), so the report pairs executed agreement
//  with the winners' real Z3 statuses, and a shortfall is
//  a synthesis result (no exact body found, a resolver
//  gap), not an encodability wall.
//
//  Run with:
//
//    cargo test -- --ignored --nocapture tokenizer_cross_validation
//
//  The report is written to
//  target/tokenizer-synthesis/cross-validation.md.
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod xval_tok {
    use crate::codegen;
    use crate::spec;
    use crate::spec::ast::{Expr, FunctionSpec, Param};
    use crate::synthesis::engine::{self, CandidateImplementation, IRNode, IRPattern, Provenance};
    use crate::synthesis::eval::{self, Value};
    use crate::synthesis::selector;
    use crate::verify;
    use std::collections::HashMap;
    use std::fmt::Write as _;
    use std::process::Command;
    use std::time::{Duration, Instant};

    // ─────────────────────────────────────────────────────────
    //  Mirrors — copied from tests/tokenizer_proof.rs (source
    //  of truth; that file's golden tests pin these functions
    //  to the specs' stated behavior). Adapted only in the
    //  integer type: the proof uses usize positions, the
    //  generated Rust uses i64 (Morphic Int), so the harness
    //  mirrors use i64 throughout. Token kinds are the spec's
    //  Int codes (0=Ident, 1=IntLit, 2=StringLit, 3=LParen,
    //  4=RParen, 5=LBrace, 6=RBrace, 7=Colon, 8=Semicolon,
    //  9=Comma, 10=Lt, 11=Gt, 12=Eq).
    // ─────────────────────────────────────────────────────────

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

    #[derive(Debug, Clone, PartialEq)]
    struct Token {
        kind: i64,
        lexeme: String,
        line: i64,
        col: i64,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct State {
        source: String,
        pos: i64,
        line: i64,
        col: i64,
        tokens: Vec<Token>,
    }

    fn st(source: &str, pos: i64, line: i64, col: i64) -> State {
        State {
            source: source.to_string(),
            pos,
            line,
            col,
            tokens: vec![],
        }
    }

    fn tok(kind: i64, lexeme: &str, line: i64, col: i64) -> Token {
        Token {
            kind,
            lexeme: lexeme.to_string(),
            line,
            col,
        }
    }

    fn mirror_peek(state: &State) -> Option<char> {
        if state.pos >= state.source.chars().count() as i64 {
            None
        } else {
            state.source.chars().nth(state.pos as usize)
        }
    }

    fn mirror_next_char(state: State) -> State {
        if mirror_peek(&state) == Some('\n') {
            State {
                pos: state.pos + 1,
                line: state.line + 1,
                col: 1,
                ..state
            }
        } else {
            State {
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

    fn mirror_skip_whitespace(state: State) -> State {
        let mut state = state;
        while mirror_peek(&state).map_or(false, mirror_is_whitespace) {
            state = mirror_next_char(state);
        }
        state
    }

    /// Append a token stamped with the START state's line/col
    /// (the spec's tests) to the END state's token list.
    fn mirror_push_token(start: &State, end: State, kind: i64, lexeme: String) -> State {
        let mut tokens = end.tokens.clone();
        tokens.push(Token {
            kind,
            lexeme,
            line: start.line,
            col: start.col,
        });
        State { tokens, ..end }
    }

    fn mirror_single(state: &State, kind: i64, lexeme: &str) -> Result<State, String> {
        let end = mirror_next_char(state.clone());
        Ok(mirror_push_token(state, end, kind, lexeme.to_string()))
    }

    fn mirror_tokenize_one(state: State) -> Result<State, String> {
        let state = mirror_skip_whitespace(state);
        match mirror_peek(&state) {
            None => Ok(state), // EOF
            Some(c) if mirror_is_ident_start(c) => {
                let mut lexeme = String::new();
                let mut s = state.clone();
                while mirror_peek(&s).map_or(false, mirror_is_ident_continue) {
                    if let Some(ch) = mirror_peek(&s) {
                        lexeme.push(ch);
                    }
                    s = mirror_next_char(s);
                }
                Ok(mirror_push_token(&state, s, KIND_IDENT, lexeme))
            }
            Some(':') => mirror_single(&state, KIND_COLON, ":"),
            Some('{') => mirror_single(&state, KIND_LBRACE, "{"),
            Some('}') => mirror_single(&state, KIND_RBRACE, "}"),
            Some('(') => mirror_single(&state, KIND_LPAREN, "("),
            Some(')') => mirror_single(&state, KIND_RPAREN, ")"),
            Some(',') => mirror_single(&state, KIND_COMMA, ","),
            Some('<') => mirror_single(&state, KIND_LT, "<"),
            Some('>') => mirror_single(&state, KIND_GT, ">"),
            Some('=') => mirror_single(&state, KIND_EQ, "="),
            Some(other) => Err(format!(
                "Unexpected character '{}' at line {}",
                other, state.line
            )),
        }
    }

    fn mirror_tokenize_all(state: State) -> Result<Vec<Token>, String> {
        let mut state = state;
        while mirror_peek(&state).is_some() {
            state = mirror_tokenize_one(state)?;
        }
        Ok(state.tokens)
    }

    fn mirror_classifier(name: &str, c: char) -> bool {
        match name {
            "is_whitespace" => mirror_is_whitespace(c),
            "is_ident_start" => mirror_is_ident_start(c),
            "is_ident_continue" => mirror_is_ident_continue(c),
            other => panic!("not a classifier: {}", other),
        }
    }

    // ─────────────────────────────────────────────────────────
    //  Canonical outcome strings — the compiled runner prints
    //  exactly these (its prelude carries show_state /
    //  show_tokens with identical formatting), and the
    //  harness formats mirror outcomes with the same code
    //  shapes, so agreement is string equality.
    // ─────────────────────────────────────────────────────────

    fn show_tokens(ts: &[Token]) -> String {
        ts.iter()
            .map(|t| format!("{},{:?},{},{}", t.kind, t.lexeme, t.line, t.col))
            .collect::<Vec<_>>()
            .join(";")
    }

    fn show_state(s: &State) -> String {
        format!(
            "{}|{}|{}|{}|{:?}|{}",
            s.pos,
            s.line,
            s.col,
            s.source.chars().count(),
            s.source,
            show_tokens(&s.tokens)
        )
    }

    fn show_peek(out: Option<char>) -> String {
        match out {
            None => "NONE".to_string(),
            Some(c) => format!("SOME {}", c as u32),
        }
    }

    fn show_result_state(out: &Result<State, String>) -> String {
        match out {
            Ok(s) => format!("OK|{}", show_state(s)),
            Err(e) => format!("ERR|{:?}", e),
        }
    }

    fn show_result_tokens(out: &Result<Vec<Token>, String>) -> String {
        match out {
            Ok(ts) => format!("OK|{}", show_tokens(ts)),
            Err(e) => format!("ERR|{:?}", e),
        }
    }

    // ─────────────────────────────────────────────────────────
    //  Batteries
    // ─────────────────────────────────────────────────────────

    /// The Phase 45 char battery: U+0000..=U+02FF plus the
    /// six probes its engine test used (Arabic-Indic digit
    /// U+0660, Extended Arabic-Indic U+06F0, superscript two
    /// U+00B2, fullwidth digit U+FF10, an emoji, U+10FFFF).
    /// 768 + 6 = 774 chars.
    fn char_battery() -> Vec<char> {
        let mut battery: Vec<char> = ('\u{0}'..='\u{2FF}').collect();
        battery.extend(['\u{660}', '\u{6F0}', '\u{B2}', '\u{FF10}', '😀', '\u{10FFFF}']);
        battery
    }

    /// peek / next_char states: the Phase 47 cross-check
    /// sources at (line 3, col 7) over every position from 0
    /// to one PAST the end (n+1), plus each source's initial
    /// state (0, 1, 1). 33 + 6 = 39 states.
    fn state_battery() -> Vec<State> {
        let mut out = Vec::new();
        for src in ["", "ab", "a\nb", "é😀x", " \t\r\n", "spec sort"] {
            let n = src.chars().count() as i64;
            for pos in 0..=n + 1 {
                out.push(st(src, pos, 3, 7));
            }
            out.push(st(src, 0, 1, 1));
        }
        out
    }

    /// skip_whitespace states: the Phase 47 battery's eight
    /// sources from (0, 1, 1), plus mid-source starts (inside
    /// and at the edge of whitespace runs).
    fn skip_battery() -> Vec<State> {
        let mut out = Vec::new();
        for src in ["", " ", "  \t\n x", "no-ws", "\n\nab", " \u{A0}x", "a b", "\r\n\t"] {
            out.push(st(src, 0, 1, 1));
        }
        out.push(st("a b", 1, 1, 2));
        out.push(st("  \t\n x", 2, 1, 3));
        out.push(st("\n\nab", 1, 1, 2));
        out.push(st(" \u{A0}x", 1, 1, 2));
        out
    }

    /// tokenize_one states: the Phase 47 goldens plus one
    /// case per arm family (ident, each single-char kind
    /// sampled, the digit error, whitespace-only, leading
    /// newline run).
    fn tokenize_one_battery() -> Vec<State> {
        vec![
            st("spec", 0, 1, 1),
            st("", 0, 1, 1),
            st("  spec", 0, 1, 1),
            st(":", 0, 1, 1),
            st("{", 0, 1, 1),
            st("=", 0, 1, 1),
            st("5", 0, 1, 1),
            st(" ", 0, 1, 1),
            st("a b", 0, 1, 1),
            st("\n\nx", 0, 1, 1),
        ]
    }

    /// tokenize_all sources: the Phase 47 goldens and
    /// cross-checks (incl. the error case and the
    /// punctuation string) plus a newline-separated pair.
    fn tokenize_all_battery() -> Vec<State> {
        ["", "spec sort", "spec{input:()}", "spec 5", "  \n ", "a", "x{y}:(,)", "a\nb"]
            .iter()
            .map(|src| st(src, 0, 1, 1))
            .collect()
    }

    // ─────────────────────────────────────────────────────────
    //  Call resolution over a body index — adapted from the
    //  M3 gate's winner resolver (src/codegen/m3_gate.rs):
    //  calls to indexed specs are replaced by the indexed
    //  body with parameters substituted; capture is refused,
    //  never renamed; depth cap 8 with a visiting set. The
    //  index maps a spec name to (body, params) so the same
    //  code resolves winners, and pool candidates against an
    //  exact-body index, without a CandidateImplementation.
    // ─────────────────────────────────────────────────────────

    type BodyIndex = HashMap<String, (IRNode, Vec<Param>)>;

    const MAX_RESOLVE_DEPTH: usize = 8;

    fn resolve_spec_body(
        name: &str,
        index: &BodyIndex,
        cache: &mut HashMap<String, IRNode>,
        visiting: &[String],
    ) -> Result<IRNode, String> {
        if let Some(body) = cache.get(name) {
            return Ok(body.clone());
        }
        if visiting.iter().any(|v| v == name) {
            return Err(format!("call cycle through {}", name));
        }
        if visiting.len() >= MAX_RESOLVE_DEPTH {
            return Err(format!("call depth cap at {}", name));
        }
        let (body, _) = index
            .get(name)
            .ok_or_else(|| format!("spec {} not in the body index", name))?;
        let body = body.clone();
        let mut visiting = visiting.to_vec();
        visiting.push(name.to_string());
        let resolved = resolve_node(&body, index, cache, &visiting)?;
        cache.insert(name.to_string(), resolved.clone());
        Ok(resolved)
    }

    fn resolve_node(
        node: &IRNode,
        index: &BodyIndex,
        cache: &mut HashMap<String, IRNode>,
        visiting: &[String],
    ) -> Result<IRNode, String> {
        match node {
            IRNode::Call { function, args } => {
                let mut resolved_args = Vec::with_capacity(args.len());
                for a in args {
                    resolved_args.push(resolve_node(a, index, cache, visiting)?);
                }
                if !index.contains_key(function) {
                    // Builtins (len, List) and anything else
                    // the index does not hold pass through
                    // untouched; judging them is the
                    // executor's business.
                    return Ok(IRNode::Call {
                        function: function.clone(),
                        args: resolved_args,
                    });
                }
                let callee_body = resolve_spec_body(function, index, cache, visiting)?;
                let (_, params) = index.get(function).unwrap();
                substitute_params_winner(&callee_body, params, &resolved_args)
            }
            _ => rebuild_children(node, &mut |child| resolve_node(child, index, cache, visiting)),
        }
    }

    /// Rebuild `node` with `f` applied to every child node.
    /// (Copied from src/codegen/m3_gate.rs.)
    fn rebuild_children(
        node: &IRNode,
        f: &mut dyn FnMut(&IRNode) -> Result<IRNode, String>,
    ) -> Result<IRNode, String> {
        use IRNode::*;
        Ok(match node {
            Hole { .. } | Literal(_) | Var(_) => node.clone(),
            Block(stmts) => Block(stmts.iter().map(|s| f(s)).collect::<Result<Vec<_>, _>>()?),
            Let { name, value, body } => Let {
                name: name.clone(),
                value: Box::new(f(value)?),
                body: Box::new(f(body)?),
            },
            If { condition, then_branch, else_branch } => If {
                condition: Box::new(f(condition)?),
                then_branch: Box::new(f(then_branch)?),
                else_branch: Box::new(f(else_branch)?),
            },
            While { condition, body, invariant } => While {
                condition: Box::new(f(condition)?),
                body: Box::new(f(body)?),
                invariant: invariant.clone(),
            },
            For { var, start, end, body } => For {
                var: var.clone(),
                start: Box::new(f(start)?),
                end: Box::new(f(end)?),
                body: Box::new(f(body)?),
            },
            Match { scrutinee, cases } => Match {
                scrutinee: Box::new(f(scrutinee)?),
                cases: cases
                    .iter()
                    .map(|(p, b)| Ok((p.clone(), f(b)?)))
                    .collect::<Result<Vec<_>, String>>()?,
            },
            Call { function, args } => Call {
                function: function.clone(),
                args: args.iter().map(|a| f(a)).collect::<Result<Vec<_>, _>>()?,
            },
            Return(inner) => Return(Box::new(f(inner)?)),
            BinOp(op, l, r) => BinOp(*op, Box::new(f(l)?), Box::new(f(r)?)),
            UnaryOp(op, inner) => UnaryOp(*op, Box::new(f(inner)?)),
            Lambda { params, body } => Lambda {
                params: params.clone(),
                body: Box::new(f(body)?),
            },
            Closure { params, captures, body } => Closure {
                params: params.clone(),
                captures: captures.clone(),
                body: Box::new(f(body)?),
            },
            Collection { kind, elements } => Collection {
                kind: *kind,
                elements: elements.iter().map(|e| f(e)).collect::<Result<Vec<_>, _>>()?,
            },
            Map { function, over } => Map {
                function: Box::new(f(function)?),
                over: Box::new(f(over)?),
            },
            Filter { predicate, over } => Filter {
                predicate: Box::new(f(predicate)?),
                over: Box::new(f(over)?),
            },
            Fold { operation, initial, over } => Fold {
                operation: Box::new(f(operation)?),
                initial: Box::new(f(initial)?),
                over: Box::new(f(over)?),
            },
            Sort { comparator, collection } => Sort {
                comparator: Box::new(f(comparator)?),
                collection: Box::new(f(collection)?),
            },
            Alloc { name, ty, initial } => Alloc {
                name: name.clone(),
                ty: ty.clone(),
                initial: Box::new(f(initial)?),
            },
            Assign { target, value } => Assign {
                target: Box::new(f(target)?),
                value: Box::new(f(value)?),
            },
            Typed { node, ty } => Typed {
                node: Box::new(f(node)?),
                ty: ty.clone(),
            },
            Index(c, i) => Index(Box::new(f(c)?), Box::new(f(i)?)),
            Loop { body } => Loop { body: Box::new(f(body)?) },
            Break { value } => Break {
                value: match value {
                    Some(v) => Some(Box::new(f(v)?)),
                    None => None,
                },
            },
            StructDef { name, fields, body } => StructDef {
                name: name.clone(),
                fields: fields.clone(),
                body: Box::new(f(body)?),
            },
            TryPropagate(inner) => TryPropagate(Box::new(f(inner)?)),
            // M1 features (Phase 73): structural rebuild.
            FieldAccess { base, field } => FieldAccess {
                base: Box::new(f(base)?),
                field: field.clone(),
            },
            RecordLit { type_name, fields } => RecordLit {
                type_name: type_name.clone(),
                fields: fields
                    .iter()
                    .map(|(n, v)| Ok((n.clone(), f(v)?)))
                    .collect::<Result<Vec<_>, String>>()?,
            },
        })
    }

    /// Every name `node` binds anywhere: match pattern
    /// variables, let names, lambda/closure parameters.
    /// (Copied from src/codegen/m3_gate.rs.)
    fn collect_binders(node: &IRNode, out: &mut Vec<String>) {
        match node {
            IRNode::Match { cases, .. } => {
                for (pat, _) in cases {
                    collect_pattern_binders(pat, out);
                }
            }
            IRNode::Let { name, .. } => out.push(name.clone()),
            IRNode::Lambda { params, .. } | IRNode::Closure { params, .. } => {
                for (name, _) in params {
                    out.push(name.clone());
                }
            }
            _ => {}
        }
        let _ = rebuild_children(node, &mut |child| {
            collect_binders(child, out);
            Ok(child.clone())
        });
    }

    fn collect_pattern_binders(pat: &IRPattern, out: &mut Vec<String>) {
        match pat {
            IRPattern::Variable(name) => out.push(name.clone()),
            IRPattern::Constructor(_, subs) => {
                for s in subs {
                    collect_pattern_binders(s, out);
                }
            }
            IRPattern::Guard(inner, _) => collect_pattern_binders(inner, out),
            _ => {}
        }
    }

    /// Every variable name occurring in `node` (an
    /// over-approximation of the free variables —
    /// conservative for a capture screen).
    fn collect_var_names(node: &IRNode, out: &mut Vec<String>) {
        if let IRNode::Var(name) = node {
            out.push(name.clone());
        }
        let _ = rebuild_children(node, &mut |child| {
            collect_var_names(child, out);
            Ok(child.clone())
        });
    }

    /// Substitute argument trees for a callee's parameters
    /// in its (already call-resolved) body. Refused when any
    /// binder name in the body occurs in any argument
    /// (capture), mirroring the engine resolver's discipline.
    /// (Copied from src/codegen/m3_gate.rs.)
    fn substitute_params_winner(
        body: &IRNode,
        params: &[Param],
        args: &[IRNode],
    ) -> Result<IRNode, String> {
        if params.len() != args.len() {
            return Err(format!(
                "arity mismatch: {} params, {} args",
                params.len(),
                args.len()
            ));
        }
        let mut binders = Vec::new();
        collect_binders(body, &mut binders);
        if !binders.is_empty() {
            let mut arg_names = Vec::new();
            for a in args {
                collect_var_names(a, &mut arg_names);
            }
            if binders.iter().any(|b| arg_names.contains(b)) {
                return Err(format!(
                    "substitution refused: a callee binder ({}) occurs in an argument",
                    binders.iter().find(|b| arg_names.contains(b)).unwrap()
                ));
            }
        }
        let map: HashMap<&str, &IRNode> =
            params.iter().map(|p| p.name.as_str()).zip(args.iter()).collect();
        substitute_rec_winner(body, &map, &[])
    }

    fn substitute_rec_winner(
        node: &IRNode,
        map: &HashMap<&str, &IRNode>,
        shadowed: &[String],
    ) -> Result<IRNode, String> {
        match node {
            IRNode::Var(name) => {
                if shadowed.iter().any(|s| s == name) {
                    return Ok(node.clone());
                }
                match map.get(name.as_str()) {
                    Some(arg) => Ok((*arg).clone()),
                    None => Ok(node.clone()),
                }
            }
            IRNode::Match { scrutinee, cases } => {
                let scrutinee = Box::new(substitute_rec_winner(scrutinee, map, shadowed)?);
                let mut out = Vec::with_capacity(cases.len());
                for (pat, body) in cases {
                    let mut pat_binders = Vec::new();
                    collect_pattern_binders(pat, &mut pat_binders);
                    let body = if pat_binders.is_empty() {
                        substitute_rec_winner(body, map, shadowed)?
                    } else {
                        let mut shadowed = shadowed.to_vec();
                        shadowed.extend(pat_binders);
                        substitute_rec_winner(body, map, &shadowed)?
                    };
                    out.push((pat.clone(), body));
                }
                Ok(IRNode::Match {
                    scrutinee,
                    cases: out,
                })
            }
            IRNode::Let { name, value, body } => {
                let value = Box::new(substitute_rec_winner(value, map, shadowed)?);
                let mut shadowed = shadowed.to_vec();
                shadowed.push(name.clone());
                let body = Box::new(substitute_rec_winner(body, map, &shadowed)?);
                Ok(IRNode::Let {
                    name: name.clone(),
                    value,
                    body,
                })
            }
            IRNode::Lambda { params, body } => {
                let mut shadowed = shadowed.to_vec();
                shadowed.extend(params.iter().map(|(n, _)| n.clone()));
                Ok(IRNode::Lambda {
                    params: params.clone(),
                    body: Box::new(substitute_rec_winner(body, map, &shadowed)?),
                })
            }
            IRNode::Closure { params, captures, body } => {
                let mut shadowed = shadowed.to_vec();
                shadowed.extend(params.iter().map(|(n, _)| n.clone()));
                Ok(IRNode::Closure {
                    params: params.clone(),
                    captures: captures.clone(),
                    body: Box::new(substitute_rec_winner(body, map, &shadowed)?),
                })
            }
            _ => rebuild_children(node, &mut |child| substitute_rec_winner(child, map, shadowed)),
        }
    }

    // ─────────────────────────────────────────────────────────
    //  Shared helpers
    // ─────────────────────────────────────────────────────────

    fn provenance_name(p: &Provenance) -> String {
        match p {
            Provenance::LLMGenerated { .. } => "LLMGenerated".to_string(),
            Provenance::Mutation { .. } => "Mutation".to_string(),
            Provenance::Crossover { .. } => "Crossover".to_string(),
            Provenance::TestInduced { .. } => "TestInduced".to_string(),
            Provenance::ConstraintInduced { kind } => format!("ConstraintInduced({})", kind),
            Provenance::Template { .. } => "Template".to_string(),
            Provenance::HumanGuided => "HumanGuided".to_string(),
        }
    }

    fn count_holes(node: &IRNode) -> usize {
        let mut n = if matches!(node, IRNode::Hole { .. }) { 1 } else { 0 };
        let _ = rebuild_children(node, &mut |child| {
            n += count_holes(child);
            Ok(child.clone())
        });
        n
    }

    /// Short structural key of a body (its Debug text,
    /// hashed): the cross-run determinism evidence.
    fn body_key(body: &IRNode) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        format!("{:?}", body).hash(&mut h);
        format!("{:016x}", h.finish())
    }

    /// The module spec names a body calls (module specs
    /// only — builtins like len/List are not in the set).
    fn called_module_specs(body: &IRNode, module_names: &[String]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if let IRNode::Call { function, .. } = body {
            if module_names.iter().any(|n| n == function) && !out.contains(function) {
                out.push(function.clone());
            }
        }
        let _ = rebuild_children(body, &mut |child| {
            for n in called_module_specs(child, module_names) {
                if !out.contains(&n) {
                    out.push(n);
                }
            }
            Ok(child.clone())
        });
        out
    }

    fn load_tokenizer_module() -> (Vec<FunctionSpec>, spec::ast::Spec) {
        let root = env!("CARGO_MANIFEST_DIR");
        let path = format!("{}/examples/functional_tokenizer.morph", root);
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = spec::parser::parse(&source).expect("parse must succeed");
        let checked = spec::typeck::check(parsed).expect("typeck must succeed");
        (checked.spec.functions.clone(), checked.spec)
    }

    // ─────────────────────────────────────────────────────────
    //  Classifier execution (IR interpreter)
    // ─────────────────────────────────────────────────────────

    /// Evaluate a (call-resolved) classifier body on one
    /// char. Err carries the interpreter's reason — an
    /// evaluability fact, counted separately from
    /// disagreement.
    fn eval_classifier_body(body: &IRNode, c: char) -> Result<bool, String> {
        let mut env = eval::Env::new();
        env.insert("c".to_string(), Value::Char(c));
        match eval::eval(body, &env) {
            Ok(Value::Bool(b)) => Ok(b),
            Ok(other) => Err(format!("non-Bool result {:?}", other)),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    /// Resolve a body against an index (fresh cache per
    /// body: the cache is a within-resolution memo only).
    fn resolve_body(body: &IRNode, index: &BodyIndex) -> Result<IRNode, String> {
        let mut cache = HashMap::new();
        resolve_node(body, index, &mut cache, &[])
    }

    #[derive(Default)]
    struct ClassifierMeasure {
        /// Winner agreement over the full battery (pinned:
        /// the classifier equations are total).
        winner_agree: usize,
        /// Battery chars where the winner body could not be
        /// evaluated at all (counted within the non-agreeing
        /// remainder, reported separately).
        winner_eval_errs: usize,
        /// Winner agreement on the spec's in-spec tests.
        golden_agree: usize,
        golden_total: usize,
        /// Raw-pool scan: candidates, candidates evaluable on
        /// every char, candidates exact on every char.
        pool_n: usize,
        pool_evaluable: usize,
        pool_exact: usize,
        /// The first exact pool candidate's RAW body, if any
        /// (the main test installs it in the scan index so a
        /// dependent classifier's pool scan resolves calls to
        /// an exact callee).
        exact_body: Option<IRNode>,
        /// Resolution failure for the winner body, if any.
        winner_note: String,
    }

    fn measure_classifier(
        func: &FunctionSpec,
        winner: &CandidateImplementation,
        pool: &[CandidateImplementation],
        winner_index: &BodyIndex,
        scan_index: &BodyIndex,
        battery: &[char],
    ) -> ClassifierMeasure {
        let name = func.name.as_str();
        let mut m = ClassifierMeasure {
            golden_total: func.tests.len(),
            pool_n: pool.len(),
            ..Default::default()
        };

        // Golden inputs: the in-spec tests (char literal in,
        // Bool literal out), asserted to lie in the battery
        // and to match the mirror (the Phase 47 golden).
        let golden: Vec<char> = func
            .tests
            .iter()
            .map(|t| match (&t.input, &t.expected_output) {
                (Expr::CharLit(c), Expr::BoolLit(b)) => {
                    assert!(battery.contains(c), "{}: golden char {:?} outside battery", name, c);
                    assert_eq!(
                        mirror_classifier(name, *c),
                        *b,
                        "{}: mirror disagrees with in-spec test on {:?}",
                        name,
                        c
                    );
                    *c
                }
                other => panic!("{}: in-spec test is not a char->bool case: {:?}", name, other),
            })
            .collect();

        // The winner, call-resolved over the WINNER index
        // (composition semantics: a call denotes the callee
        // winner, the M3 gate precedent).
        match resolve_body(&winner.body, winner_index) {
            Ok(resolved) => {
                for &c in battery {
                    match eval_classifier_body(&resolved, c) {
                        Ok(b) => {
                            if b == mirror_classifier(name, c) {
                                m.winner_agree += 1;
                                if golden.contains(&c) {
                                    m.golden_agree += 1;
                                }
                            }
                        }
                        Err(_) => m.winner_eval_errs += 1,
                    }
                }
            }
            Err(e) => {
                m.winner_note = format!("winner call resolution failed: {}", e);
            }
        }

        // The raw-pool scan, resolved against the scan index
        // (winner index, with exact callee bodies substituted
        // where an earlier classifier's pool held one — so
        // is_ident_continue's exactness question is about its
        // own candidates, not about the crowned callee).
        for cand in pool {
            let resolved = match resolve_body(&cand.body, scan_index) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let mut evaluable = true;
            let mut exact = true;
            for &c in battery {
                match eval_classifier_body(&resolved, c) {
                    Ok(b) => {
                        if b != mirror_classifier(name, c) {
                            exact = false;
                        }
                    }
                    Err(_) => {
                        evaluable = false;
                        break;
                    }
                }
            }
            if evaluable {
                m.pool_evaluable += 1;
                if exact {
                    m.pool_exact += 1;
                    if m.exact_body.is_none() {
                        m.exact_body = Some(cand.body.clone());
                    }
                }
            }
        }
        m
    }

    // ─────────────────────────────────────────────────────────
    //  Compiled runner for the record specs (the M2/M3
    //  pattern): prelude + winner code(s) + a generated main
    //  that drives the battery and prints one canonical line
    //  per input. Per-input catch_unwind: a panicking winner
    //  prints PANIC for that input and the run continues.
    // ─────────────────────────────────────────────────────────

    const RUNNER_PRELUDE: &str = r#"
#[derive(Debug, Clone, PartialEq)]
pub struct Token { pub kind: i64, pub lexeme: String, pub line: i64, pub col: i64 }
#[derive(Debug, Clone, PartialEq)]
pub struct TokenizerState { pub source: String, pub pos: i64, pub line: i64, pub col: i64, pub tokens: Vec<Token> }
pub type StepResult = Result<TokenizerState, String>;
fn show_tokens(ts: &Vec<Token>) -> String { ts.iter().map(|t| format!("{},{:?},{},{}", t.kind, t.lexeme, t.line, t.col)).collect::<Vec<_>>().join(";") }
fn show_state(s: &TokenizerState) -> String { format!("{}|{}|{}|{}|{:?}|{}", s.pos, s.line, s.col, s.source.chars().count(), s.source, show_tokens(&s.tokens)) }
"#;

    fn state_literal(s: &State) -> String {
        format!(
            "TokenizerState {{ source: {:?}.to_string(), pos: {}, line: {}, col: {}, tokens: vec![] }}",
            s.source, s.pos, s.line, s.col
        )
    }

    /// `codes` are (spec name, generated file text) pairs —
    /// the target winner plus its transitive call closure.
    /// The generated test module is stripped (the runner is
    /// not compiled with --test; the marker is codegen's
    /// "\n#[cfg(test)]\n" at src/codegen/mod.rs). The prelude
    /// also carries the spec's `StepResult` alias: generate
    /// renders named types verbatim, so a winner's signature
    /// may reference the module's alias — the alias is part
    /// of the spec's declared type environment, not of any
    /// winner body.
    fn build_runner(spec_name: &str, codes: &[(String, String)], inputs: &[State]) -> String {
        let mut out = String::new();
        out.push_str(RUNNER_PRELUDE);
        for (_, code) in codes {
            let stripped = code.split("#[cfg(test)]").next().unwrap_or("");
            out.push_str(stripped);
            out.push('\n');
        }
        let vals: Vec<String> = inputs.iter().map(state_literal).collect();
        let call_block = match spec_name {
            "peek" => r#"match r {
            Ok(None) => println!("NONE"),
            Ok(Some(c)) => println!("SOME {}", c as u32),
            Err(_) => println!("PANIC"),
        }"#,
            "next_char" | "skip_whitespace" => r#"match r {
            Ok(out) => println!("{}", show_state(&out)),
            Err(_) => println!("PANIC"),
        }"#,
            "tokenize_one" => r#"match r {
            Ok(Ok(st)) => println!("OK|{}", show_state(&st)),
            Ok(Err(e)) => println!("ERR|{:?}", e),
            Err(_) => println!("PANIC"),
        }"#,
            "tokenize_all" => r#"match r {
            Ok(Ok(ts)) => println!("OK|{}", show_tokens(&ts)),
            Ok(Err(e)) => println!("ERR|{:?}", e),
            Err(_) => println!("PANIC"),
        }"#,
            other => panic!("no runner shape for {}", other),
        };
        let _ = write!(
            out,
            "\nfn main() {{\n    std::panic::set_hook(Box::new(|_| {{}}));\n    let __inputs: Vec<TokenizerState> = vec![{}];\n    for s in &__inputs {{\n        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {}(s.clone())));\n        {}\n    }}\n}}\n",
            vals.join(", "),
            spec_name,
            call_block
        );
        out
    }

    /// Per-runner wall-clock limit, inherited from the M2/M3
    /// harnesses (Phase 16 lesson): winners can be
    /// non-terminating, and a hung winner must read as total
    /// disagreement, never hang the harness.
    const RUNNER_TIMEOUT_SECS: u64 = 10;

    fn execute_runner(
        run_idx: usize,
        spec_name: &str,
        codes: &[(String, String)],
        inputs: &[State],
    ) -> Result<Vec<String>, String> {
        let dir = std::env::temp_dir().join(format!(
            "morphic_xval_tok_{}_{}_{}",
            std::process::id(),
            run_idx,
            spec_name
        ));
        std::fs::create_dir_all(&dir).map_err(|e| format!("temp dir: {}", e))?;
        let src_path = dir.join("main.rs");
        std::fs::write(&src_path, build_runner(spec_name, codes, inputs))
            .map_err(|e| format!("write runner: {}", e))?;
        let bin_path = dir.join("runner");
        let compile = Command::new("rustc")
            .arg("--edition")
            .arg("2021")
            .arg(&src_path)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .map_err(|e| format!("spawn rustc: {}", e))?;
        if !compile.status.success() {
            let stderr = String::from_utf8_lossy(&compile.stderr);
            let errors: Vec<&str> = stderr
                .lines()
                .filter(|l| l.starts_with("error"))
                .take(3)
                .collect();
            let excerpt: Vec<&str> = if errors.is_empty() {
                stderr.lines().take(4).collect()
            } else {
                errors
            };
            return Err(format!("rustc failed: {}", excerpt.join(" | ")));
        }
        let stdout_path = dir.join("stdout.txt");
        let stdout_file =
            std::fs::File::create(&stdout_path).map_err(|e| format!("stdout file: {}", e))?;
        let mut child = Command::new(&bin_path)
            .stdout(std::process::Stdio::from(stdout_file))
            .spawn()
            .map_err(|e| format!("spawn runner: {}", e))?;
        let deadline = Instant::now() + Duration::from_secs(RUNNER_TIMEOUT_SECS);
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        let _ = std::fs::remove_dir_all(&dir);
                        return Err(format!(
                            "runner timed out after {}s (non-terminating winner)",
                            RUNNER_TIMEOUT_SECS
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => return Err(format!("wait runner: {}", e)),
            }
        };
        if !status.success() {
            return Err(format!("runner exited {}", status));
        }
        let stdout =
            std::fs::read_to_string(&stdout_path).map_err(|_| "runner output is not UTF-8")?;
        let lines: Vec<String> = stdout.lines().map(|l| l.to_string()).collect();
        if lines.len() != inputs.len() {
            return Err(format!(
                "expected {} output lines, got {}",
                inputs.len(),
                lines.len()
            ));
        }
        let _ = std::fs::remove_dir_all(&dir);
        Ok(lines)
    }

    /// The transitive closure of module specs a winner calls
    /// (itself first), over the winner index.
    fn call_closure(
        target: &str,
        winners: &HashMap<String, CandidateImplementation>,
        module_names: &[String],
    ) -> Vec<String> {
        let mut order: Vec<String> = Vec::new();
        let mut queue = vec![target.to_string()];
        while let Some(n) = queue.pop() {
            if order.contains(&n) {
                continue;
            }
            order.push(n.clone());
            if let Some(w) = winners.get(&n) {
                for c in called_module_specs(&w.body, module_names) {
                    queue.push(c);
                }
            }
        }
        order
    }

    // ─────────────────────────────────────────────────────────
    //  Comparison helpers (canonical-line equality against
    //  mirror-computed expectations)
    // ─────────────────────────────────────────────────────────

    fn disagreement(input: &State, winner: &str, mirror: &str) -> String {
        format!(
            "input {}: winner {}, mirror {}",
            show_state(input),
            winner,
            mirror
        )
    }

    fn agreement_peek(lines: &[String], inputs: &[State]) -> (usize, Vec<String>) {
        let mut agree = 0;
        let mut dis = Vec::new();
        for (line, input) in lines.iter().zip(inputs) {
            let expected = show_peek(mirror_peek(input));
            if *line == expected {
                agree += 1;
            } else if dis.len() < 5 {
                dis.push(disagreement(input, line, &expected));
            }
        }
        (agree, dis)
    }

    /// (full-state agree, pinned-field agree, disagreements).
    /// Pinned fields for next_char: pos (always), col
    /// (always), line (only where peek is a newline).
    fn agreement_next_char(lines: &[String], inputs: &[State]) -> (usize, usize, Vec<String>) {
        let mut full = 0;
        let mut pinned = 0;
        let mut dis = Vec::new();
        for (line, input) in lines.iter().zip(inputs) {
            let expected = mirror_next_char(input.clone());
            if *line == show_state(&expected) {
                full += 1;
            }
            let fields: Vec<&str> = line.split('|').collect();
            let parsed = if fields.len() == 6 {
                match (
                    fields[0].parse::<i64>(),
                    fields[1].parse::<i64>(),
                    fields[2].parse::<i64>(),
                ) {
                    (Ok(p), Ok(l), Ok(c)) => Some((p, l, c)),
                    _ => None,
                }
            } else {
                None
            };
            let pinned_ok = match parsed {
                Some((p, l, c)) => {
                    p == expected.pos
                        && c == expected.col
                        && (mirror_peek(input) != Some('\n') || l == expected.line)
                }
                None => false,
            };
            if pinned_ok {
                pinned += 1;
            } else if dis.len() < 5 {
                dis.push(disagreement(input, line, &show_state(&expected)));
            }
        }
        (full, pinned, dis)
    }

    /// (full mirror agree, stated-postcondition agree,
    /// disagreements). The spec's two stated postconditions:
    /// output.pos >= state.pos and
    /// len(output.source) == len(state.source) — fields 0 and
    /// 3 of the canonical line.
    fn agreement_skip(lines: &[String], inputs: &[State]) -> (usize, usize, Vec<String>) {
        let mut full = 0;
        let mut post = 0;
        let mut dis = Vec::new();
        for (line, input) in lines.iter().zip(inputs) {
            let expected = mirror_skip_whitespace(input.clone());
            if *line == show_state(&expected) {
                full += 1;
            }
            let fields: Vec<&str> = line.split('|').collect();
            let post_ok = if fields.len() == 6 {
                match (fields[0].parse::<i64>(), fields[3].parse::<i64>()) {
                    (Ok(p), Ok(n)) => {
                        p >= input.pos && n == input.source.chars().count() as i64
                    }
                    _ => false,
                }
            } else {
                false
            };
            if post_ok {
                post += 1;
            } else if dis.len() < 5 {
                dis.push(disagreement(input, line, &show_state(&expected)));
            }
        }
        (full, post, dis)
    }

    fn agreement_lines(
        lines: &[String],
        inputs: &[State],
        expected: &[String],
    ) -> (usize, Vec<String>) {
        let mut agree = 0;
        let mut dis = Vec::new();
        for ((line, input), exp) in lines.iter().zip(inputs).zip(expected) {
            if line == exp {
                agree += 1;
            } else if dis.len() < 5 {
                dis.push(disagreement(input, line, exp));
            }
        }
        (agree, dis)
    }

    // ─────────────────────────────────────────────────────────
    //  The harness (ignored; run explicitly)
    // ─────────────────────────────────────────────────────────

    #[derive(Default)]
    struct RecordMeasure {
        agree: usize,
        total: usize,
        /// next_char: pinned-field agreement; skip_whitespace:
        /// stated-postcondition agreement. None elsewhere.
        secondary: Option<usize>,
        /// "ok" or the execution failure, verbatim.
        exec: String,
        disagreements: Vec<String>,
    }

    fn measure_record_spec(
        run: usize,
        name: &str,
        winners: &HashMap<String, CandidateImplementation>,
        module_names: &[String],
    ) -> RecordMeasure {
        let inputs: Vec<State> = match name {
            "peek" | "next_char" => state_battery(),
            "skip_whitespace" => skip_battery(),
            "tokenize_one" => tokenize_one_battery(),
            "tokenize_all" => tokenize_all_battery(),
            other => panic!("not a record spec: {}", other),
        };
        let mut m = RecordMeasure {
            total: inputs.len(),
            exec: "ok".to_string(),
            ..Default::default()
        };
        let closure = call_closure(name, winners, module_names);
        let mut codes: Vec<(String, String)> = Vec::new();
        for member in &closure {
            match winners.get(member) {
                Some(w) => match codegen::generate(w, "rust") {
                    Ok(code) => codes.push((member.clone(), code)),
                    Err(e) => {
                        m.exec = format!("FAIL(codegen error for {}: {})", member, e);
                        return m;
                    }
                },
                None => {
                    m.exec = format!("FAIL(no winner for closure member {})", member);
                    return m;
                }
            }
        }
        let lines = match execute_runner(run, name, &codes, &inputs) {
            Ok(l) => l,
            Err(e) => {
                m.exec = format!("FAIL({})", e);
                return m;
            }
        };
        match name {
            "peek" => {
                let (agree, dis) = agreement_peek(&lines, &inputs);
                m.agree = agree;
                m.disagreements = dis;
            }
            "next_char" => {
                let (full, pinned, dis) = agreement_next_char(&lines, &inputs);
                m.agree = full;
                m.secondary = Some(pinned);
                m.disagreements = dis;
            }
            "skip_whitespace" => {
                let (full, post, dis) = agreement_skip(&lines, &inputs);
                m.agree = full;
                m.secondary = Some(post);
                m.disagreements = dis;
            }
            "tokenize_one" => {
                let expected: Vec<String> = inputs
                    .iter()
                    .map(|s| show_result_state(&mirror_tokenize_one(s.clone())))
                    .collect();
                let (agree, dis) = agreement_lines(&lines, &inputs, &expected);
                m.agree = agree;
                m.disagreements = dis;
            }
            "tokenize_all" => {
                let expected: Vec<String> = inputs
                    .iter()
                    .map(|s| show_result_tokens(&mirror_tokenize_all(s.clone())))
                    .collect();
                let (agree, dis) = agreement_lines(&lines, &inputs, &expected);
                m.agree = agree;
                m.disagreements = dis;
            }
            _ => unreachable!(),
        }
        m
    }

    #[test]
    #[ignore = "cross-validation harness: 3 randomized synthesis runs x 8 specs, executes winners against the Phase 47 mirrors; run explicitly with --ignored --nocapture"]
    fn tokenizer_cross_validation() {
        let root = env!("CARGO_MANIFEST_DIR");
        let out_dir = format!("{}/target/tokenizer-synthesis", root);
        std::fs::create_dir_all(&out_dir).unwrap();
        let (functions, module) = load_tokenizer_module();
        assert_eq!(functions.len(), 8, "tokenizer module spec count");
        let module_names: Vec<String> = functions.iter().map(|f| f.name.clone()).collect();
        let battery = char_battery();
        assert_eq!(battery.len(), 774, "Phase 45 char battery size");

        let mut md = String::new();
        let _ = writeln!(md, "# Tokenizer cross-validation (Phase 48; classifiers re-measured in Phase 49 with the char_equation seed family; re-instrumented in Phase 79 for the completed module — post-D5a verdicts, D4a agreement, restored tokenize constraints): synthesized winners vs tests/tokenizer_proof.rs mirrors (S4 copies in src/tokenizer_cross_validation.rs)");
        let _ = writeln!(md);
        let _ = writeln!(md, "Pipeline per spec per run: parse → typecheck → synthesize_with_module (opt 1) → Z3 verify_all (module-aware: specs + type declarations) → select_best. Classifiers execute on the IR interpreter over the 774-char Phase 45 battery, judged by executed mirror agreement (the standing D5a cross-check, re-measured every run); their winners' Z3 statuses are real since Phase 76 and appear in the table. Record specs execute through compiled runners (10 s limit, per-input panic isolation) over the Phase 47 batteries. Pinned vs spec-silent per the header: peek fully pinned; next_char pinned on pos+col always and line only on newlines; skip_whitespace pinned only on its two stated postconditions; tokenize_one/tokenize_all carry their restored constraints — the rows pin partial content (the Ok/Err split + growth/length rows), so executed agreement against the mirrors is their verdict content, paired with the winners' Z3 statuses.");
        let _ = writeln!(md);
        let _ = writeln!(md, "| Run | Spec | Winner tpr | Z3 status | Winner prov | Seed in pool | Agreement | Exec |");
        let _ = writeln!(md, "|---|---|---|---|---|---|---|---|");

        let mut details = String::new();
        let _ = writeln!(details, "\n## Details (pool scans, disagreements, determinism)\n");

        // Cross-run accumulators for the verdict table.
        let mut clf_runs: HashMap<String, Vec<ClassifierMeasure>> = HashMap::new();
        let mut rec_runs: HashMap<String, Vec<RecordMeasure>> = HashMap::new();
        let mut winner_keys: HashMap<String, Vec<String>> = HashMap::new();
        let mut winner_facts: HashMap<String, Vec<String>> = HashMap::new();

        for run in 1..=3usize {
            // ── The real pipeline, per spec ──
            let mut pools: HashMap<String, Vec<CandidateImplementation>> = HashMap::new();
            let mut winners: HashMap<String, CandidateImplementation> = HashMap::new();
            let mut failed: Vec<String> = Vec::new();
            for func in &functions {
                let name = func.name.clone();
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    engine::synthesize_with_module(func, 1, &functions)
                }));
                let candidates = match outcome {
                    Ok(Ok(c)) if !c.is_empty() => c,
                    Ok(Ok(_)) => {
                        failed.push(format!("{}: synthesis returned no candidates", name));
                        continue;
                    }
                    Ok(Err(e)) => {
                        failed.push(format!("{}: synthesis error: {}", name, e));
                        continue;
                    }
                    Err(_) => {
                        failed.push(format!("{}: synthesis panicked", name));
                        continue;
                    }
                };
                pools.insert(name.clone(), candidates.clone());
                let verified = match verify::verifier::Verifier::with_module(&module)
                    .verify_all(candidates, func)
                {
                    Ok(v) => v,
                    Err(e) => {
                        failed.push(format!("{}: verify_all error: {}", name, e));
                        continue;
                    }
                };
                match selector::select_best(verified, func) {
                    Ok(w) => {
                        winners.insert(name, w);
                    }
                    Err(e) => {
                        failed.push(format!("{}: select_best error: {}", name, e));
                    }
                }
            }
            for f in &failed {
                println!("TOKX|run={}|FAIL|{}", run, f);
                let _ = writeln!(details, "- run {} pipeline failure: {}", run, f);
            }

            // Winner index for call resolution.
            let mut winner_index: BodyIndex = HashMap::new();
            for func in &functions {
                if let Some(w) = winners.get(&func.name) {
                    winner_index.insert(func.name.clone(), (w.body.clone(), func.params.clone()));
                    winner_keys
                        .entry(func.name.clone())
                        .or_default()
                        .push(body_key(&w.body));
                    winner_facts.entry(func.name.clone()).or_default().push(format!(
                        "prov={} tpr={:.3} status={:?} verified={} holes={}",
                        provenance_name(&w.provenance),
                        w.scores.test_pass_ratio,
                        w.verification_status,
                        w.verified,
                        count_holes(&w.body)
                    ));
                }
            }

            // ── Classifiers, in dependency order (the scan
            // index accumulates exact bodies for callees) ──
            let mut scan_index = winner_index.clone();
            for name in ["is_whitespace", "is_ident_start", "is_ident_continue"] {
                let func = functions.iter().find(|f| f.name == name).unwrap();
                let (winner, pool) = match (winners.get(name), pools.get(name)) {
                    (Some(w), Some(p)) => (w, p),
                    _ => continue,
                };
                let m = measure_classifier(func, winner, pool, &winner_index, &scan_index, &battery);
                if let Some(exact_body) = &m.exact_body {
                    scan_index.insert(name.to_string(), (exact_body.clone(), func.params.clone()));
                }
                let seed_in_pool = pool
                    .iter()
                    .any(|c| matches!(c.provenance, Provenance::ConstraintInduced { .. }));
                println!(
                    "TOKX|run={}|spec={}|winner_agree={}/{}|golden={}/{}|eval_errs={}|pool={}|pool_evaluable={}|pool_exact={}|seed_in_pool={}|{}",
                    run, name, m.winner_agree, battery.len(), m.golden_agree, m.golden_total,
                    m.winner_eval_errs, m.pool_n, m.pool_evaluable, m.pool_exact, seed_in_pool,
                    winner_facts[name].last().cloned().unwrap_or_default()
                );
                let _ = writeln!(
                    md,
                    "| {} | {} | {:.3} | {:?} | {} | {} | {}/{} (golden {}/{}) | interp |",
                    run,
                    name,
                    winner.scores.test_pass_ratio,
                    winner.verification_status,
                    provenance_name(&winner.provenance),
                    seed_in_pool,
                    m.winner_agree,
                    battery.len(),
                    m.golden_agree,
                    m.golden_total
                );
                if !m.winner_note.is_empty() {
                    let _ = writeln!(details, "- run {} {}: {}", run, name, m.winner_note);
                }
                clf_runs.entry(name.to_string()).or_default().push(m);
            }

            // ── Record specs + the tokenize pair ──
            for name in ["peek", "next_char", "skip_whitespace", "tokenize_one", "tokenize_all"] {
                if !winners.contains_key(name) {
                    continue;
                }
                let winner = &winners[name];
                let m = measure_record_spec(run, name, &winners, &module_names);
                let seed_in_pool = pools
                    .get(name)
                    .map(|p| {
                        p.iter()
                            .any(|c| matches!(c.provenance, Provenance::ConstraintInduced { .. }))
                    })
                    .unwrap_or(false);
                let secondary = match m.secondary {
                    Some(s) => format!("|secondary={}/{}", s, m.total),
                    None => String::new(),
                };
                println!(
                    "TOKX|run={}|spec={}|agree={}/{}{}|seed_in_pool={}|exec={}|{}",
                    run,
                    name,
                    m.agree,
                    m.total,
                    secondary,
                    seed_in_pool,
                    m.exec,
                    winner_facts[name].last().cloned().unwrap_or_default()
                );
                let _ = writeln!(
                    md,
                    "| {} | {} | {:.3} | {:?} | {} | {} | {}/{}{} | {} |",
                    run,
                    name,
                    winner.scores.test_pass_ratio,
                    winner.verification_status,
                    provenance_name(&winner.provenance),
                    seed_in_pool,
                    m.agree,
                    m.total,
                    match m.secondary {
                        Some(s) => format!(" (secondary {}/{})", s, m.total),
                        None => String::new(),
                    },
                    m.exec
                );
                for d in &m.disagreements {
                    let _ = writeln!(details, "- run {} {} disagreement: {}", run, name, d);
                }
                rec_runs.entry(name.to_string()).or_default().push(m);
            }
        }

        // ── Verdicts ──
        let _ = writeln!(details, "\n## Verdicts (per the rules in the harness header)\n");
        let nb = battery.len();
        for name in ["is_whitespace", "is_ident_start", "is_ident_continue"] {
            let runs = &clf_runs[name];
            let agrees: Vec<String> = runs.iter().map(|m| m.winner_agree.to_string()).collect();
            let exacts: Vec<String> = runs.iter().map(|m| m.pool_exact.to_string()).collect();
            let verdict = if runs.iter().all(|m| m.winner_agree == nb) {
                "EXACT"
            } else {
                "PARTIAL"
            };
            let keys = &winner_keys[name];
            let deterministic = keys.len() == 3 && keys.iter().all(|k| k == &keys[0]);
            let line = format!(
                "- {}: {} — winner battery agreement per run [{}/{}], pool-exact bodies per run [{}], winner deterministic across runs: {}",
                name,
                verdict,
                agrees.join(", "),
                nb,
                exacts.join(", "),
                deterministic
            );
            println!("TOKX-VERDICT|{}", line);
            let _ = writeln!(details, "{}", line);
        }
        for name in ["peek", "next_char", "skip_whitespace", "tokenize_one", "tokenize_all"] {
            let runs = &rec_runs[name];
            let total = runs.first().map(|m| m.total).unwrap_or(0);
            let agrees: Vec<String> = runs.iter().map(|m| m.agree.to_string()).collect();
            let keys = &winner_keys[name];
            let deterministic = keys.len() == 3 && keys.iter().all(|k| k == &keys[0]);
            let verdict = match name {
                "peek" => {
                    if runs.iter().all(|m| m.agree == total) {
                        "EXACT".to_string()
                    } else {
                        "PARTIAL".to_string()
                    }
                }
                "next_char" => {
                    if runs.iter().all(|m| m.agree == total) {
                        "EXACT".to_string()
                    } else if runs
                        .iter()
                        .all(|m| m.secondary == Some(total))
                    {
                        "PARTIAL (pinned fields exact in every run; divergence confined to the spec-silent line field)".to_string()
                    } else {
                        "PARTIAL".to_string()
                    }
                }
                "skip_whitespace" => {
                    let posts: Vec<String> = runs
                        .iter()
                        .map(|m| m.secondary.unwrap_or(0).to_string())
                        .collect();
                    if runs.iter().all(|m| m.agree == total) {
                        format!(
                            "EXACT on this battery (postconditions [{}/{}]) — the spec still under-determines the behavior off-battery",
                            posts.join(", "),
                            total
                        )
                    } else {
                        format!(
                            "PARTIAL by design — stated postconditions [{}/{}], full mirror agreement [{}/{}]; the shortfall against the mirror is a property of the spec as stated (only two postconditions), not a machinery failure",
                            posts.join(", "),
                            total,
                            agrees.join(", "),
                            total
                        )
                    }
                }
                _ => {
                    // tokenize_one / tokenize_all (Phase 79):
                    // no longer smoke-only — the constraints
                    // are restored and Z3 judges the winners
                    // (their statuses are in the table above);
                    // the verdict content is executed
                    // agreement, EXACT/PARTIAL as peek's.
                    if runs.iter().all(|m| m.agree == total) {
                        format!(
                            "EXACT — winner agreed with the mirror on every battery case in every run (agreement [{}/{}])",
                            agrees.join(", "),
                            total
                        )
                    } else {
                        format!(
                            "PARTIAL — executed agreement [{}/{}]; the restored constraints pin partial content (the Ok/Err split + growth/length rows), not full outputs, so the shortfall is measured against the mirror's completion",
                            agrees.join(", "),
                            total
                        )
                    }
                }
            };
            let line = format!(
                "- {}: {} — winner deterministic across runs: {}",
                name, verdict, deterministic
            );
            println!("TOKX-VERDICT|{}", line);
            let _ = writeln!(details, "{}", line);
        }

        md.push_str(&details);
        let report_path = format!("{}/cross-validation.md", out_dir);
        std::fs::write(&report_path, &md).unwrap();
        println!("TOKX-REPORT|written to {}", report_path);
        print!("{}", md);
    }

    // ─────────────────────────────────────────────────────────
    //  Mechanics tests (not ignored): the harness must
    //  provably measure what it claims — mirrors match the
    //  Phase 47 golden, batteries have the recorded sizes,
    //  canonical formats are stable, the resolver inlines,
    //  and the compiled runner detects total agreement,
    //  partial agreement, and panics.
    // ─────────────────────────────────────────────────────────

    #[test]
    fn tok_mirrors_match_phase47_golden() {
        // peek (the two moved tests)
        assert_eq!(mirror_peek(&st("hi", 0, 1, 1)), Some('h'));
        assert_eq!(mirror_peek(&st("", 0, 1, 1)), None);
        // next_char ("normal" + the newline rows as a case)
        assert_eq!(mirror_next_char(st("ab", 0, 1, 1)), st("ab", 1, 1, 2));
        assert_eq!(mirror_next_char(st("a\nb", 1, 1, 2)), st("a\nb", 2, 2, 1));
        // The nine carried classifier tests
        assert!(mirror_is_whitespace(' '));
        assert!(!mirror_is_whitespace('a'));
        assert!(mirror_is_whitespace('\n'));
        assert!(mirror_is_ident_start('a'));
        assert!(!mirror_is_ident_start('5'));
        assert!(mirror_is_ident_start('_'));
        assert!(mirror_is_ident_continue('x'));
        assert!(mirror_is_ident_continue('7'));
        assert!(!mirror_is_ident_continue('{'));
        // skip_whitespace (two moved tests + the M0 proof's case)
        assert_eq!(mirror_skip_whitespace(st("hi", 0, 1, 1)), st("hi", 0, 1, 1));
        assert_eq!(mirror_skip_whitespace(st("  hi", 0, 1, 1)), st("  hi", 2, 1, 3));
        assert_eq!(
            mirror_skip_whitespace(st("   hello", 0, 1, 1)),
            st("   hello", 3, 1, 4)
        );
        // tokenize_one ("keyword-spec": token stamped at its
        // START, col 1 — D4a-normative on both sides since
        // Phase 79; "end-of-input")
        let out = mirror_tokenize_one(st("spec", 0, 1, 1)).unwrap();
        assert_eq!(
            out,
            State {
                source: "spec".to_string(),
                pos: 4,
                line: 1,
                col: 5,
                tokens: vec![tok(KIND_IDENT, "spec", 1, 1)],
            }
        );
        assert_eq!(mirror_tokenize_one(st("", 0, 1, 1)).unwrap(), st("", 0, 1, 1));
        // tokenize_all ("simple-spec": START cols 1 and 6;
        // "empty")
        assert_eq!(
            mirror_tokenize_all(st("spec sort", 0, 1, 1)).unwrap(),
            vec![tok(KIND_IDENT, "spec", 1, 1), tok(KIND_IDENT, "sort", 1, 6)]
        );
        assert_eq!(
            mirror_tokenize_all(st("", 0, 1, 1)).unwrap(),
            Vec::<Token>::new()
        );
        // The digit error message, byte-identical to the
        // hand-written tokenizer's.
        assert_eq!(
            mirror_tokenize_all(st("spec 5", 0, 1, 1)),
            Err("Unexpected character '5' at line 1".to_string())
        );
    }

    #[test]
    fn tok_batteries_have_the_recorded_sizes() {
        assert_eq!(char_battery().len(), 774);
        assert_eq!(state_battery().len(), 39);
        assert_eq!(skip_battery().len(), 12);
        assert_eq!(tokenize_one_battery().len(), 10);
        assert_eq!(tokenize_all_battery().len(), 8);
        // Every classifier's golden chars lie in the battery.
        let battery = char_battery();
        for c in [' ', 'a', '\n', '5', '_', 'x', '7', '{'] {
            assert!(battery.contains(&c), "golden char {:?} missing", c);
        }
    }

    #[test]
    fn tok_canonical_formats() {
        assert_eq!(show_state(&st("hi", 0, 1, 1)), "0|1|1|2|\"hi\"|");
        assert_eq!(show_peek(None), "NONE");
        assert_eq!(show_peek(Some('a')), "SOME 97");
        assert_eq!(show_peek(Some('\n')), "SOME 10");
        assert_eq!(
            show_result_state(&Err("x".to_string())),
            "ERR|\"x\""
        );
        assert_eq!(
            show_result_tokens(&Ok(vec![tok(KIND_IDENT, "spec", 1, 1)])),
            "OK|0,\"spec\",1,1"
        );
    }

    #[test]
    fn tok_resolver_inlines_classifier_calls() {
        use crate::spec::ast::TypeRef;
        use engine::{IRBinOp, IRLiteral};
        let c = || IRNode::Var("c".to_string());
        let ch = |x: char| IRNode::Literal(IRLiteral::Char(x));
        let bin = |op: IRBinOp, l: IRNode, r: IRNode| {
            IRNode::BinOp(op, Box::new(l), Box::new(r))
        };
        // is_ident_start, restricted to lowercase + '_' (the
        // probes below stay inside what this body decides
        // exactly as the full mirror does).
        let start_body = bin(
            IRBinOp::Or,
            bin(
                IRBinOp::And,
                bin(IRBinOp::Ge, c(), ch('a')),
                bin(IRBinOp::Le, c(), ch('z')),
            ),
            bin(IRBinOp::Eq, c(), ch('_')),
        );
        let cont_body = bin(
            IRBinOp::Or,
            IRNode::Call {
                function: "is_ident_start".to_string(),
                args: vec![c()],
            },
            bin(
                IRBinOp::And,
                bin(IRBinOp::Ge, c(), ch('0')),
                bin(IRBinOp::Le, c(), ch('9')),
            ),
        );
        let mut index: BodyIndex = HashMap::new();
        index.insert(
            "is_ident_start".to_string(),
            (
                start_body,
                vec![Param {
                    name: "c".to_string(),
                    ty: TypeRef::Char,
                    annotations: HashMap::new(),
                }],
            ),
        );
        let resolved = resolve_body(&cont_body, &index).expect("resolution must succeed");
        // No Call to an indexed spec survives resolution.
        assert!(called_module_specs(&resolved, &["is_ident_start".to_string()]).is_empty());
        assert_eq!(eval_classifier_body(&resolved, 'b'), Ok(true));
        assert_eq!(eval_classifier_body(&resolved, '5'), Ok(true));
        assert_eq!(eval_classifier_body(&resolved, '_'), Ok(true));
        assert_eq!(eval_classifier_body(&resolved, '{'), Ok(false));
    }

    #[test]
    fn tok_runner_executes_and_detects() {
        let inputs = state_battery();
        let run_with = |code: &str| {
            let codes = vec![("peek".to_string(), code.to_string())];
            execute_runner(0, "peek", &codes, &inputs).expect("runner must execute")
        };
        // A correct peek: total agreement (39/39).
        let correct = "pub fn peek(state: TokenizerState) -> Option<char> { state.source.chars().nth(state.pos as usize) }";
        let lines = run_with(correct);
        let (agree, dis) = agreement_peek(&lines, &inputs);
        assert_eq!(agree, inputs.len(), "disagreements: {:?}", dis);
        // A constant-None peek: agrees exactly on the 13
        // battery states whose mirror output is None (two
        // past/at-end positions per source + the empty
        // source's initial state).
        let none = "pub fn peek(state: TokenizerState) -> Option<char> { None }";
        let lines = run_with(none);
        let (agree, _) = agreement_peek(&lines, &inputs);
        assert_eq!(agree, 13);
        // A panicking peek: per-input PANIC, zero agreement,
        // and the runner itself still completes.
        let panic = "pub fn peek(state: TokenizerState) -> Option<char> { panic!(\"boom\") }";
        let lines = run_with(panic);
        assert!(lines.iter().all(|l| l == "PANIC"));
        let (agree, _) = agreement_peek(&lines, &inputs);
        assert_eq!(agree, 0);
    }
}
