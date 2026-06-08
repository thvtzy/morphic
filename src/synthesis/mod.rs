// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC SYNTHESIS MODULE                                │
// │  Hybrid MCTS + Z3 + LLM synthesis engine                 │
// └──────────────────────────────────────────────────────────┘

pub mod engine;
pub mod selector;
pub mod interactive;

// Re-exports
pub use engine::{synthesize, CandidateImplementation, candidate_count, elapsed_ms};
pub use selector::select_best;
