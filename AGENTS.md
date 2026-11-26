# Morphic Workspace Rules

These rules are mandatory for every task in this repository. Never relax them under time pressure.

## Non-Negotiable Hard Rules

1. **`cargo test` ONLY — NEVER `cargo check`.** `z3-sys` build-script fingerprints mismatch under `cargo check` (the product command `morphic check` is unrelated and fine). For fast inner-loop checks, run a targeted `cargo test --lib <filter>` or `cargo test --test <bin>`.
2. **NEVER run `cargo clean` and NEVER delete root `target/`.** Preserves the compiled Z3 build cache.
3. **Run the three frozen test configurations SOLO (one at a time), NEVER in parallel:**
   - `cargo test --no-fail-fast` (default)
   - `cargo test --features bootstrap-codegen --no-fail-fast`
   - `cargo test --no-default-features --no-fail-fast`
   Heavy Z3 crown tests flip under CPU load; parallel runs produce invalid evidence. Always save full test output to a log file (never rely on truncated terminal output).
4. **Known load flake (`phase84_symbolic_state_encodability`):** If it fails during a full suite run, NEVER revert code immediately. Re-run it in isolation (`cargo test --lib phase84_symbolic_state_encodability -- --exact`) AND re-run its whole test binary solo (`cargo test --lib`) before calling it a regression.
5. **One topic / phase = one branch, two separate commits:**
   - Write a failing reproduction test or probe first, then minimal fix, then verify all three suites solo.
   - Commit 1: code and tests only.
   - Commit 2: docs (`ROAD_TO_SELF_HOSTING.md` note and/or `CHANGELOG.md` bullet) separately. NEVER use `git add -A` to mix code and docs into one commit.
6. **PR and CI discipline:** Pass PR bodies via `--body-file` (never an inline quoted shell string). Inspect ALL CI checks green before merging any PR, and merge with `gh pr merge --merge` so separate code and docs commits are preserved.
7. **Honesty discipline:** Never report an unmeasured claim as a measurement. Never amend frozen `*_SCOPE.md` gate texts; record incidents, flips, and discarded runs openly.
8. **Version lockstep:** Crate, packages (`packages/npm/`, `packages/rust/`), components (`packages/component/`, `packages/component-callbacks/`), and VS Code `.vsix` versions move together ONLY at an explicit release stamp — never bump versions for a standalone bugfix or chore.
9. **Owner-only releases:** NEVER execute `npm publish`, `cargo publish`, VS Code Marketplace publish, or live deploys — even if asked in a hurry. At most run `--dry-run` from a tagged tree and hand over the exact commands for the owner to run.
10. **Language split:** Communicate with the owner in casual Indonesian. Write all code, comments, docs, PR bodies, and commit messages in English.

## Workflow Skill

For campaign phases (Audit → Freeze → Build → Gate), skill routing, frozen budgets, and the solo 3-suite runner script, load the workspace skill `developing-morphic` (`.agents/skills/developing-morphic/SKILL.md`).
