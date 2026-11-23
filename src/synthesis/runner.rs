// ─────────────────────────────────────────────────────────────
//  COMPILED-RUNNER FINAL SCORING (Phase 58, campaign slice G1)
//
//  The problem (Phases 53/57): for specs whose in-spec tests
//  do not all bind in the IR interpreter — today exactly the
//  lambda-carrying tuple residue (sort, binary_search: a
//  lambda argument for a fn-typed parameter; the evaluator's
//  `Value` has no function variant) — `evaluate_candidate`
//  scores every candidate by the FALLBACK test_pass_ratio 1.0,
//  so crowns are decided by tie-breakers and compilability is
//  never priced: Phase 57 measured binary_search crowns that
//  do not compile beating pool members that do. This stage
//  gives finalists a REAL score from a compiled runner: the
//  candidate is rendered through the production Rust codegen,
//  a runner main is generated from the spec's tests (the
//  generated test module's own invocations, re-read from
//  generate's output and annotated with a concrete element
//  type — the Phase 53 instrument's harness pattern), the
//  file is compiled with rustc and executed, and the measured
//  pass ratio replaces the FALLBACK tpr before selection.
//
//  POLICY (recorded, Phase 57 item 5b): where the runner runs,
//  the compiled vehicle is the behavioral ground truth. The
//  MCTS reward loop keeps the interpreter/FALLBACK signal
//  during search (one rustc per candidate is impossible
//  there); this stage runs once, after verification, on a
//  bounded finalist set.
//
//  TRIGGER (hard invariant): the stage is a no-op unless
//  the spec HAS in-spec tests and the interpreter cannot
//  bind all of them (`eval::bindable_test_count(spec) <
//  tests.len()`). When every test binds, the returned pool
//  is byte-identical to the input — the M2/M3 instruments'
//  measurements rest on this, and their specs never trigger
//  the stage.
//
//  TRIGGER EXTENSION (Phase 60, campaign slice G3): the
//  invariant above covers specs whose TESTS the
//  interpreter cannot bind. A second, disjoint coverage
//  gap exists on the BODY side: a constraint-constructed
//  seed (the forall-fold scan is the measured case) whose
//  body the interpreter cannot evaluate on ANY bindable
//  test — its interpreter test_pass_ratio is 0.0 by
//  coverage, not by behavior, so selection ranks it below
//  bodies the interpreter merely scores low (count_matching's
//  seed at 0.0 beside constant-0's measured 1/3), and Z3
//  cannot crown it either (loop bodies are Inconclusive
//  by construction). The stage therefore ALSO triggers
//  when every test binds but the pool holds such a seed
//  (`Provenance::ConstraintInduced` and
//  `eval::valued_test_count(body, spec) == 0`); the
//  triggering seeds are guaranteed finalist slots, like
//  the would-be winner. Scope, recorded: CONSTRUCTED
//  CONSTRAINT SEEDS ONLY. Template-provenance bodies are
//  NOT a trigger — is_sorted's adjacent-scan template
//  sits behind the same interpreter wall, and extending
//  compiled judging to it is a judging-policy decision
//  deliberately left for a separate slice (Phase 59's
//  record); arbitrary candidates (Mutation/MCTS bodies)
//  are the broader wall beyond that. Specs whose
//  constructed seeds the interpreter CAN evaluate are
//  untouched: their seeds' scores are behavioral truth
//  already, and the M2/M3/tokenizer winner rows — every
//  one an interpreter-evaluable constraint seed — stay
//  byte-identical.
//
//  TRIGGER EXTENSION (Phase 61, campaign slice G6): the
//  Phase 60 extension guaranteed the TRIGGERING seeds a
//  finalist slot. The same gap exists one provenance over
//  inside the ORIGINAL trigger's territory: when the stage
//  fires via UnbindableTests, a template seed (a complete,
//  hole-free constructed body) can rank outside the top-k
//  on composite — sort's scan template measured finalist
//  in 0/5 draws (Phase 61), so the stage never priced the
//  one constructed body in the pool. Within an
//  already-triggered UnbindableTests stage, Template-
//  provenance candidates are therefore guaranteed finalist
//  slots by the same displacement rule. Scope, recorded:
//  the TRIGGER is unchanged — Template provenance does
//  not fire the stage (a bindable-tests spec holding a
//  template, is_sorted's shape, still never triggers;
//  that wall remains reserved for a separate decision,
//  exactly as Phase 60 left it), and the Phase 60 arm's
//  guarantee set is exactly its triggering seeds,
//  unchanged.
//
//  FINALISTS: the top `RUNNER_FINALISTS` candidates by the
//  existing composite ranking, with the would-be winner
//  (what `select_best` crowns on the un-rescored pool)
//  guaranteed a slot — it takes the last finalist's place
//  if it ranked outside the top-k. Under the Phase 60
//  extension the triggering seeds are guaranteed slots by
//  the same rule, displacing the lowest-ranked unprotected
//  finalists (never the would-be winner, never another
//  guaranteed seed); under the Phase 61 extension the
//  same holds for Template-provenance candidates inside
//  an UnbindableTests stage.
//
//  RANKING SEMANTICS (explicit): a finalist's runner ratio
//  replaces its test_pass_ratio (times the same per-hole
//  factor `evaluate_candidate` applies, so holed finalists
//  keep their discount). A finalist that fails to compile, or
//  whose runner times out, scores 0 — compilability is priced
//  by construction. Non-finalists keep their FALLBACK score
//  but are CAPPED at the worst finalist's measured ratio: an
//  unmeasured candidate may not claim a higher test score
//  than the worst measured finalist, so no unscored
//  fallback-1.0 can outrank a runner-scored finalist on the
//  tpr dimension.
//
//  DEGRADATION: infrastructure failure — rustc cannot be
//  spawned, the temp dir cannot be created or written, or
//  generate's own test module cannot be re-read in its
//  expected shape — degrades the WHOLE spec to today's
//  fallback behavior (the pool is returned unchanged). A
//  build must never fail because the runner failed. Per-
//  candidate compile/run timeouts are verdicts (score 0),
//  not infrastructure failures: the code being compiled and
//  run is the candidate's own.
// ─────────────────────────────────────────────────────────────

use crate::codegen;
use crate::spec::ast::{Expr, FunctionSpec};
use crate::synthesis::engine::{
    composite_score, count_holes, CandidateImplementation, Provenance,
};
use crate::synthesis::selector;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How many finalists the runner judges per triggered spec.
pub const RUNNER_FINALISTS: usize = 8;

/// rustc budget per finalist (Phase 53 instrument precedent).
const COMPILE_TIMEOUT: Duration = Duration::from_secs(60);
/// Runner execution budget per finalist (Phase 53 precedent).
const RUN_TIMEOUT: Duration = Duration::from_secs(10);

/// One finalist's measured outcome.
#[derive(Debug, Clone)]
pub struct FinalistOutcome {
    pub candidate_id: u64,
    pub passed: usize,
    pub total: usize,
    /// The test_pass_ratio the finalist carries after
    /// rescoring (runner ratio × hole factor).
    pub new_test_pass_ratio: f64,
}

/// What the stage did, for instruments and reports.
#[derive(Debug)]
pub enum RunnerOutcome {
    /// Trigger condition not met; pool returned unchanged.
    NotTriggered,
    /// Triggered, but an infrastructure failure degraded the
    /// whole spec to the fallback behavior; pool unchanged.
    Degraded,
    /// Triggered and scored.
    Scored {
        finalists: Vec<FinalistOutcome>,
        capped_non_finalists: usize,
        elapsed: Duration,
    },
}

enum RunnerVerdict {
    Scored { passed: usize, total: usize },
    InfraFailure,
}

/// Why the stage runs for a pool (see the module header's
/// TRIGGER sections). Pure and eval-only — unit-tested
/// without rustc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trigger {
    /// Phase 58: some in-spec tests cannot bind in the
    /// interpreter, so every candidate sits at the FALLBACK
    /// test_pass_ratio.
    UnbindableTests,
    /// Phase 60: every test binds, but these pool indices
    /// are constraint-constructed seeds the interpreter
    /// cannot evaluate on any test. They are guaranteed
    /// finalist slots.
    ConstructedSeeds(Vec<usize>),
}

/// The stage's trigger decision. `None` — the hard
/// invariant's no-op — unless the spec has tests and one
/// of the two recorded coverage gaps holds.
pub fn trigger(candidates: &[CandidateImplementation], spec: &FunctionSpec) -> Option<Trigger> {
    if candidates.is_empty() || spec.tests.is_empty() {
        return None;
    }
    if super::eval::bindable_test_count(spec) < spec.tests.len() {
        return Some(Trigger::UnbindableTests);
    }
    // Phase 60 extension: all tests bind, so a candidate's
    // interpreter score is behavioral truth — EXCEPT a
    // constraint-constructed seed whose body the
    // interpreter cannot evaluate at all (valued on zero
    // bindable tests): its 0.0 records the interpreter's
    // subset, not the body's behavior.
    let seeds: Vec<usize> = candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c.provenance, Provenance::ConstraintInduced { .. }))
        .filter(|(_, c)| super::eval::valued_test_count(&c.body, spec) == 0)
        .map(|(i, _)| i)
        .collect();
    if seeds.is_empty() {
        None
    } else {
        Some(Trigger::ConstructedSeeds(seeds))
    }
}

/// The pipeline stage, as wired into `main.rs` between
/// verification and selection (the `verify_all` precedent: a
/// stage that takes the pool and returns a re-scored pool).
pub fn rescore_finalists(
    candidates: Vec<CandidateImplementation>,
    spec: &FunctionSpec,
) -> Vec<CandidateImplementation> {
    rescore_finalists_report(candidates, spec).0
}

/// `rescore_finalists`, also reporting what it did.
pub fn rescore_finalists_report(
    candidates: Vec<CandidateImplementation>,
    spec: &FunctionSpec,
) -> (Vec<CandidateImplementation>, RunnerOutcome) {
    // Trigger (hard invariant + Phase 60 extension — see
    // the module header). The guarantee set: the Phase 60
    // arm's triggering seeds, or — Phase 61 — the pool's
    // Template-provenance candidates inside an
    // UnbindableTests stage.
    let trig = match trigger(&candidates, spec) {
        None => return (candidates, RunnerOutcome::NotTriggered),
        Some(t) => t,
    };
    let guaranteed: Vec<usize> = guaranteed_finalists(&candidates, &trig);

    let started = Instant::now();
    let dir = std::env::temp_dir().join(format!(
        "morphic-runner-{}-{}",
        std::process::id(),
        spec.name
    ));
    if std::fs::create_dir_all(&dir).is_err() {
        return (candidates, RunnerOutcome::Degraded);
    }

    let finalist_idx = choose_finalists(&candidates, spec, &guaranteed);
    let mut verdicts: Vec<(usize, usize, usize)> = Vec::new();
    let mut degraded = false;
    for &idx in &finalist_idx {
        match runner_verdict(&candidates[idx], spec, &dir) {
            RunnerVerdict::Scored { passed, total } => verdicts.push((idx, passed, total)),
            RunnerVerdict::InfraFailure => {
                degraded = true;
                break;
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    if degraded {
        return (candidates, RunnerOutcome::Degraded);
    }

    let (pool, outcomes, capped) = apply_runner_scores(candidates, &finalist_idx, &verdicts);
    (
        pool,
        RunnerOutcome::Scored {
            finalists: outcomes,
            capped_non_finalists: capped,
            elapsed: started.elapsed(),
        },
    )
}

/// The guarantee set for a triggered stage (see the
/// module header's TRIGGER EXTENSION sections). Pure —
/// unit-tested without rustc.
/// - Phase 60 arm: exactly the triggering constraint
///   seeds' pool indices.
/// - Phase 61 (inside the original UnbindableTests
///   trigger only): the pool's Template-provenance
///   candidates — constructed bodies the composite ranking
///   can leave outside the top-k (sort's scan template:
///   finalist in 0/5 draws before this extension).
fn guaranteed_finalists(
    candidates: &[CandidateImplementation],
    trigger: &Trigger,
) -> Vec<usize> {
    match trigger {
        Trigger::ConstructedSeeds(indices) => indices.clone(),
        Trigger::UnbindableTests => candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| matches!(c.provenance, Provenance::Template { .. }))
            .map(|(i, _)| i)
            .collect(),
    }
}

/// The finalist set: top `RUNNER_FINALISTS` by the existing
/// composite ranking (stable — ties keep pool order), with
/// the would-be winner guaranteed a slot, and — under the
/// Phase 60 trigger extension — each triggering seed in
/// `guaranteed` guaranteed one too. A guarantee displaces
/// the lowest-ranked finalist that is neither the would-be
/// winner nor another guaranteed seed; with a pool smaller
/// than k everyone is a finalist anyway.
fn choose_finalists(
    candidates: &[CandidateImplementation],
    spec: &FunctionSpec,
    guaranteed: &[usize],
) -> Vec<usize> {
    let mut order: Vec<usize> = (0..candidates.len()).collect();
    order.sort_by(|&a, &b| {
        composite_score(&candidates[b].scores)
            .partial_cmp(&composite_score(&candidates[a].scores))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut chosen: Vec<usize> =
        order.iter().take(RUNNER_FINALISTS).cloned().collect();
    let mut protected: Vec<usize> = Vec::new();
    if let Ok(winner) = selector::select_best(candidates.to_vec(), spec) {
        if let Some(pos) = candidates.iter().position(|c| c.id == winner.id) {
            protected.push(pos);
            if !chosen.contains(&pos) {
                if chosen.len() >= RUNNER_FINALISTS {
                    chosen.pop();
                }
                chosen.push(pos);
            }
        }
    }
    for &idx in guaranteed {
        if chosen.contains(&idx) {
            if !protected.contains(&idx) {
                protected.push(idx);
            }
            continue;
        }
        if chosen.len() >= RUNNER_FINALISTS {
            // Displace the lowest-ranked unprotected
            // finalist; if every finalist is protected
            // (a degenerate pool), the guarantee yields —
            // it cannot evict the winner or a fellow seed.
            let Some(displace) = chosen
                .iter()
                .rposition(|i| !protected.contains(i))
            else {
                continue;
            };
            chosen.remove(displace);
        }
        chosen.push(idx);
        protected.push(idx);
    }
    chosen
}

/// Apply measured verdicts to the pool: finalists get their
/// runner ratio (× the hole factor, mirroring
/// `evaluate_candidate`); non-finalists are capped at the
/// worst finalist's new ratio. Pure — unit-tested without
/// rustc. Returns the pool, the per-finalist outcomes, and
/// how many non-finalists were capped.
fn apply_runner_scores(
    mut candidates: Vec<CandidateImplementation>,
    finalists: &[usize],
    verdicts: &[(usize, usize, usize)],
) -> (Vec<CandidateImplementation>, Vec<FinalistOutcome>, usize) {
    let mut new_tpr: HashMap<usize, f64> = HashMap::new();
    let mut outcomes = Vec::new();
    for &(idx, passed, total) in verdicts {
        let ratio = if total > 0 {
            passed as f64 / total as f64
        } else {
            0.0
        };
        let hole_factor = 0.5f64.powi(count_holes(&candidates[idx].body) as i32);
        let t = ratio * hole_factor;
        new_tpr.insert(idx, t);
        outcomes.push(FinalistOutcome {
            candidate_id: candidates[idx].id,
            passed,
            total,
            new_test_pass_ratio: t,
        });
    }
    if new_tpr.is_empty() {
        return (candidates, outcomes, 0);
    }
    let cap = new_tpr.values().cloned().fold(f64::INFINITY, f64::min);
    let finalist_set: std::collections::HashSet<usize> = finalists.iter().cloned().collect();
    let mut capped = 0usize;
    for (i, c) in candidates.iter_mut().enumerate() {
        if let Some(&t) = new_tpr.get(&i) {
            c.scores.test_pass_ratio = t;
            c.score = composite_score(&c.scores);
        } else if !finalist_set.contains(&i) && c.scores.test_pass_ratio > cap {
            c.scores.test_pass_ratio = cap;
            c.score = composite_score(&c.scores);
            capped += 1;
        }
    }
    (candidates, outcomes, capped)
}

/// Judge one finalist by the compiled vehicle.
fn runner_verdict(
    candidate: &CandidateImplementation,
    spec: &FunctionSpec,
    dir: &Path,
) -> RunnerVerdict {
    let total = spec.tests.len();
    let scored0 = RunnerVerdict::Scored { passed: 0, total };
    // Render through the production codegen — the same output
    // the build itself ships. A candidate the codegen cannot
    // render passes nothing.
    let generated = match codegen::generate(candidate, "rust") {
        Ok(text) => text,
        Err(_) => return scored0,
    };
    // If generate's own test module cannot be re-read in its
    // expected shape, the harness — not the candidate — is at
    // fault: infrastructure failure, degrade the whole spec.
    let source = match build_runner_source(&generated, spec) {
        Some(s) => s,
        None => return RunnerVerdict::InfraFailure,
    };
    let sub = dir.join(format!("c{}", candidate.id));
    if std::fs::create_dir_all(&sub).is_err() {
        return RunnerVerdict::InfraFailure;
    }
    let src_path = sub.join("runner.rs");
    if std::fs::write(&src_path, &source).is_err() {
        return RunnerVerdict::InfraFailure;
    }
    let bin_path = sub.join("runner");
    let _ = std::fs::remove_file(&bin_path);
    let src_arg = src_path.to_string_lossy().into_owned();
    let bin_arg = bin_path.to_string_lossy().into_owned();
    match run_timed(
        &sub,
        "cc",
        "rustc",
        &["--edition", "2021", &src_arg, "-o", &bin_arg],
        COMPILE_TIMEOUT,
    ) {
        TimedResult::SpawnFailed => RunnerVerdict::InfraFailure,
        TimedResult::Timeout => scored0,
        TimedResult::Finished(false, _) => scored0,
        TimedResult::Finished(true, _) => match run_timed(&sub, "run", &bin_arg, &[], RUN_TIMEOUT) {
            TimedResult::SpawnFailed => RunnerVerdict::InfraFailure,
            TimedResult::Timeout => scored0,
            TimedResult::Finished(_, stdout) => match parse_total(&stdout) {
                Some((passed, _)) => RunnerVerdict::Scored { passed, total },
                None => scored0,
            },
        },
    }
}

/// The runner's `TOTAL p/t` line.
fn parse_total(stdout: &str) -> Option<(usize, usize)> {
    stdout.lines().find_map(|line| {
        let rest = line.strip_prefix("TOTAL ")?;
        let (p, t) = rest.split_once('/')?;
        Some((p.trim().parse().ok()?, t.trim().parse().ok()?))
    })
}

/// Build the runner source: generate's winner function plus a
/// generated main. The main's per-test invocations are re-read
/// from generate's OWN generated test module (the one true
/// test-value rendering — tuple spread, lambda and `List`
/// forms included) and annotated with a concrete element type
/// (turbofish), the Phase 53 harness pattern: without it, an
/// empty-list test leaves the generic element type unpinned
/// and the harness itself fails to compile (the E0283 class —
/// the shipped test module's own fix is G6's territory; here
/// only the harness annotates). Returns None when the
/// generated text does not have the expected shape.
fn build_runner_source(generated: &str, spec: &FunctionSpec) -> Option<String> {
    let mut fn_part = String::new();
    let mut test_part = String::new();
    let mut in_tests = false;
    for line in generated.lines() {
        if line.trim() == "#[cfg(test)]" {
            in_tests = true;
        }
        if in_tests {
            test_part.push_str(line);
            test_part.push('\n');
        } else {
            fn_part.push_str(line);
            fn_part.push('\n');
        }
    }
    if test_part.is_empty() {
        return None;
    }
    let invocations = extract_invocations(&test_part)?;
    // The harness must account for every in-spec test; any
    // drift in generate's test-module shape disables the
    // stage for this spec instead of mis-scoring it.
    if invocations.len() != spec.tests.len() {
        return None;
    }
    let mut checks = Vec::new();
    for (call, expected) in &invocations {
        checks.push((annotate_call(call, spec)?, expected.clone()));
    }
    let mut source = fn_part;
    source.push_str(&runner_main(&checks));
    Some(source)
}

/// Re-read the per-test invocations from a generated test
/// module: pairs of (call text, expected text) from each
/// `let result = <call>;` / `assert_eq!(result, <expected>);`
/// statement pair.
fn extract_invocations(test_part: &str) -> Option<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut rest = test_part;
    while let Some(pos) = rest.find("let result = ") {
        let after = &rest[pos + "let result = ".len()..];
        let (call, used) = scan_statement(after)?;
        let tail = &after[used..];
        let apos = tail.find("assert_eq!(result,")?;
        let exp_start = apos + "assert_eq!(result,".len();
        let expected = scan_assert_arg(&tail[exp_start..])?;
        out.push((call.trim().to_string(), expected.trim().to_string()));
        rest = &tail[exp_start..];
    }
    Some(out)
}

/// Scan one expression statement: from `start` to the `;` at
/// nesting depth 0 (string/char literals respected). Returns
/// the statement text (without the `;`) and the number of
/// bytes consumed including the `;`.
fn scan_statement(text: &str) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i] as char;
        match c {
            '"' => i = skip_string(text, i)?,
            '\'' => i = skip_char_literal(text, i),
            '(' | '[' | '{' => {
                depth += 1;
                i += 1;
            }
            ')' | ']' | '}' => {
                depth -= 1;
                i += 1;
            }
            ';' if depth == 0 => return Some((text[..i].to_string(), i + 1)),
            _ => i += 1,
        }
    }
    None
}

/// Scan an `assert_eq!` argument: from just after
/// `assert_eq!(result,` to the assert's own closing paren at
/// nesting depth 0. Returns the argument text.
fn scan_assert_arg(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i] as char;
        match c {
            '"' => i = skip_string(text, i)?,
            '\'' => i = skip_char_literal(text, i),
            '(' | '[' | '{' => {
                depth += 1;
                i += 1;
            }
            ')' if depth == 0 => return Some(text[..i].to_string()),
            ')' | ']' | '}' => {
                depth -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

/// Skip a double-quoted string literal starting at `i`
/// (escapes respected); returns the index just past the
/// closing quote.
fn skip_string(text: &str, i: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut j = i + 1;
    while j < bytes.len() {
        match bytes[j] {
            b'\\' => j += 2,
            b'"' => return Some(j + 1),
            _ => j += 1,
        }
    }
    None
}

/// Skip a char literal starting at `i` when the text there
/// has a char-literal shape (`'x'`, `'\n'`, `'\u{...}'`);
/// otherwise treat the quote as an ordinary character (a
/// lifetime) and advance by one.
fn skip_char_literal(text: &str, i: usize) -> usize {
    let rest = &text[i + 1..];
    let mut chars = rest.char_indices();
    match chars.next() {
        Some((_, '\\')) => {
            // Escape form: scan to the closing quote.
            let bytes = text.as_bytes();
            let mut j = i + 2;
            while j < bytes.len() {
                if bytes[j] == b'\'' {
                    return j + 1;
                }
                j += 1;
            }
            text.len()
        }
        Some((_, c)) => {
            // Single character followed by a closing quote.
            let after = i + 1 + c.len_utf8();
            if text[after..].starts_with('\'') {
                after + 1
            } else {
                i + 1
            }
        }
        None => i + 1,
    }
}

/// Annotate a re-read call with a turbofish of concrete types
/// when the spec is generic (see build_runner_source).
/// Returns None when the call does not start with the spec's
/// name — a harness/format mismatch, not a candidate fault.
fn annotate_call(call: &str, spec: &FunctionSpec) -> Option<String> {
    if spec.generics.is_empty() {
        return Some(call.to_string());
    }
    let name = &spec.name;
    if !call.starts_with(name.as_str()) {
        return None;
    }
    let rest = &call[name.len()..];
    // Phase 61 (G6): the shipped test module itself now
    // annotates its E0283-class calls (the generics_text
    // the driver hands rust_test_case) — a re-read call
    // that already carries a turbofish is used as-is,
    // never annotated twice.
    if rest.starts_with("::<") {
        return Some(call.to_string());
    }
    if !rest.starts_with('(') {
        return None;
    }
    let ty = concrete_type_for_spec(spec);
    let types = vec![ty; spec.generics.len()].join(", ");
    Some(format!("{}::<{}>{}", name, types, rest))
}

/// The concrete element type the harness pins the spec's
/// generics to: the first scalar literal kind found in the
/// spec's own tests (DFS over inputs, then expected outputs,
/// test by test), defaulting to i64. A harness heuristic over
/// the spec's own data — the tests are written against one
/// concrete element type, and the harness adopts it.
fn concrete_type_for_spec(spec: &FunctionSpec) -> &'static str {
    for test in &spec.tests {
        if let Some(ty) = first_literal_type(&test.input) {
            return ty;
        }
        if let Some(ty) = first_literal_type(&test.expected_output) {
            return ty;
        }
    }
    "i64"
}

fn first_literal_type(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::IntLit(_) => Some("i64"),
        Expr::FloatLit(_) => Some("f64"),
        Expr::BoolLit(_) => Some("bool"),
        Expr::StringLit(_) => Some("String"),
        Expr::CharLit(_) => Some("char"),
        Expr::Var(_) => None,
        Expr::Field(inner, _) => first_literal_type(inner),
        Expr::Index(a, b) => first_literal_type(a).or_else(|| first_literal_type(b)),
        Expr::Call(_, args) => args.iter().find_map(|a| first_literal_type(a)),
        Expr::BinOp(_, l, r) => {
            first_literal_type(l).or_else(|| first_literal_type(r))
        }
        Expr::UnaryOp(_, e) => first_literal_type(e),
        Expr::Lambda(_, body) => first_literal_type(body),
        Expr::Length(e) => first_literal_type(e),
        Expr::Comprehension {
            collection, body, ..
        } => first_literal_type(collection).or_else(|| first_literal_type(body)),
        // M1 features (Phase 73): same structural walk as
        // codegen's copy of this heuristic — receiver before
        // arguments, field values in written order.
        Expr::MethodCall { receiver, args, .. } => first_literal_type(receiver)
            .or_else(|| args.iter().find_map(|a| first_literal_type(a))),
        Expr::RecordLit { fields, .. } => fields
            .iter()
            .find_map(|(_, v)| first_literal_type(v)),
    }
}

/// The generated runner main: each test runs under
/// catch_unwind (a panicking candidate fails that test, it
/// does not kill the harness), panic output is silenced, and
/// a single TOTAL line reports the outcome.
fn runner_main(checks: &[(String, String)]) -> String {
    let mut s = String::new();
    s.push_str("\nfn main() {\n");
    s.push_str("    std::panic::set_hook(Box::new(|_| {}));\n");
    s.push_str("    let mut passed: usize = 0;\n");
    for (call, expected) in checks {
        s.push_str("    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { let result = ");
        s.push_str(call);
        s.push_str("; result == ");
        s.push_str(expected);
        s.push_str(" })).unwrap_or(false) { passed += 1; }\n");
    }
    s.push_str("    println!(\"TOTAL {}/");
    s.push_str(&checks.len().to_string());
    s.push_str("\", passed);\n");
    s.push_str("}\n");
    s
}

enum TimedResult {
    Finished(bool, String),
    Timeout,
    SpawnFailed,
}

/// Run a command with a timeout, output redirected to files
/// under `dir` (the Phase 53 instrument's pattern,
/// productionized). Returns the outcome plus stdout.
fn run_timed(
    dir: &Path,
    tag: &str,
    program: &str,
    args: &[&str],
    timeout: Duration,
) -> TimedResult {
    let out_path: PathBuf = dir.join(format!("{}.stdout", tag));
    let err_path: PathBuf = dir.join(format!("{}.stderr", tag));
    let out_file = match std::fs::File::create(&out_path) {
        Ok(f) => f,
        Err(_) => return TimedResult::SpawnFailed,
    };
    let err_file = match std::fs::File::create(&err_path) {
        Ok(f) => f,
        Err(_) => return TimedResult::SpawnFailed,
    };
    let mut child = match Command::new(program)
        .args(args)
        .stdout(Stdio::from(out_file))
        .stderr(Stdio::from(err_file))
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return TimedResult::SpawnFailed,
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = std::fs::read_to_string(&out_path).unwrap_or_default();
                return TimedResult::Finished(status.success(), stdout);
            }
            Ok(None) => {
                if started.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return TimedResult::Timeout;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => return TimedResult::SpawnFailed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{parser, typeck};
    use crate::synthesis::engine::{
        CollectionKind, IRNode, Provenance, ScoreBreakdown,
    };

    fn spec_from(source: &str) -> FunctionSpec {
        let parsed = parser::parse(source).expect("parse");
        let checked = typeck::check(parsed).expect("typeck");
        checked.spec.functions.into_iter().next().expect("one spec")
    }

    /// A miniature of examples/sort.morph: generic, fn-typed
    /// parameter, lambda-carrying tuple tests. (Until
    /// Phase 130 this was also the trigger's UnbindableTests
    /// shape; its tests bind since the evaluator gained
    /// function values — the trigger's standing fixture is
    /// UNBINDABLE_TESTS below.)
    const MINI_SORT: &str = r#"
Mini {
spec mini_sort<T> {
    input: list: List<T>, cmp: fn(T, T) -> Bool
    output: List<T>

    test "empty" {
        input: ([], |a, b| a < b)
        expect: []
    }

    test "single" {
        input: ([5], |a, b| a < b)
        expect: [5]
    }

    test "reverse" {
        input: ([2, 1], |a, b| a < b)
        expect: [1, 2]
    }
}
}
"#;

    /// A spec whose only test still cannot bind after
    /// Phase 130: the input tuple carries a NESTED tuple,
    /// and there is no tuple Value for the element to map
    /// to — the remaining Unbindable class. (Named function
    /// references, the other former class, bind since
    /// Phase 132 wherever a judging route carries a module
    /// index; this stage's context-free count still reads
    /// them as unbindable, having no index in hand.) The
    /// Phase 58 trigger's standing fixture.
    const UNBINDABLE_TESTS: &str = r#"
Mini {
spec nested_pair {
    input: pair: List<Int>, x: Int
    output: Int

    test "nested" {
        input: ((1, 2), 3)
        expect: 3
    }
}
}
"#;

    /// First-order tests only — every test binds in the
    /// interpreter, so the stage must never trigger.
    const FIRST_ORDER: &str = r#"
Mini {
spec add_one {
    input: x: Int
    output: Int

    test "one" {
        input: 1
        expect: 2
    }

    test "two" {
        input: 2
        expect: 3
    }
}
}
"#;

    /// The count_matching shape (Phase 60): two parameters,
    /// tuple tests of first-order values — every test binds
    /// in the interpreter (Phase 53), so the Phase 58
    /// trigger stays silent; only the Phase 60 extension
    /// (a constraint-constructed seed the interpreter
    /// cannot evaluate) may fire.
    const COUNT_SHAPED: &str = r#"
Mini {
spec count_above {
    input: values: List<Int>, limit: Int
    output: Int

    constraint: forall x: Int in values:
        x > limit implies output > 0
    constraint: output >= 0

    test "all" { input: ([10, 20, 30], 5); expect: 3 }
    test "none" { input: ([1, 2, 3], 5); expect: 0 }
    test "mixed" { input: ([1, 10, 2, 20], 5); expect: 2 }
}
}
"#;

    /// The forall-fold scan body for COUNT_SHAPED — the
    /// Phase 60 family's construction (count the elements
    /// above the limit). The interpreter binds the spec's
    /// tests but cannot evaluate this body on any of them:
    /// its unevaluable ingredient stays outside the
    /// subset. The fixture's construction has been
    /// re-based each time the subset grew, semantics
    /// unchanged (the count of elements above the limit):
    /// the original was a For scan (Alloc + For + Index +
    /// conditional increment) — Alloc/Assign graduated in
    /// Phase 95 and For in Phase 115 (the List family's
    /// judging machinery); then a Fold over a Lambda —
    /// the higher-order nodes graduated in Phase 131
    /// (G4). The ingredient now is the Tail-chop: the
    /// scan walks the list by repeatedly Assigning
    /// `rest = Tail(rest)` and reading `Head(rest)`, and
    /// Head/Tail remain outside the evaluable subset
    /// (the Phase 95 residual: no measured consumer) —
    /// while their compiled renderings (`(rest)[0]`,
    /// `(rest)[1..].to_vec()`) are the standard ones, so
    /// the compiled vehicle scores the body 3/3.
    fn scan_body() -> IRNode {
        let var = |n: &str| IRNode::Var(n.to_string());
        let int = |n: i64| {
            IRNode::Literal(crate::synthesis::engine::IRLiteral::Int(n))
        };
        let binop = |op: crate::synthesis::engine::IRBinOp, l: IRNode, r: IRNode| {
            IRNode::BinOp(op, Box::new(l), Box::new(r))
        };
        IRNode::Block(vec![
            IRNode::Alloc {
                name: "count".into(),
                ty: crate::spec::ast::TypeRef::Int,
                initial: Box::new(int(0)),
            },
            IRNode::Alloc {
                name: "rest".into(),
                ty: crate::spec::ast::TypeRef::List(Box::new(
                    crate::spec::ast::TypeRef::Int,
                )),
                initial: Box::new(var("values")),
            },
            IRNode::While {
                condition: Box::new(binop(
                    crate::synthesis::engine::IRBinOp::Gt,
                    IRNode::UnaryOp(
                        crate::synthesis::engine::IRUnaryOp::Len,
                        Box::new(var("rest")),
                    ),
                    int(0),
                )),
                body: Box::new(IRNode::Block(vec![
                    IRNode::If {
                        condition: Box::new(binop(
                            crate::synthesis::engine::IRBinOp::Gt,
                            IRNode::UnaryOp(
                                crate::synthesis::engine::IRUnaryOp::Head,
                                Box::new(var("rest")),
                            ),
                            var("limit"),
                        )),
                        then_branch: Box::new(IRNode::Assign {
                            target: Box::new(var("count")),
                            value: Box::new(binop(
                                crate::synthesis::engine::IRBinOp::Add,
                                var("count"),
                                int(1),
                            )),
                        }),
                        else_branch: Box::new(IRNode::Literal(
                            crate::synthesis::engine::IRLiteral::Unit,
                        )),
                    },
                    IRNode::Assign {
                        target: Box::new(var("rest")),
                        value: Box::new(IRNode::UnaryOp(
                            crate::synthesis::engine::IRUnaryOp::Tail,
                            Box::new(var("rest")),
                        )),
                    },
                ])),
                invariant: None,
            },
            IRNode::Return(Box::new(var("count"))),
        ])
    }

    fn candidate_with_provenance(
        id: u64,
        spec: &FunctionSpec,
        body: IRNode,
        tpr: f64,
        provenance: Provenance,
    ) -> CandidateImplementation {
        let scores = ScoreBreakdown {
            test_pass_ratio: tpr,
            constraint_score: 0.0,
            complexity_score: 0.8,
            quality_score: 0.8,
            synthesis_time_us: 0,
        };
        CandidateImplementation {
            id,
            body,
            spec_name: spec.name.clone(),
            spec: spec.clone(),
            score: composite_score(&scores),
            scores,
            generation: 0,
            provenance,
            verified: false,
            verification_status: None,
        }
    }

    const NO_TESTS: &str = r#"
Mini {
spec add_one {
    input: x: Int
    output: Int
}
}
"#;

    fn candidate(
        id: u64,
        spec: &FunctionSpec,
        body: IRNode,
        tpr: f64,
        constraint: f64,
    ) -> CandidateImplementation {
        let scores = ScoreBreakdown {
            test_pass_ratio: tpr,
            constraint_score: constraint,
            complexity_score: 0.8,
            quality_score: 0.8,
            synthesis_time_us: 0,
        };
        CandidateImplementation {
            id,
            body,
            spec_name: spec.name.clone(),
            spec: spec.clone(),
            score: composite_score(&scores),
            scores,
            generation: 0,
            provenance: Provenance::Template {
                template_name: "test".into(),
            },
            verified: false,
            verification_status: None,
        }
    }

    fn tprs(pool: &[CandidateImplementation]) -> Vec<f64> {
        pool.iter().map(|c| c.scores.test_pass_ratio).collect()
    }

    #[test]
    fn rescore_noop_when_no_tests() {
        let spec = spec_from(NO_TESTS);
        let pool = vec![candidate(1, &spec, IRNode::Var("x".into()), 1.0, 0.0)];
        let (out, outcome) = rescore_finalists_report(pool, &spec);
        assert!(matches!(outcome, RunnerOutcome::NotTriggered));
        assert_eq!(tprs(&out), vec![1.0]);
    }

    #[test]
    fn rescore_noop_when_every_test_binds() {
        let spec = spec_from(FIRST_ORDER);
        assert_eq!(crate::synthesis::eval::bindable_test_count(&spec), 2);
        let pool = vec![
            candidate(1, &spec, IRNode::Var("x".into()), 1.0, 0.0),
            candidate(2, &spec, IRNode::Var("x".into()), 0.5, 0.0),
        ];
        let (out, outcome) = rescore_finalists_report(pool, &spec);
        assert!(matches!(outcome, RunnerOutcome::NotTriggered));
        assert_eq!(tprs(&out), vec![1.0, 0.5]);
    }

    /// Phase 60 trigger extension: all tests bind, but the
    /// pool holds a constraint-constructed seed the
    /// interpreter cannot evaluate on any test — the stage
    /// must fire, naming the seed's pool index.
    #[test]
    fn trigger_fires_for_unevaluable_constraint_seed() {
        let spec = spec_from(COUNT_SHAPED);
        assert_eq!(crate::synthesis::eval::bindable_test_count(&spec), 3);
        let seed = candidate_with_provenance(
            1,
            &spec,
            scan_body(),
            0.0,
            Provenance::ConstraintInduced { kind: "forall_fold".into() },
        );
        let constant = candidate_with_provenance(
            2,
            &spec,
            IRNode::Literal(crate::synthesis::engine::IRLiteral::Int(0)),
            1.0 / 3.0,
            Provenance::Mutation { parent_id: 0, mutation_kind: "test".into() },
        );
        match trigger(&[seed, constant], &spec) {
            Some(Trigger::ConstructedSeeds(indices)) => {
                assert_eq!(indices, vec![0], "the seed's pool index must be named");
            }
            other => panic!("expected ConstructedSeeds trigger, got {:?}", other),
        }
    }

    /// The extension's scope limits, recorded as tests:
    /// an EVALUABLE constraint seed needs no bridge; an
    /// unevaluable body of any other provenance — a
    /// template seed (is_sorted's wall, reserved for a
    /// separate decision) or an arbitrary mutation — does
    /// not fire the stage.
    #[test]
    fn trigger_ignores_evaluable_seeds_and_other_provenances() {
        let spec = spec_from(COUNT_SHAPED);
        // Evaluable constraint seed (an expression body the
        // interpreter judges directly).
        let evaluable_seed = candidate_with_provenance(
            1,
            &spec,
            IRNode::Literal(crate::synthesis::engine::IRLiteral::Int(0)),
            1.0 / 3.0,
            Provenance::ConstraintInduced { kind: "equation".into() },
        );
        assert!(trigger(&[evaluable_seed], &spec).is_none());
        // The same unevaluable scan body under Template
        // provenance: no trigger (the recorded scope limit).
        let template = candidate_with_provenance(
            2,
            &spec,
            scan_body(),
            0.0,
            Provenance::Template { template_name: "adjacent_scan".into() },
        );
        assert!(trigger(&[template], &spec).is_none());
        // Under Mutation provenance: no trigger either —
        // arbitrary candidates are the broader wall, not
        // this extension's.
        let mutation = candidate_with_provenance(
            3,
            &spec,
            scan_body(),
            0.0,
            Provenance::Mutation { parent_id: 0, mutation_kind: "test".into() },
        );
        assert!(trigger(&[mutation], &spec).is_none());
        // No candidates / the Phase 58 trigger: unchanged.
        // (Its fixture is UNBINDABLE_TESTS since Phase 130 —
        // MINI_SORT's lambda-carrying tests bind now that
        // the evaluator has function values, so mini_sort no
        // longer stands in UnbindableTests territory.)
        let unbindable = spec_from(UNBINDABLE_TESTS);
        let any = candidate(1, &unbindable, IRNode::Var("x".into()), 1.0, 0.0);
        assert!(matches!(trigger(&[any], &unbindable), Some(Trigger::UnbindableTests)));
    }

    /// The guaranteed-slot rule: a triggering seed outside
    /// the top-k (and not the would-be winner) still becomes
    /// a finalist, displacing the lowest-ranked unprotected
    /// finalist — never the would-be winner and never
    /// another guaranteed seed.
    #[test]
    fn choose_finalists_guarantees_triggering_seeds() {
        let spec = spec_from(COUNT_SHAPED);
        // Ten candidates, composite strictly decreasing in
        // id; the seed sits last with the worst scores, so
        // it is neither top-k nor the would-be winner.
        let mut pool = Vec::new();
        for i in 0..10u64 {
            let mut c = candidate_with_provenance(
                i + 1,
                &spec,
                IRNode::Literal(crate::synthesis::engine::IRLiteral::Int(0)),
                1.0,
                Provenance::Mutation { parent_id: 0, mutation_kind: "test".into() },
            );
            c.scores.quality_score = 1.0 - (i as f64) * 0.05;
            c.score = composite_score(&c.scores);
            pool.push(c);
        }
        pool[9] = candidate_with_provenance(
            10,
            &spec,
            scan_body(),
            0.0,
            Provenance::ConstraintInduced { kind: "forall_fold".into() },
        );
        let winner = selector::select_best(pool.clone(), &spec).unwrap();
        assert_ne!(winner.id, 10, "setup: the seed must not be the would-be winner");
        let chosen = choose_finalists(&pool, &spec, &[9]);
        assert_eq!(chosen.len(), RUNNER_FINALISTS);
        assert!(chosen.contains(&9), "the triggering seed must be a finalist");
        assert!(
            chosen.iter().any(|&i| pool[i].id == winner.id),
            "the would-be winner keeps its guaranteed slot"
        );
        // Without the guarantee the seed is not chosen.
        let plain = choose_finalists(&pool, &spec, &[]);
        assert!(!plain.contains(&9));
    }

    /// Phase 61 guarantee extension: inside an
    /// UnbindableTests stage the guarantee set is exactly
    /// the pool's Template-provenance indices; inside the
    /// Phase 60 arm it remains exactly the triggering
    /// seeds (a template there is NOT added — the two arms'
    /// scopes are distinct, as recorded).
    #[test]
    fn guaranteed_finalists_scopes_per_trigger_arm() {
        let spec = spec_from(MINI_SORT);
        let template = candidate_with_provenance(
            1,
            &spec,
            IRNode::Var("list".into()),
            1.0,
            Provenance::Template { template_name: "sort_primitive".into() },
        );
        let mutation = candidate_with_provenance(
            2,
            &spec,
            IRNode::Var("list".into()),
            1.0,
            Provenance::Mutation { parent_id: 0, mutation_kind: "test".into() },
        );
        let seed = candidate_with_provenance(
            3,
            &spec,
            IRNode::Var("list".into()),
            1.0,
            Provenance::ConstraintInduced { kind: "equation".into() },
        );
        let pool = vec![mutation, template, seed];
        assert_eq!(
            guaranteed_finalists(&pool, &Trigger::UnbindableTests),
            vec![1],
            "the template's pool index, and only it"
        );
        assert_eq!(
            guaranteed_finalists(&pool, &Trigger::ConstructedSeeds(vec![2])),
            vec![2],
            "the Phase 60 arm's set is its triggering seeds, unchanged"
        );
        // And the composed rule: a bottom-ranked template in
        // an unbindable-tests pool becomes a finalist.
        let mut big: Vec<CandidateImplementation> = Vec::new();
        for i in 0..10u64 {
            let mut c = candidate_with_provenance(
                i + 1,
                &spec,
                IRNode::Var("list".into()),
                1.0,
                Provenance::Mutation { parent_id: 0, mutation_kind: "test".into() },
            );
            c.scores.quality_score = 1.0 - (i as f64) * 0.05;
            c.score = composite_score(&c.scores);
            big.push(c);
        }
        big[9] = candidate_with_provenance(
            10,
            &spec,
            IRNode::Var("list".into()),
            1.0,
            Provenance::Template { template_name: "sort_primitive".into() },
        );
        let guaranteed = guaranteed_finalists(&big, &Trigger::UnbindableTests);
        assert_eq!(guaranteed, vec![9]);
        let chosen = choose_finalists(&big, &spec, &guaranteed);
        assert_eq!(chosen.len(), RUNNER_FINALISTS);
        assert!(chosen.contains(&9), "the template must be a finalist under the Phase 61 guarantee");
    }

    /// Phase 61 (G6): generate's shipped test module now
    /// annotates E0283-class calls itself; the runner's
    /// re-read of such a call must use it as-is (the stage
    /// must not degrade on the new shape, and must never
    /// annotate twice).
    #[test]
    fn runner_source_keeps_shipped_turbofish() {
        let spec = spec_from(MINI_SORT);
        let three = GENERATED_SORT_SHAPE.replace(
            "    #[test]\n    fn reverse() {",
            "    #[test]\n    fn single() {\n        let result = mini_sort(vec![5], |a, b| (a < b));\n        assert_eq!(result, vec![5]);\n    }\n    #[test]\n    fn reverse() {",
        );
        let annotated = three.replace(
            "mini_sort(vec![], |a, b| (a < b))",
            "mini_sort::<i64>(vec![], |a, b| (a < b))",
        );
        assert_ne!(annotated, three, "setup: the shipped annotation must be present");
        let source = build_runner_source(&annotated, &spec).expect("runner source");
        assert!(source.contains("mini_sort::<i64>(vec![], |a, b| (a < b))"));
        assert!(!source.contains("::<i64>::<"), "never annotated twice: {}", source);
        assert_eq!(
            source.matches("mini_sort::<i64>(vec![], |a, b| (a < b))").count(),
            1,
            "the shipped call is used exactly as-is, once"
        );
    }

    /// End-to-end in miniature (ignored: drives real rustc):
    /// the scan seed sits at interpreter tpr 0.0 and the
    /// constant body at a measured 1/3 before the stage;
    /// after it, the seed's recorded ratio is the runner's
    /// 3/3 and selection crowns the seed.
    #[test]
    #[ignore = "Phase 60 end-to-end: compiles count-shaped finalists with rustc; run explicitly with --ignored runner_scores_forall_fold_seed"]
    fn runner_scores_forall_fold_seed_and_crowns_it() {
        let spec = spec_from(COUNT_SHAPED);
        let seed = candidate_with_provenance(
            1,
            &spec,
            scan_body(),
            0.0,
            Provenance::ConstraintInduced { kind: "forall_fold".into() },
        );
        let constant = candidate_with_provenance(
            2,
            &spec,
            IRNode::Literal(crate::synthesis::engine::IRLiteral::Int(0)),
            1.0 / 3.0,
            Provenance::Mutation { parent_id: 0, mutation_kind: "test".into() },
        );
        let (pool, outcome) = rescore_finalists_report(vec![seed, constant], &spec);
        match outcome {
            RunnerOutcome::Scored { finalists, .. } => {
                assert_eq!(finalists.len(), 2, "both candidates are finalists");
            }
            other => panic!("expected Scored, got {:?}", other),
        }
        let by_id: HashMap<u64, f64> = pool
            .iter()
            .map(|c| (c.id, c.scores.test_pass_ratio))
            .collect();
        assert!((by_id[&1] - 1.0).abs() < 1e-9, "seed tpr {}", by_id[&1]);
        assert!((by_id[&2] - 1.0 / 3.0).abs() < 1e-9, "constant tpr {}", by_id[&2]);
        let winner = selector::select_best(pool, &spec).unwrap();
        assert_eq!(winner.id, 1, "the forall-fold seed must crown");
    }

    #[test]
    fn choose_finalists_takes_top_k_and_always_the_would_be_winner() {
        let spec = spec_from(MINI_SORT);
        // Ten hole-free candidates; composite strictly
        // decreasing in id via the quality tie-breaker, all at
        // FALLBACK tpr 1.0 — select_best's criteria chain
        // crowns by those same tie-breakers, so the would-be
        // winner is in the top-k here by construction; the
        // assertion pins the contract (size + membership).
        let mut pool = Vec::new();
        for i in 0..10u64 {
            let mut c = candidate(i + 1, &spec, IRNode::Var("list".into()), 1.0, 0.0);
            c.scores.quality_score = 1.0 - (i as f64) * 0.05;
            c.score = composite_score(&c.scores);
            pool.push(c);
        }
        let chosen = choose_finalists(&pool, &spec, &[]);
        assert_eq!(chosen.len(), RUNNER_FINALISTS);
        let winner = selector::select_best(pool.clone(), &spec).unwrap();
        assert!(
            chosen.iter().any(|&i| pool[i].id == winner.id),
            "the would-be winner must be a finalist"
        );
        // A pool smaller than k: everyone is a finalist.
        let small = pool[..3].to_vec();
        assert_eq!(choose_finalists(&small, &spec, &[]).len(), 3);
    }

    #[test]
    fn apply_runner_scores_replaces_fallback_and_caps_non_finalists() {
        let spec = spec_from(MINI_SORT);
        let holed = IRNode::Block(vec![
            IRNode::Return(Box::new(IRNode::Var("list".into()))),
            IRNode::Hole {
                id: 7,
                expected_type: None,
            },
        ]);
        let pool = vec![
            // finalist, hole-free, runner 3/6 → 0.5
            candidate(1, &spec, IRNode::Var("list".into()), 1.0, 0.0),
            // finalist, one hole, runner 6/6 → 1.0 × 0.5 = 0.5
            candidate(2, &spec, holed, 1.0, 0.0),
            // non-finalist at FALLBACK 1.0 → capped at the
            // worst finalist's 0.5
            candidate(3, &spec, IRNode::Var("list".into()), 1.0, 0.0),
            // non-finalist already below the cap → untouched
            candidate(4, &spec, IRNode::Var("list".into()), 0.25, 0.0),
        ];
        let (out, outcomes, capped) =
            apply_runner_scores(pool, &[0, 1], &[(0, 3, 6), (1, 6, 6)]);
        assert_eq!(tprs(&out), vec![0.5, 0.5, 0.5, 0.25]);
        assert_eq!(capped, 1);
        assert_eq!(outcomes.len(), 2);
        assert_eq!(outcomes[0].candidate_id, 1);
        assert_eq!(outcomes[1].new_test_pass_ratio, 0.5);
        for c in &out {
            assert_eq!(c.score, composite_score(&c.scores));
        }
    }

    const GENERATED_SORT_SHAPE: &str = r#"pub fn mini_sort<T>(list: Vec<T>, cmp: fn(T, T) -> bool) -> Vec<T> {
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let result = mini_sort(vec![], |a, b| (a < b));
        assert_eq!(result, vec![]);
    }
    #[test]
    fn reverse() {
        let result = mini_sort(vec![2, 1], |a, b| (a < b));
        assert_eq!(result, vec![1, 2]);
    }
}
"#;

    #[test]
    fn extraction_rereads_generated_invocations() {
        let invocations = extract_invocations(GENERATED_SORT_SHAPE).unwrap();
        assert_eq!(
            invocations,
            vec![
                (
                    "mini_sort(vec![], |a, b| (a < b))".to_string(),
                    "vec![]".to_string()
                ),
                (
                    "mini_sort(vec![2, 1], |a, b| (a < b))".to_string(),
                    "vec![1, 2]".to_string()
                ),
            ]
        );
    }

    #[test]
    fn runner_source_annotates_and_counts() {
        // The mini spec has 3 tests; the generated shape above
        // carries 2 — a count mismatch must refuse (None),
        // never mis-score.
        let spec = spec_from(MINI_SORT);
        assert!(build_runner_source(GENERATED_SORT_SHAPE, &spec).is_none());
        // With a matching 3-test shape the source builds and
        // the calls carry the turbofish.
        let three = GENERATED_SORT_SHAPE.replace(
            "    #[test]\n    fn reverse() {",
            "    #[test]\n    fn single() {\n        let result = mini_sort(vec![5], |a, b| (a < b));\n        assert_eq!(result, vec![5]);\n    }\n    #[test]\n    fn reverse() {",
        );
        let source = build_runner_source(&three, &spec).expect("runner source");
        assert!(source.contains("mini_sort::<i64>(vec![], |a, b| (a < b))"));
        assert!(source.contains("TOTAL {}/3"));
        assert_eq!(concrete_type_for_spec(&spec), "i64");
    }

    /// End-to-end in miniature — RE-BASED at Phase 131
    /// (G4); the compiled premise is retired, the question
    /// it asked is pinned where it now lives. At Phase 58
    /// this instrument (ignored: drove real rustc) showed
    /// the identity body and the constant-empty body
    /// sitting at FALLBACK tpr 1.0 — mini_sort's tests
    /// carry lambdas, which the interpreter could not bind
    /// — and the compiled stage replacing the fallback
    /// with measured ratios (2/3, 1/3), selection crowning
    /// the identity. Phase 130 (function values) closed
    /// that premise: the tests bind, the interpreter
    /// measures the same two ratios itself, and the stage
    /// does not fire. The remaining unbindable class —
    /// nested-tuple test inputs (the Phase 130
    /// UNBINDABLE_TESTS fixture) — is one the compiled
    /// harness cannot render either (the Phase 53
    /// blocker-2 record: tuple inputs generate as
    /// `__tuple__(...)`), so no spec remains in the
    /// UnbindableTests arm's joint domain; the arm stands
    /// for whatever future input form lands there. The
    /// stage's compiled end-to-end coverage lives on in
    /// `runner_scores_forall_fold_seed_and_crowns_it`
    /// (the ConstructedSeeds arm). What this test pins
    /// now, in-process: the spec binds 3/3; the two bodies
    /// measure 2/3 and 1/3 under the interpreter; the
    /// stage returns the pool untouched; selection crowns
    /// the identity on the measured ratios.
    #[test]
    fn runner_scoring_end_to_end() {
        let spec = spec_from(MINI_SORT);
        // The premise, flipped: the lambda tests bind.
        assert_eq!(crate::synthesis::eval::bindable_test_count(&spec), 3);
        let identity_body = IRNode::Var("list".into());
        let empty_body = IRNode::Collection {
            kind: CollectionKind::List,
            elements: vec![],
        };
        // The interpreter measures what the compiled stage
        // used to: identity 2/3, constant-empty 1/3.
        let id_run = crate::synthesis::eval::run_spec_tests(&identity_body, &spec);
        assert_eq!((id_run.evaluable, id_run.passed), (3, 2));
        let empty_run = crate::synthesis::eval::run_spec_tests(&empty_body, &spec);
        assert_eq!((empty_run.evaluable, empty_run.passed), (3, 1));
        let identity = candidate(1, &spec, identity_body, 2.0 / 3.0, 0.0);
        let empty = candidate(2, &spec, empty_body, 1.0 / 3.0, 0.0);
        let (pool, outcome) = rescore_finalists_report(vec![identity, empty], &spec);
        assert!(
            matches!(outcome, RunnerOutcome::NotTriggered),
            "the stage does not fire for a spec the interpreter judges: {outcome:?}"
        );
        assert_eq!(tprs(&pool), vec![2.0 / 3.0, 1.0 / 3.0]);
        let winner = selector::select_best(pool, &spec).unwrap();
        assert_eq!(winner.id, 1);
    }
}
