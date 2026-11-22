#!/usr/bin/env node
// ─────────────────────────────────────────────────────────
//  GATE RUNNER (Phase 67 W2 + Phase 68 W3): pinned-corpus
//  agreement + synthesis pins + UI-copy audit.
//
//  Every check case in cases.json is run through BOTH:
//    * the native CLI  — `morphic check <file>`, accept iff
//      exit code 0 (the CLI's own contract), and
//    * the playground WASM module — the exact pkg/ build
//      the page loads (wasm-bindgen --target web glue,
//      initialised from the .wasm bytes, which is also how
//      Node can drive a web-target module).
//  Accept/reject must agree on EVERY case (the W2 gate).
//  Stage / position / bindability expectations from the
//  corpus are asserted against the WASM report.
//
//  Prerequisites: playground built (../build.sh) and the
//  native CLI built (`cargo build --bin morphic`, or set
//  MORPHIC_BIN). Exit code 0 iff all cases pass.
// ─────────────────────────────────────────────────────────
import { readFileSync, writeFileSync, mkdtempSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, '..', '..');
const morphicBin = process.env.MORPHIC_BIN || join(repoRoot, 'target', 'debug', 'morphic');

const corpus = JSON.parse(readFileSync(join(here, 'cases.json'), 'utf8'));

// ── WASM side: initialise the page's own module ─────────
const pkgDir = join(here, '..', 'pkg');
let wasm;
try {
  wasm = await import(join(pkgDir, 'morphic_wasm.js'));
  const bytes = readFileSync(join(pkgDir, 'morphic_wasm_bg.wasm'));
  await wasm.default({ module_or_path: bytes });
  // Phase 112: install the standard-library asset exactly
  // as the page does (the library is runtime data now,
  // not bundle data). A corpus run against an
  // uninstalled prelude would silently test a different
  // compiler than the page ships.
  const asset = readFileSync(join(pkgDir, 'stdlib-asset.bin'));
  if (wasm.init_stdlib(asset) !== true) {
    console.error('init_stdlib did not confirm installation');
    process.exit(2);
  }
} catch (e) {
  console.error(`cannot load the playground WASM package at ${pkgDir} — run playground/build.sh first\n${e}`);
  process.exit(2);
}

// ── Native side: the CLI binary must exist ──────────────
{
  const probe = spawnSync(morphicBin, ['--version'], { encoding: 'utf8' });
  if (probe.error) {
    console.error(`cannot run the native CLI at ${morphicBin} — build it with \`cargo build --bin morphic\` or set MORPHIC_BIN`);
    process.exit(2);
  }
}

const scratch = mkdtempSync(join(tmpdir(), 'morphic-w2-corpus-'));
let failures = 0;

for (const c of corpus.cases) {
  const source = c.file
    ? readFileSync(resolve(here, c.file), 'utf8')
    : c.source;

  // Native CLI on a real file (its filesystem contract).
  const file = join(scratch, `${c.id}.morph`);
  writeFileSync(file, source);
  const cli = spawnSync(morphicBin, ['check', file], { encoding: 'utf8' });
  const cliOk = cli.status === 0;

  // Playground WASM on the same source text.
  const report = JSON.parse(wasm.check(source));
  const wasmOk = report.ok === true;

  const problems = [];
  if (cliOk !== c.expect.ok) problems.push(`native CLI ${cliOk ? 'accepted' : 'rejected'}, corpus expects ${c.expect.ok ? 'accept' : 'reject'}`);
  if (wasmOk !== c.expect.ok) problems.push(`WASM ${wasmOk ? 'accepted' : 'rejected'}, corpus expects ${c.expect.ok ? 'accept' : 'reject'}`);
  if (cliOk !== wasmOk) problems.push(`DISAGREEMENT: native=${cliOk} wasm=${wasmOk}`);
  if (c.expect.stage) {
    if (!report.error || report.error.stage !== c.expect.stage)
      problems.push(`WASM stage ${report.error?.stage ?? '(none)'} != expected ${c.expect.stage}`);
  }
  if (c.expect.hasPosition) {
    if (!report.error || typeof report.error.line !== 'number' || typeof report.error.column !== 'number')
      problems.push('WASM error carries no line/column');
  }
  const tests = report.specs.flatMap((s) => s.tests);
  const unbindable = tests.filter((t) => !t.bindable).length;
  const bindable = tests.length - unbindable;
  if (c.expect.minUnbindable && unbindable < c.expect.minUnbindable)
    problems.push(`unbindable tests ${unbindable} < expected >= ${c.expect.minUnbindable}`);
  if (c.expect.minBindable && bindable < c.expect.minBindable)
    problems.push(`bindable tests ${bindable} < expected >= ${c.expect.minBindable}`);

  const status = problems.length === 0 ? 'PASS' : 'FAIL';
  if (problems.length) failures++;
  console.log(
    `${status}  ${c.id.padEnd(30)} native=${cliOk ? 'accept' : 'reject'}  wasm=${wasmOk ? 'accept' : 'reject'}` +
    (report.error ? `  stage=${report.error.stage}` : '') +
    (tests.length ? `  tests=${tests.length} (bindable ${bindable}, unbindable ${unbindable})` : '')
  );
  for (const p of problems) console.log(`      ✗ ${p}`);
}

// ── W3 synthesis gate (Phase 68) ────────────────────────
// Every synthesis case was pinned from the NATIVE
// interpreter first (the config-gated Rust harness
// api::tests::w3_gate_fractions_match_native_interpreter,
// run in this build's own no-Z3/no-runner configuration,
// asserts body identity + run_spec_tests reproduction
// there). Here the WASM build must reproduce every pin:
// the exact fraction, the per-test outcome kinds, the
// winner status, and the winner body's SHA-256 (body
// identity across builds, via the native pins).
let synthFailures = 0;
for (const c of corpus.synthesisCases ?? []) {
  const source = c.file
    ? readFileSync(resolve(here, c.file), 'utf8')
    : c.source;
  const problems = [];
  let report = null;
  try {
    report = JSON.parse(wasm.synthesize_seeded_spec(source, c.spec, c.seed));
  } catch (e) {
    problems.push(`WASM synthesis threw: ${e}`);
  }
  if (report && !report.winner) {
    problems.push(`WASM synthesis failed at stage ${report.stage}: ${report.message}`);
    report = null;
  }
  if (report) {
    const w = report.winner;
    const e = c.expect;
    if (report.spec_name !== c.spec) problems.push(`spec ${report.spec_name} != ${c.spec}`);
    if (w.tests_passed !== e.passed || w.tests_evaluable !== e.evaluable || w.tests_total !== e.total)
      problems.push(`fraction ${w.tests_passed}/${w.tests_evaluable}/${w.tests_total} != pin ${e.passed}/${e.evaluable}/${e.total}`);
    if (!(w.tests_passed <= w.tests_evaluable && w.tests_evaluable <= w.tests_total))
      problems.push('fraction ordering violated (passed <= evaluable <= total)');
    const kinds = w.test_outcomes.map((o) => o.outcome);
    if (JSON.stringify(kinds) !== JSON.stringify(e.outcomes))
      problems.push(`outcomes ${kinds.join(',')} != pin ${e.outcomes.join(',')}`);
    if (w.verification_status !== e.status)
      problems.push(`status ${w.verification_status} != pin ${e.status}`);
    if (report.verification_available !== e.verificationAvailable)
      problems.push(`verification_available ${report.verification_available} != pin ${e.verificationAvailable}`);
    const hash = createHash('sha256').update(w.body_debug).digest('hex');
    if (hash !== e.bodySha256)
      problems.push(`winner body hash ${hash.slice(0, 12)}… != pin ${e.bodySha256.slice(0, 12)}… (body identity)`);
    if (!Array.isArray(w.renders) || w.renders.length !== 6)
      problems.push(`renders: expected 6 targets, got ${w.renders?.length}`);
    if (e.unbindableReasonContains) {
      const bad = w.test_outcomes.filter(
        (o) => o.outcome === 'unbindable' && !(o.reason ?? '').includes(e.unbindableReasonContains));
      if (bad.length) problems.push('an unbindable outcome does not name the expected reason class');
    }
  }
  const status = problems.length === 0 ? 'PASS' : 'FAIL';
  if (problems.length) synthFailures++;
  console.log(`${status}  [synth] ${c.id.padEnd(28)} pin ${c.expect.passed}/${c.expect.evaluable}/${c.expect.total}`);
  for (const p of problems) console.log(`      ✗ ${p}`);
}

// ── W3 UI-copy audit ────────────────────────────────────
// The word "Verified" must not appear anywhere in the
// built site's copy: this build produces no formally
// verified result, and no wording may claim one. (The
// mandated panel text is "Formal verification unavailable
// in browser build" — lowercase, a statement about the
// build, not a claim about a result.)
{
  const siteFiles = ['../index.html', '../src/main.js', '../styles.css'];
  const offenders = siteFiles.filter((f) =>
    readFileSync(resolve(here, f), 'utf8').includes('Verified'));
  if (offenders.length) {
    synthFailures++;
    console.log(`FAIL  [copy] "Verified" appears in: ${offenders.join(', ')}`);
  } else {
    console.log('PASS  [copy] no "Verified" claim anywhere in the site copy');
  }
}

console.log(`\n${corpus.cases.length - failures}/${corpus.cases.length} check cases agree`);
if (corpus.synthesisCases) {
  const total = corpus.synthesisCases.length + 1; // + the copy audit
  console.log(`${total - synthFailures}/${total} synthesis gate checks pass (incl. UI-copy audit)`);
}
process.exit(failures === 0 && synthFailures === 0 ? 0 : 1);
