// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC VERIFIER                                        │
// │  Z3-based formal verification pipeline                   │
// └──────────────────────────────────────────────────────────┘

pub mod verifier;
pub mod smt;

pub use verifier::{verify_all, VerificationResult, VerificationStatus};
