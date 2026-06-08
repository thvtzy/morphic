// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC SELECTOR                                        │
// │  Chooses the optimal implementation from candidates      │
// └──────────────────────────────────────────────────────────┘

use super::engine::{CandidateImplementation, ScoreBreakdown};
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

    // Apply multiple criteria in priority order
    let best = select_by_criteria(&candidates, &[
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

fn select_by_criteria(
    candidates: &[CandidateImplementation],
    criteria: &[SelectionCriterion],
) -> Result<&CandidateImplementation, SelectionError> {
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
            candidates.sort_by(|a, b| {
                let da = candidates.iter().filter(|o| dominates(o, a)).count();
                let db = candidates.iter().filter(|o| dominates(o, b)).count();
                da.cmp(&db)
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
