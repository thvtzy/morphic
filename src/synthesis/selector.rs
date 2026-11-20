// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC SELECTOR                                        │
// │  Chooses the optimal implementation from candidates      │
// └──────────────────────────────────────────────────────────┘

use std::collections::HashMap;
use super::engine::{count_holes, CandidateImplementation, ScoreBreakdown};
use crate::spec::ast::FunctionSpec;

/// Selection criteria for choosing among candidates
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionCriterion {
    /// Highest overall composite score (default)
    BestOverall,
    /// Fastest (best time complexity)
    Fastest,
    /// Best verified correctness
    MostCorrect,
    /// Most readable / maintainable
    MostReadable,
    /// Pareto-optimal (best tradeoff)
    ParetoOptimal,
    /// Smallest code size
    MostCompact,
}

/// Select the single best implementation from a list
pub fn select_best(
    candidates: Vec<CandidateImplementation>,
    _spec: &FunctionSpec,
) -> Result<CandidateImplementation, SelectionError> {
    if candidates.is_empty() {
        return Err(SelectionError::NoCandidates);
    }

    // Completeness gate: a candidate whose body still contains
    // unfilled Holes is not a finished implementation, whatever its
    // scores say (test/constraint credit for holed bodies is partly
    // placeholder — see the hole penalty in engine::evaluate_candidate).
    // If any hole-free candidate exists, holed candidates are
    // excluded from winning outright. Only when every candidate is
    // holed does selection fall back to ranking the holed pool, so
    // callers still get the least-incomplete answer available.
    let hole_free: Vec<CandidateImplementation> = candidates
        .iter()
        .filter(|c| count_holes(&c.body) == 0)
        .cloned()
        .collect();
    let pool: &[CandidateImplementation] =
        if hole_free.is_empty() { &candidates } else { &hole_free };

    // Apply multiple criteria in priority order
    let best = select_by_criteria(pool, &[
        SelectionCriterion::MostCorrect,
        SelectionCriterion::ParetoOptimal,
        SelectionCriterion::BestOverall,
        SelectionCriterion::MostReadable,
    ])?;

    Ok(best.clone())
}

/// Select top N implementations
pub fn select_top_n(
    candidates: &[CandidateImplementation],
    n: usize,
    criterion: SelectionCriterion,
) -> Vec<CandidateImplementation> {
    let mut sorted = candidates.to_vec();
    sort_by_criterion(&mut sorted, criterion);
    sorted.truncate(n);
    sorted
}

/// Find Pareto-optimal frontier
pub fn pareto_frontier(
    candidates: &[CandidateImplementation],
) -> Vec<CandidateImplementation> {
    let mut frontier = Vec::new();

    for c in candidates {
        // A candidate is Pareto-optimal if no other candidate
        // is better in ALL dimensions
        let dominated = candidates.iter().any(|other| {
            dominates(other, c)
        });

        if !dominated {
            frontier.push(c.clone());
        }
    }

    frontier
}

fn dominates(a: &CandidateImplementation, b: &CandidateImplementation) -> bool {
    let sa = &a.scores;
    let sb = &b.scores;

    // a dominates b if a is at least as good in all dimensions
    // and strictly better in at least one
    let at_least_as_good =
        sa.test_pass_ratio >= sb.test_pass_ratio
        && sa.constraint_score >= sb.constraint_score
        && sa.complexity_score >= sb.complexity_score
        && sa.quality_score >= sb.quality_score;

    let strictly_better =
        sa.test_pass_ratio > sb.test_pass_ratio
        || sa.constraint_score > sb.constraint_score
        || sa.complexity_score > sb.complexity_score
        || sa.quality_score > sb.quality_score;

    at_least_as_good && strictly_better
}

fn select_by_criteria<'a>(
    candidates: &'a [CandidateImplementation],
    criteria: &[SelectionCriterion],
) -> Result<&'a CandidateImplementation, SelectionError> {
    let mut pool: Vec<&CandidateImplementation> = candidates.iter().collect();

    for criterion in criteria {
        if pool.len() <= 1 {
            break;
        }
        pool = select_by_single(&pool, *criterion);
    }

    pool.first()
        .cloned()
        .ok_or(SelectionError::NoCandidates)
}

fn select_by_single<'a>(
    candidates: &[&'a CandidateImplementation],
    criterion: SelectionCriterion,
) -> Vec<&'a CandidateImplementation> {
    match criterion {
        SelectionCriterion::BestOverall => {
            let max_score = candidates.iter()
                .map(|c| c.score)
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(0.0);
            candidates.iter()
                .filter(|c| c.score >= max_score)
                .copied()
                .collect()
        }

        SelectionCriterion::Fastest => {
            let best_complexity = candidates.iter()
                .map(|c| c.scores.complexity_score)
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(0.0);
            candidates.iter()
                .filter(|c| c.scores.complexity_score >= best_complexity)
                .copied()
                .collect()
        }

        SelectionCriterion::MostCorrect => {
            let best_correctness = candidates.iter()
                .map(|c| {
                    c.scores.test_pass_ratio * 0.6 + c.scores.constraint_score * 0.4
                })
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(0.0);
            let threshold = best_correctness - 0.01; // ε-tolerance
            candidates.iter()
                .filter(|c| {
                    (c.scores.test_pass_ratio * 0.6 + c.scores.constraint_score * 0.4) >= threshold
                })
                .copied()
                .collect()
        }

        SelectionCriterion::MostReadable => {
            let best_quality = candidates.iter()
                .map(|c| c.scores.quality_score)
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(0.0);
            candidates.iter()
                .filter(|c| c.scores.quality_score >= best_quality)
                .copied()
                .collect()
        }

        SelectionCriterion::ParetoOptimal => {
            // Find all non-dominated candidates
            let mut pareto = Vec::new();
            for (i, c) in candidates.iter().enumerate() {
                let is_dominated = candidates.iter().enumerate().any(|(j, other)| {
                    i != j && dominates(other, c)
                });
                if !is_dominated {
                    pareto.push(*c);
                }
            }
            pareto
        }

        SelectionCriterion::MostCompact => {
            // Compactness is inverse of AST depth (approximated by quality)
            let most_compact = candidates.iter()
                .map(|c| c.scores.quality_score)
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(0.0);
            candidates.iter()
                .filter(|c| c.scores.quality_score >= most_compact)
                .copied()
                .collect()
        }
    }
}

fn sort_by_criterion(
    candidates: &mut [CandidateImplementation],
    criterion: SelectionCriterion,
) {
    match criterion {
        SelectionCriterion::BestOverall => {
            candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        }
        SelectionCriterion::Fastest => {
            candidates.sort_by(|a, b| {
                b.scores.complexity_score
                    .partial_cmp(&a.scores.complexity_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        SelectionCriterion::MostCorrect => {
            candidates.sort_by(|a, b| {
                let ca = a.scores.test_pass_ratio * 0.6 + a.scores.constraint_score * 0.4;
                let cb = b.scores.test_pass_ratio * 0.6 + b.scores.constraint_score * 0.4;
                cb.partial_cmp(&ca).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        SelectionCriterion::MostReadable => {
            candidates.sort_by(|a, b| {
                b.scores.quality_score
                    .partial_cmp(&a.scores.quality_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        SelectionCriterion::ParetoOptimal => {
            // Dominance count: fewer dominators = better
            let dominance: HashMap<u64, usize> = candidates
                .iter()
                .map(|a| {
                    let count = candidates.iter()
                        .filter(|o| dominates(o, a))
                        .count();
                    (a.id, count)
                })
                .collect();
            candidates.sort_by(|a, b| {
                let da = dominance.get(&a.id).unwrap_or(&0);
                let db = dominance.get(&b.id).unwrap_or(&0);
                da.cmp(db)
            });
        }
        SelectionCriterion::MostCompact => {
            candidates.sort_by(|a, b| {
                b.scores.quality_score
                    .partial_cmp(&a.scores.quality_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }
}

#[derive(Debug)]
pub enum SelectionError {
    NoCandidates,
}

impl std::fmt::Display for SelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SelectionError::NoCandidates => write!(f, "No candidates to select from"),
        }
    }
}

impl std::error::Error for SelectionError {}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::ast::*;
    use crate::synthesis::engine::{IRNode, IRLiteral, Provenance};
    use std::collections::HashMap;

    fn make_candidate(id: u64, score: f64, test_pass: f64, quality: f64) -> CandidateImplementation {
        CandidateImplementation {
            id,
            body: IRNode::Literal(IRLiteral::Unit),
            spec_name: "test".into(),
            spec: FunctionSpec {
                name: "test".into(),
                doc: None,
                generics: vec![],
                params: vec![],
                return_type: TypeRef::Unit,
                preconditions: vec![],
                postconditions: vec![],
                invariants: vec![],
                complexity: None,
                resource: None,
                tests: vec![],
                annotations: HashMap::new(),
            },
            score,
            scores: ScoreBreakdown {
                test_pass_ratio: test_pass,
                constraint_score: 0.8,
                complexity_score: 0.7,
                quality_score: quality,
                synthesis_time_us: 0,
            },
            generation: 0,
            provenance: Provenance::Template { template_name: "test".into() },
            verified: false,
        }
    }

    #[test]
    fn select_best_overall() {
        let candidates = vec![
            make_candidate(1, 0.5, 0.5, 0.5),
            make_candidate(2, 0.9, 0.9, 0.9),
            make_candidate(3, 0.3, 0.3, 0.3),
        ];
        let spec = candidates[0].spec.clone();
        let best = select_best(candidates, &spec).unwrap();
        assert_eq!(best.id, 2);
    }

    fn with_body(mut c: CandidateImplementation, body: IRNode) -> CandidateImplementation {
        c.body = body;
        c
    }

    fn holed_body() -> IRNode {
        IRNode::Block(vec![
            IRNode::Return(Box::new(IRNode::Literal(IRLiteral::Int(1)))),
            IRNode::Hole { id: 99, expected_type: None },
        ])
    }

    #[test]
    fn select_best_prefers_hole_free_at_equal_scores() {
        // Identical scores, holed candidate listed first — only the
        // completeness gate can explain the hole-free one winning.
        let holed = with_body(make_candidate(1, 0.9, 0.9, 0.9), holed_body());
        let complete = make_candidate(2, 0.9, 0.9, 0.9);
        let spec = complete.spec.clone();
        let best = select_best(vec![holed, complete], &spec).unwrap();
        assert_eq!(best.id, 2);
    }

    #[test]
    fn select_best_gate_beats_higher_scored_holed() {
        // Even a maximally scored holed candidate must not win
        // while any hole-free candidate exists.
        let holed = with_body(make_candidate(1, 1.0, 1.0, 1.0), holed_body());
        let complete = make_candidate(2, 0.5, 0.5, 0.5);
        let spec = complete.spec.clone();
        let best = select_best(vec![holed, complete], &spec).unwrap();
        assert_eq!(best.id, 2);
    }

    #[test]
    fn select_best_still_ranks_pool_when_all_holed() {
        // No hole-free candidate: selection falls back to ranking
        // the holed pool instead of failing.
        let weak = with_body(make_candidate(1, 0.4, 0.4, 0.4), holed_body());
        let strong = with_body(make_candidate(2, 0.9, 0.9, 0.9), holed_body());
        let spec = strong.spec.clone();
        let best = select_best(vec![weak, strong], &spec).unwrap();
        assert_eq!(best.id, 2);
    }

    #[test]
    fn select_best_crowns_measured_truth_over_shape() {
        // A plain candidate that passes every measured test must
        // beat a "fancy" one that passes none — even when the
        // fancy one maxes constraint/complexity/quality and the
        // plain one gets nothing from them. Candidate scores are
        // stamped with the engine's composite, exactly as
        // score_and_rank leaves them in the real pipeline.
        use crate::synthesis::engine::composite_score;
        let breakdown = |tpr: f64, constraint: f64, complexity: f64, quality: f64| {
            ScoreBreakdown {
                test_pass_ratio: tpr,
                constraint_score: constraint,
                complexity_score: complexity,
                quality_score: quality,
                synthesis_time_us: 0,
            }
        };
        let mut truthful = make_candidate(1, 0.0, 1.0, 0.0);
        truthful.scores = breakdown(1.0, 0.0, 0.0, 0.0);
        truthful.score = composite_score(&truthful.scores);
        let mut fancy = make_candidate(2, 0.0, 0.0, 1.0);
        fancy.scores = breakdown(0.0, 1.0, 1.0, 1.0);
        fancy.score = composite_score(&fancy.scores);
        let spec = truthful.spec.clone();
        let best = select_best(vec![fancy, truthful], &spec).unwrap();
        assert_eq!(best.id, 1);
    }

    // ── Return-type verdict + gate (Phase 11) ──────────────
    //
    // The two test-less M2 specs (tags_compatible, needs_parens)
    // have no TestInduced seed and no truth signal — tpr is the
    // neutral 0.5 for every candidate — so selection used to
    // crown arbitrary random trees whose WHOLE bodies had the
    // wrong type vs the spec's return type (`(actual + expected)`
    // as a Bool body; a `for` loop as a Bool body): E0308 on
    // every probe run. The verdict below is CONSERVATIVE: it
    // only calls a body Mismatched when the mismatch is provable
    // from the IR alone; everything unprovable is Unknown and
    // stays neutral in selection.

    use crate::synthesis::engine::{IRBinOp, IRPattern};

    fn spec_with(
        params: Vec<(&str, TypeRef)>,
        return_type: TypeRef,
    ) -> FunctionSpec {
        FunctionSpec {
            name: "gated".into(),
            doc: None,
            generics: vec![],
            params: params
                .into_iter()
                .map(|(name, ty)| Param {
                    name: name.into(),
                    ty,
                    annotations: HashMap::new(),
                })
                .collect(),
            return_type,
            preconditions: vec![],
            postconditions: vec![],
            invariants: vec![],
            complexity: None,
            resource: None,
            tests: vec![],
            annotations: HashMap::new(),
        }
    }

    fn breakdown(tpr: f64, constraint: f64, complexity: f64, quality: f64) -> ScoreBreakdown {
        ScoreBreakdown {
            test_pass_ratio: tpr,
            constraint_score: constraint,
            complexity_score: complexity,
            quality_score: quality,
            synthesis_time_us: 0,
        }
    }

    fn candidate_for(
        spec: &FunctionSpec,
        id: u64,
        body: IRNode,
        scores: ScoreBreakdown,
    ) -> CandidateImplementation {
        use crate::synthesis::engine::composite_score;
        CandidateImplementation {
            id,
            body,
            spec_name: spec.name.clone(),
            spec: spec.clone(),
            score: composite_score(&scores),
            scores,
            generation: 0,
            provenance: Provenance::Mutation {
                parent_id: 0,
                mutation_kind: "test".into(),
            },
            verified: false,
        }
    }

    fn var(name: &str) -> IRNode {
        IRNode::Var(name.into())
    }

    fn binop(op: IRBinOp, l: IRNode, r: IRNode) -> IRNode {
        IRNode::BinOp(op, Box::new(l), Box::new(r))
    }

    #[test]
    fn verdict_literals_and_vars() {
        let spec = spec_with(vec![("x", TypeRef::Int)], TypeRef::Bool);
        assert_eq!(
            return_type_verdict(&IRNode::Literal(IRLiteral::Bool(true)), &spec),
            TypeVerdict::Matches
        );
        assert_eq!(
            return_type_verdict(&IRNode::Literal(IRLiteral::Int(1)), &spec),
            TypeVerdict::Mismatched
        );
        // A param reference has the param's declared type — certain.
        assert_eq!(
            return_type_verdict(&var("x"), &spec),
            TypeVerdict::Mismatched
        );
        // An unbound name proves nothing.
        assert_eq!(
            return_type_verdict(&var("nope"), &spec),
            TypeVerdict::Unknown
        );
    }

    #[test]
    fn verdict_binop_rules() {
        let spec = spec_with(
            vec![("a", TypeRef::Int), ("b", TypeRef::Int)],
            TypeRef::Bool,
        );
        // Comparison of two certain Int operands is certainly Bool.
        assert_eq!(
            return_type_verdict(&binop(IRBinOp::Eq, var("a"), var("b")), &spec),
            TypeVerdict::Matches
        );
        // Arithmetic on Ints is certainly Int — the tags_compatible
        // failing winner was exactly this shape in a Bool spec.
        assert_eq!(
            return_type_verdict(&binop(IRBinOp::Add, var("a"), var("b")), &spec),
            TypeVerdict::Mismatched
        );
        // A comparison whose operands provably disagree is
        // internally ill-typed; its result type is not claimed.
        let bad = binop(
            IRBinOp::Lt,
            var("a"),
            IRNode::Literal(IRLiteral::String("s".into())),
        );
        assert_eq!(return_type_verdict(&bad, &spec), TypeVerdict::Unknown);
    }

    #[test]
    fn verdict_block_tail_and_return_scan() {
        let spec = spec_with(vec![], TypeRef::Bool);
        // Block value = tail expression's type.
        let body = IRNode::Block(vec![
            IRNode::Literal(IRLiteral::Int(0)),
            IRNode::Literal(IRLiteral::Bool(true)),
        ]);
        assert_eq!(return_type_verdict(&body, &spec), TypeVerdict::Matches);
        // A wrong-typed Return ANYWHERE is a provable mismatch,
        // even when the tail expression agrees with the spec.
        let body = IRNode::Block(vec![
            IRNode::Literal(IRLiteral::Bool(true)),
            IRNode::Return(Box::new(IRNode::Literal(IRLiteral::Int(1)))),
        ]);
        assert_eq!(return_type_verdict(&body, &spec), TypeVerdict::Mismatched);
        // An early Return makes the tail unreachable: the verdict
        // follows the Return, not the dead tail.
        let body = IRNode::Block(vec![
            IRNode::Return(Box::new(IRNode::Literal(IRLiteral::Int(1)))),
            IRNode::Literal(IRLiteral::Bool(true)),
        ]);
        assert_eq!(return_type_verdict(&body, &spec), TypeVerdict::Mismatched);
        // A right-typed early Return with a dead wrong tail: the
        // Return checks out and the tail proves nothing.
        let body = IRNode::Block(vec![
            IRNode::Return(Box::new(IRNode::Literal(IRLiteral::Bool(false)))),
            IRNode::Literal(IRLiteral::Int(1)),
        ]);
        assert_eq!(return_type_verdict(&body, &spec), TypeVerdict::Unknown);
        // Empty block is Unit: wrong for Bool, right for Unit.
        assert_eq!(
            return_type_verdict(&IRNode::Block(vec![]), &spec),
            TypeVerdict::Mismatched
        );
        let unit_spec = spec_with(vec![], TypeRef::Unit);
        assert_eq!(
            return_type_verdict(&IRNode::Block(vec![]), &unit_spec),
            TypeVerdict::Matches
        );
    }

    #[test]
    fn verdict_branches_must_agree() {
        let spec = spec_with(vec![("kind", TypeRef::Int)], TypeRef::String);
        let lit_str = |s: &str| IRNode::Literal(IRLiteral::String(s.into()));
        let if_ok = IRNode::If {
            condition: Box::new(IRNode::Literal(IRLiteral::Bool(true))),
            then_branch: Box::new(lit_str("a")),
            else_branch: Box::new(lit_str("b")),
        };
        assert_eq!(return_type_verdict(&if_ok, &spec), TypeVerdict::Matches);
        let if_split = IRNode::If {
            condition: Box::new(IRNode::Literal(IRLiteral::Bool(true))),
            then_branch: Box::new(lit_str("a")),
            else_branch: Box::new(IRNode::Literal(IRLiteral::Int(1))),
        };
        assert_eq!(return_type_verdict(&if_split, &spec), TypeVerdict::Unknown);
        let match_ok = IRNode::Match {
            scrutinee: Box::new(var("kind")),
            cases: vec![
                (IRPattern::Literal(IRLiteral::Int(0)), lit_str("x")),
                (IRPattern::Wildcard, lit_str("y")),
            ],
        };
        assert_eq!(return_type_verdict(&match_ok, &spec), TypeVerdict::Matches);
        let match_split = IRNode::Match {
            scrutinee: Box::new(var("kind")),
            cases: vec![
                (IRPattern::Literal(IRLiteral::Int(0)), lit_str("x")),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::Int(1)),
                ),
            ],
        };
        assert_eq!(
            return_type_verdict(&match_split, &spec),
            TypeVerdict::Unknown
        );
    }

    #[test]
    fn verdict_collections_and_len_call() {
        let list_spec = spec_with(
            vec![("xs", TypeRef::List(Box::new(TypeRef::Int)))],
            TypeRef::List(Box::new(TypeRef::Int)),
        );
        let coll = |elements: Vec<IRNode>| IRNode::Collection {
            kind: crate::synthesis::engine::CollectionKind::List,
            elements,
        };
        assert_eq!(
            return_type_verdict(
                &coll(vec![
                    IRNode::Literal(IRLiteral::Int(1)),
                    IRNode::Literal(IRLiteral::Int(2)),
                ]),
                &list_spec
            ),
            TypeVerdict::Matches
        );
        // An empty list's element type is unconstrained: it could
        // be the spec's List(Int) — provable neither way.
        assert_eq!(
            return_type_verdict(&coll(vec![]), &list_spec),
            TypeVerdict::Unknown
        );
        // …but a list is provably never a Bool.
        let bool_spec = spec_with(vec![], TypeRef::Bool);
        assert_eq!(
            return_type_verdict(&coll(vec![]), &bool_spec),
            TypeVerdict::Mismatched
        );
        // len() is a known pure builtin returning Int.
        let int_spec = spec_with(vec![("s", TypeRef::String)], TypeRef::Int);
        let len_call = IRNode::Call {
            function: "len".into(),
            args: vec![var("s")],
        };
        assert_eq!(
            return_type_verdict(&len_call, &int_spec),
            TypeVerdict::Matches
        );
        assert_eq!(
            return_type_verdict(&len_call, &bool_spec),
            TypeVerdict::Mismatched
        );
        // Unknown functions prove nothing.
        let mystery = IRNode::Call {
            function: "frobnicate".into(),
            args: vec![var("s")],
        };
        assert_eq!(
            return_type_verdict(&mystery, &int_spec),
            TypeVerdict::Unknown
        );
    }

    #[test]
    fn verdict_unproven_shapes_are_unknown() {
        let spec = spec_with(vec![("n", TypeRef::Int)], TypeRef::Bool);
        let for_body = IRNode::For {
            var: "i".into(),
            start: Box::new(IRNode::Literal(IRLiteral::Int(0))),
            end: Box::new(var("n")),
            body: Box::new(IRNode::Literal(IRLiteral::Bool(true))),
        };
        assert_eq!(
            return_type_verdict(&for_body, &spec),
            TypeVerdict::Unknown
        );
        assert_eq!(
            return_type_verdict(
                &IRNode::Hole { id: 1, expected_type: Some(TypeRef::Bool) },
                &spec
            ),
            TypeVerdict::Unknown
        );
        // A Let whose value type is certain binds certainly…
        let let_certain = IRNode::Let {
            name: "y".into(),
            value: Box::new(IRNode::Literal(IRLiteral::Int(3))),
            body: Box::new(var("y")),
        };
        assert_eq!(
            return_type_verdict(&let_certain, &spec),
            TypeVerdict::Mismatched
        );
        // …and one whose value type is unprovable binds unknown.
        let let_unknown = IRNode::Let {
            name: "y".into(),
            value: Box::new(var("ghost")),
            body: Box::new(var("y")),
        };
        assert_eq!(
            return_type_verdict(&let_unknown, &spec),
            TypeVerdict::Unknown
        );
    }

    #[test]
    fn verdict_integer_family_unifies() {
        // Integer literals and fixed-width ints are one family:
        // a literal is accepted wherever an integer type stands.
        let i64_spec = spec_with(vec![], TypeRef::I64);
        assert_eq!(
            return_type_verdict(&IRNode::Literal(IRLiteral::Int(5)), &i64_spec),
            TypeVerdict::Matches
        );
        let int_spec = spec_with(vec![("v", TypeRef::I32)], TypeRef::Int);
        assert_eq!(
            return_type_verdict(&var("v"), &int_spec),
            TypeVerdict::Matches
        );
        // But integers never unify with Float.
        let float_spec = spec_with(vec![], TypeRef::Float);
        assert_eq!(
            return_type_verdict(&IRNode::Literal(IRLiteral::Int(5)), &float_spec),
            TypeVerdict::Mismatched
        );
    }

    #[test]
    fn select_best_excludes_provably_wrong_return_type() {
        // The tags_compatible failure shape: Bool spec over two
        // Int params, no tests. The high scorer's whole body is
        // Int-typed; before the gate it won on complexity/quality.
        let spec = spec_with(
            vec![("expected", TypeRef::Int), ("actual", TypeRef::Int)],
            TypeRef::Bool,
        );
        let wrong = candidate_for(
            &spec,
            1,
            binop(IRBinOp::Add, var("actual"), var("expected")),
            breakdown(0.5, 0.7, 1.0, 1.0),
        );
        let right = candidate_for(
            &spec,
            2,
            binop(IRBinOp::Eq, var("expected"), var("actual")),
            breakdown(0.5, 0.7, 0.0, 0.0),
        );
        assert!(
            wrong.score > right.score,
            "test setup: the mistyped candidate must outscore the typed one"
        );
        let best = select_best(vec![wrong, right], &spec).unwrap();
        assert_eq!(best.id, 2);
    }

    #[test]
    fn select_best_type_gate_falls_back_when_all_mismatched() {
        // Every candidate provably mistyped: the gate must not
        // empty the pool — old ranking decides, as documented.
        let spec = spec_with(vec![("a", TypeRef::Int)], TypeRef::Bool);
        let weak = candidate_for(
            &spec,
            1,
            binop(IRBinOp::Add, var("a"), var("a")),
            breakdown(0.5, 0.7, 0.0, 0.0),
        );
        let strong = candidate_for(
            &spec,
            2,
            binop(IRBinOp::Mul, var("a"), var("a")),
            breakdown(0.5, 0.7, 1.0, 1.0),
        );
        let best = select_best(vec![weak, strong], &spec).unwrap();
        assert_eq!(best.id, 2);
    }

    #[test]
    fn select_best_type_gate_keeps_unknown_candidates() {
        // Unknown is neutral: an unprovable body (a For loop)
        // must survive the gate and beat a provably mistyped one.
        let spec = spec_with(vec![("n", TypeRef::Int)], TypeRef::Bool);
        let wrong = candidate_for(
            &spec,
            1,
            binop(IRBinOp::Add, var("n"), var("n")),
            breakdown(0.5, 0.7, 1.0, 1.0),
        );
        let unknown = candidate_for(
            &spec,
            2,
            IRNode::For {
                var: "i".into(),
                start: Box::new(IRNode::Literal(IRLiteral::Int(0))),
                end: Box::new(var("n")),
                body: Box::new(IRNode::Literal(IRLiteral::Bool(true))),
            },
            breakdown(0.5, 0.7, 0.0, 0.0),
        );
        let best = select_best(vec![wrong, unknown], &spec).unwrap();
        assert_eq!(best.id, 2);
    }

    #[test]
    fn select_best_type_gate_preserves_measured_truth() {
        // Regression: on a TESTED spec, the tpr-1.0 TestInduced
        // shape (match over the input, return-typed arms — the
        // Phase 9/10 winner shape) must still win with the gate
        // active, over a rival that maxes every other dimension.
        let mut spec = spec_with(vec![("kind", TypeRef::Int)], TypeRef::Bool);
        spec.name = "is_literal_kind".into();
        spec.tests = vec![Test {
            name: None,
            input: Expr::IntLit(0),
            expected_output: Expr::BoolLit(true),
            timeout_ms: None,
            property: false,
        }];
        let seed_shape = IRNode::Match {
            scrutinee: Box::new(var("kind")),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Int(0)),
                    IRNode::Literal(IRLiteral::Bool(true)),
                ),
                (
                    IRPattern::Literal(IRLiteral::Int(2)),
                    IRNode::Literal(IRLiteral::Bool(false)),
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::Bool(true)),
                ),
            ],
        };
        let truthful = candidate_for(&spec, 1, seed_shape, breakdown(1.0, 0.0, 0.0, 0.0));
        let fancy = candidate_for(
            &spec,
            2,
            IRNode::Literal(IRLiteral::Int(0)),
            breakdown(0.0, 1.0, 1.0, 1.0),
        );
        let best = select_best(vec![fancy, truthful], &spec).unwrap();
        assert_eq!(best.id, 1);
    }

    #[test]
    fn synthesized_testless_spec_winner_is_not_provably_mistyped() {
        // Integration, mirroring the probe pipeline (synthesize →
        // verify_all → select_best) for the two test-less M2
        // specs. The gate's precise guarantee: WHENEVER the final
        // pool contains any candidate that is not provably
        // mistyped, the winner is not provably mistyped. Both
        // halves are asserted — the pool condition is what makes
        // the winner assertion meaningful rather than vacuous.
        use crate::synthesis::engine::synthesize;
        for (file, name) in [("validator", "tags_compatible"), ("printer", "needs_parens")] {
            let root = env!("CARGO_MANIFEST_DIR");
            let path = format!("{}/examples/self_host/{}.morph", root, file);
            let source = std::fs::read_to_string(&path).unwrap();
            let parsed = crate::spec::parser::parse(&source).unwrap();
            let checked = crate::spec::typeck::check(parsed).unwrap();
            let spec = checked
                .spec
                .functions
                .iter()
                .find(|f| f.name == name)
                .unwrap()
                .clone();
            assert!(spec.tests.is_empty(), "{} must be test-less", name);

            let candidates = synthesize(&spec, 1).expect("opt-1 synthesis");
            let pool = crate::verify::verifier::verify_all(candidates, &spec)
                .expect("verify_all");
            assert!(
                pool.iter()
                    .any(|c| return_type_verdict(&c.body, &spec) != TypeVerdict::Mismatched),
                "final pool for {} contains only provably mistyped candidates — \
                 the gate has nothing to crown",
                name
            );
            let winner = select_best(pool, &spec).expect("select_best");
            assert_ne!(
                return_type_verdict(&winner.body, &spec),
                TypeVerdict::Mismatched,
                "winner for {} is provably mistyped against {}",
                name,
                spec.return_type
            );
        }
    }

    #[test]
    fn pareto_frontier_filters_dominated() {
        let candidates = vec![
            make_candidate(1, 0.9, 0.95, 0.85), // Dominates 2
            make_candidate(2, 0.7, 0.80, 0.70),
            make_candidate(3, 0.5, 0.50, 0.98), // High quality, low correctness
        ];
        let frontier = pareto_frontier(&candidates);
        assert_eq!(frontier.len(), 2);
        assert!(frontier.iter().any(|c| c.id == 1));
        assert!(frontier.iter().any(|c| c.id == 3));
    }
}
