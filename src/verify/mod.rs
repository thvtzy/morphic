// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC VERIFIER                                        │
// │  Formal verification pipeline. The verdict vocabulary    │
// │  and the encodability predicate are pure (verdict.rs,    │
// │  encodable.rs); the Z3 session backend (smt.rs + the     │
// │  solver paths in verifier.rs) exists only under the      │
// │  `z3-support` feature (Phase 66 seam). Without it, the   │
// │  verifier still exists — it judges every well-formed     │
// │  candidate Inconclusive, because no formal verdict is    │
// │  available in that build (v0.6 D2).                      │
// └──────────────────────────────────────────────────────────┘

pub mod encodable;
pub mod verdict;
pub mod verifier;
#[cfg(feature = "z3-support")]
pub mod smt;

pub use verifier::{verify_all, VerificationResult, VerificationStatus};
