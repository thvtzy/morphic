#!/usr/bin/env node
// ─────────────────────────────────────────────────────────
//  W2 GATE RUNNER (Phase 67): pinned-corpus agreement.
//
//  Every case in cases.json is checked by BOTH:
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

console.log(`\n${corpus.cases.length - failures}/${corpus.cases.length} cases agree`);
process.exit(failures === 0 ? 0 : 1);
