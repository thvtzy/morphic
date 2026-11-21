// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC SYNTHESIS MODULE                                │
// │  Hybrid MCTS + Z3 + LLM synthesis engine                 │
// └──────────────────────────────────────────────────────────┘

pub mod engine;
pub mod eval;
pub mod selector;
pub mod templates;

// The compiled-runner scoring stage (Phase 58) spawns rustc
// and compiled candidates — it cannot exist in a browser
// build, so it sits behind the `compiled-runner` feature
// (Phase 66 seam; on in the native default). Without the
// feature, `runner` is the stub in runner_stub.rs: the
// stage's documented degradation, the pool returned
// unchanged — the same outcome the real stage produces on
// infrastructure failure, never a build failure.
#[cfg(feature = "compiled-runner")]
pub mod runner;
#[cfg(not(feature = "compiled-runner"))]
#[path = "runner_stub.rs"]
pub mod runner;

// Interactive synthesis TUI — CLI furniture (colored +
// indicatif + sleeps), native binary only. The browser
// profile excludes it via the `interactive` feature.
#[cfg(feature = "interactive")]
pub mod interactive;

// Re-exports
pub use engine::{synthesize, CandidateImplementation, candidate_count, elapsed_ms};
pub use selector::select_best;
