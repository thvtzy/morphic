---
name: developing-morphic
description: Use when planning, auditing, implementing, debugging, verifying, or releasing any feature, phase, or bugfix in the Morphic compiler repository
---

# Developing Morphic

## Overview

Morphic sells formal proof. Every change follows one auditable discipline: measure before freezing, test before implementing, verify solo across all three frozen configurations, and never claim an unmeasured result.

**Ground truth:** `LANGUAGE_REFERENCE.md` (language syntax — never invent syntax), `ROAD_TO_SELF_HOSTING.md` (authoritative phase history), `CHANGELOG.md`, and the active campaign's `*_AUDIT.md` + `*_SCOPE.md`.

## Skill Routing (Use Installed Skills Together)

| Situation | REQUIRED Skill / Tool |
|---|---|
| New milestone, architecture, or D-list design | `brainstorming`, then `writing-plans` |
| Multi-file read-only survey during an Audit phase | `research` subagent (read-only parallel OK; NEVER parallel `cargo test`) |
| Implementing any feature, emitter, or bugfix | `test-driven-development` + `ponytail` (minimal diff, protect byte budgets) |
| Z3 `Inconclusive`, counterexamples, or unexpected test failures | `systematic-debugging` |
| Reporting pass counts, gate verdicts, or claiming completion | `verification-before-completion` |
| Pre-PR review and branch integration | `requesting-code-review`, then `finishing-a-development-branch` |

## Campaign Cycle (Phases)

1. **Audit phase (`docs/<campaign>-<N>-audit`):** Zero product code. Write probe tests (`tests/phase<N>_*.rs`), measure real boundaries, draft `*_AUDIT.md` with an explicit D-list (`D1`..`Dk`) and recommended options `(a)` with grounds.
2. **Freeze phase:** Record ratified decisions in `*_SCOPE.md` with frozen clauses, budgets, and closing line. Once frozen, never amend gate text; if reality disagrees, measure and record it.
3. **Build phases (one topic = one branch):**
   - **Inner loop (fast):** Run ONLY the relevant test binary (`cargo test --test phase<N>_*` or `cargo test --lib <filter>`). Never run `cargo check`.
   - **Outer gate (before commit):** Run all three frozen configurations SOLO using the bundled script (below).
   - **Two commits:** Commit 1 = code + tests; Commit 2 = docs (`ROAD_TO_SELF_HOSTING.md` + `CHANGELOG.md`).
4. **Gate + PR:** Run gate instruments clause by clause, open PR with `gh pr create --body-file <file>`, watch CI (`gh pr checks <num> --watch`), merge ONLY after every check is green.
5. **Release (owner-only publish):** Stamp versions in lockstep only at the gate phase. Never execute `npm publish` or `cargo publish`; run `--dry-run` and hand the exact commands to the owner.

## Solo 3-Suite Runner

Always invoke through the interpreter so logs are never truncated and suites never overlap:

```powershell
powershell -ExecutionPolicy Bypass -File .agents/skills/developing-morphic/scripts/run-suites.ps1
```

Single configuration during triage: `-Config default`, `-Config bootstrap`, or `-Config no-z3`. Logs are written to `$env:TEMP\morphic-suite-logs\` (outside the repo).

## Frozen Budgets & Invariants

| Surface | Frozen Cap / Rule |
|---|---|
| Playground WASM bundle | ≤ 771,072 B |
| npm package (`packages/npm/`) | ≤ 76,800 B (77 files, 35 exports) |
| WASM components | ≤ 106,496 B (library) / ≤ 90,112 B (callbacks) |
| Registry seed index / catalog | ≤ 12,288 B / ≤ 295,000 B |
| Prelude funnel | `stdlib::{prelude_specs, shipped_body}`; precedence `user > stdlib > installed` |
| Line endings | Pure LF enforced by `.gitattributes` (`0` tracked files `w/crlf`) |

Re-baselining a budget requires an explicit, recorded decision — never a silent edit.

## Rationalization Counters (Red Flags)

| Excuse | Required Action |
|---|---|
| "Just a quick `cargo check` to see if it compiles" | **Forbidden.** Run `cargo test --lib <module>` or `cargo test --test <bin>`. |
| "Run all 3 suites in parallel to save 10 minutes" | **Forbidden.** Z3 crowns flip under load; run `run-suites.ps1` sequentially. |
| "`phase84_symbolic_state_encodability` failed, revert!" | **Stop.** Re-run it with `--exact` AND run `cargo test --lib` solo first. |
| "Combine code and `CHANGELOG.md` via `git add -A`" | **Forbidden.** Stage code/tests in Commit 1 and docs in Commit 2. |
| "User asked to run `cargo publish` / `npm publish`" | **Refuse politely.** Run `--dry-run` only and hand over exact commands. |
| "CI is almost done, merge now" | **Forbidden.** Inspect every CI check green first. |
