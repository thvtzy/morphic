// ─────────────────────────────────────────────────────────────
//  ELAPSED-TIME MARKS (Phase 66, v0.6 W1 — the S6 seam)
//
//  The pipeline uses wall-clock time ONLY for statistics:
//  the engine's ELAPSED_MS counter and the verifier's
//  verification_time_ms. It never drives control flow (the
//  engine's `max_time_ms` config is never consulted in the
//  run path — measured in Phase 66), so a build may report
//  elapsed time as 0 without changing any outcome.
//
//  It MUST do so on wasm32-unknown-unknown: std has no
//  clock there and `Instant::now()` PANICS ("time not
//  implemented on this platform", std/src/sys/time/
//  unsupported.rs — measured in the Phase 66 Node smoke,
//  where the panic killed the synthesis call while `check`
//  ran fine). On that target a mark is a zero clock; the
//  JS host owns real timing (performance.now(), W4's
//  worker), and in-compiler elapsed figures in the browser
//  build read 0 — a statistic reading zero, never a
//  disguised duration.
// ─────────────────────────────────────────────────────────────

/// A point in time for elapsed statistics. Copy; query it
/// any number of times.
#[derive(Debug, Clone, Copy)]
pub struct TimeMark {
    #[cfg(not(target_arch = "wasm32"))]
    inner: std::time::Instant,
    #[cfg(target_arch = "wasm32")]
    inner: (),
}

/// Take a time mark.
pub fn now() -> TimeMark {
    TimeMark {
        #[cfg(not(target_arch = "wasm32"))]
        inner: std::time::Instant::now(),
        #[cfg(target_arch = "wasm32")]
        inner: (),
    }
}

impl TimeMark {
    /// Whole milliseconds since the mark (0 on wasm32 —
    /// see the module docs).
    pub fn elapsed_ms(&self) -> u64 {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.inner.elapsed().as_millis() as u64
        }
        #[cfg(target_arch = "wasm32")]
        {
            0
        }
    }
}
