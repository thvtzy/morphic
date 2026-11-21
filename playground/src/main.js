// ─────────────────────────────────────────────────────────
//  MORPHIC PLAYGROUND (v0.6 W3) — check + synthesis.
//
//  Drives the WASM build of the Morphic compiler (the
//  pkg/ package generated from crates/morphic-wasm):
//  `check` for parse + typecheck, `synthesize_seeded_spec`
//  for the seeded playground-profile synthesis of one
//  named spec. Everything runs locally; the source never
//  leaves the page. Synthesis is synchronous on the main
//  thread in W3 (the worker shell is W4's milestone); the
//  profile is bounded, and the panel reports each run's
//  measured duration.
//
//  Honesty rules (v0.6 anti-patterns, applied to UI copy):
//  the check panel's test list is an INVENTORY with
//  bindability — a check executes nothing. The synthesis
//  panel shows EXECUTED evidence as the exact fraction
//  passed / evaluable / total, with per-test outcomes and
//  reasons; tests the interpreter could not bind stay in
//  the total, never dropped. Formal verification is
//  stated unavailable in this build wherever a verdict
//  would appear; winner status is shown exactly as the
//  build reports it. The WAT pane is a text preview —
//  nothing is assembled or executed.
// ─────────────────────────────────────────────────────────
import init, { check, synthesize_seeded_spec } from '../pkg/morphic_wasm.js';

const EXAMPLES = [
  { file: 'validator.morph', label: 'Validator module — valid (7 specs)' },
  { file: 'tokenizer.morph', label: 'Functional tokenizer — valid (8 specs)' },
  { file: 'sort.morph', label: 'Sort — valid; tests the interpreter cannot bind' },
  { file: 'invalid-syntax.morph', label: 'Syntax error — invalid (deliberate)' },
  { file: 'invalid-type.morph', label: 'Type error — invalid (deliberate)' },
];

const TARGET_LABELS = {
  rust: 'Rust',
  c: 'C',
  wasm: 'WAT — text preview, not assembled or executed',
  python: 'Python',
  javascript: 'JavaScript',
  ir: 'IR',
};
const TARGET_EXT = {
  rust: 'rs', c: 'c', wasm: 'wat', python: 'py', javascript: 'js', ir: 'ir.txt',
};

const editor = document.getElementById('editor');
const gutter = document.getElementById('gutter');
const backdrop = document.getElementById('backdrop');
const backdropCode = document.getElementById('backdrop-code');
const picker = document.getElementById('example-picker');
const pill = document.getElementById('status-pill');
const seedInput = document.getElementById('seed-input');
const verdictEl = document.getElementById('verdict');
const diagnosticEl = document.getElementById('diagnostic');
const specListEl = document.getElementById('spec-list');
const testListEl = document.getElementById('test-list');
const synthStatusEl = document.getElementById('synth-status');
const synthSummaryEl = document.getElementById('synth-summary');
const verificationEl = document.getElementById('verification-panel');
const synthTestsEl = document.getElementById('synth-tests');
const winnerBlockEl = document.getElementById('winner-block');

let errorLine = null; // 1-based, from the diagnostic when it carries one
let wasmReady = false;
let synthBusy = false;
let currentRenders = null; // winner renders of the last run
let currentSpecName = null;
let activeTarget = 'rust';

for (const ex of EXAMPLES) {
  const opt = document.createElement('option');
  opt.value = ex.file;
  opt.textContent = ex.label;
  picker.appendChild(opt);
}

function escapeHtml(s) {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

// ── Editor chrome: line numbers + error-line backdrop ───
function paintEditorChrome() {
  const lines = editor.value.split('\n');
  gutter.innerHTML = lines
    .map((_, i) => (i + 1 === errorLine
      ? `<span class="gutter-error">${i + 1}</span>`
      : `${i + 1}`))
    .join('\n');
  backdropCode.innerHTML = lines
    .map((line, i) => (i + 1 === errorLine
      ? `<mark>${escapeHtml(line) || ' '}</mark>`
      : escapeHtml(line)))
    .join('\n') + '\n';
  syncScroll();
}

function syncScroll() {
  backdrop.scrollTop = editor.scrollTop;
  backdrop.scrollLeft = editor.scrollLeft;
  gutter.scrollTop = editor.scrollTop;
}

editor.addEventListener('scroll', syncScroll);

// ── Rendering the CheckReport ───────────────────────────
function signature(spec) {
  const params = spec.params.map((p) => `${p.name}: ${p.ty}`).join(', ');
  return `${spec.name}(${params}) -> ${spec.return_type}`;
}

function clearSynthesis(message) {
  currentRenders = null;
  currentSpecName = null;
  synthStatusEl.textContent = message;
  synthSummaryEl.innerHTML = '';
  verificationEl.innerHTML = '';
  synthTestsEl.innerHTML = '';
  winnerBlockEl.innerHTML = '';
}

function render(report) {
  errorLine = null;
  if (report.ok) {
    pill.textContent = 'checks pass';
    pill.className = 'pill pill-ok';
    verdictEl.innerHTML =
      `<span class="verdict-ok">✓ Parse + typecheck pass</span>` +
      ` — ${report.specs.length} spec${report.specs.length === 1 ? '' : 's'} in this module.`;
    diagnosticEl.innerHTML = '';
  } else {
    const err = report.error || { stage: 'internal', message: 'unknown error' };
    pill.textContent = `${err.stage} error`;
    pill.className = 'pill pill-err';
    let position = '';
    if (typeof err.line === 'number') {
      errorLine = err.line;
      position = `<span class="position-chip">line ${err.line}, column ${err.column}</span>`;
    }
    verdictEl.innerHTML =
      `<span class="verdict-err">✗ ${escapeHtml(err.stage)} error</span>${position}`;
    diagnosticEl.innerHTML = `<pre>${escapeHtml(err.message)}</pre>`;
    if (report.specs.length === 0) {
      verdictEl.innerHTML +=
        `<div class="spec-meta">No specs could be read from this source.</div>`;
    }
  }

  if (report.specs.length === 0) {
    specListEl.innerHTML = '<p class="empty">—</p>';
    testListEl.innerHTML = '<p class="empty">—</p>';
  } else {
    specListEl.innerHTML = report.specs.map((spec) => `
      <div class="spec-block">
        <div class="spec-row">
          <span class="spec-sig">${escapeHtml(signature(spec))}</span>
          <button type="button" class="synth-button" data-spec="${escapeHtml(spec.name)}"
            ${report.ok && !synthBusy ? '' : 'disabled'}>Synthesize</button>
        </div>
        <div class="spec-meta">${spec.postconditions} postcondition${spec.postconditions === 1 ? '' : 's'}
          · ${spec.tests.length} in-spec test${spec.tests.length === 1 ? '' : 's'}</div>
      </div>`).join('');

    testListEl.innerHTML = report.specs.map((spec) => {
      if (spec.tests.length === 0) {
        return `<div class="spec-block"><div class="spec-sig">${escapeHtml(spec.name)}</div>
          <p class="empty">no in-spec tests</p></div>`;
      }
      const items = spec.tests.map((t) => `
        <li>
          <span class="test-name">${escapeHtml(t.name ?? `test #${t.index + 1}`)}</span>
          ${t.bindable
            ? '<span class="chip chip-bindable">interpreter can bind</span>'
            : '<span class="chip chip-unbindable">cannot be bound by the interpreter</span>'}
        </li>`).join('');
      return `<div class="spec-block"><div class="spec-sig">${escapeHtml(spec.name)}</div>
        <ul class="test-list">${items}</ul></div>`;
    }).join('');
  }
  paintEditorChrome();
}

function runCheck() {
  if (!wasmReady) return;
  let report;
  try {
    report = JSON.parse(check(editor.value));
  } catch (e) {
    report = {
      ok: false,
      error: { stage: 'internal', message: String(e) },
      specs: [],
    };
  }
  render(report);
  clearSynthesis('No synthesis run yet for the current source.');
}

// ── Synthesis ───────────────────────────────────────────
function currentSeed() {
  const n = Number.parseInt(seedInput.value, 10);
  return Number.isFinite(n) && n >= 0 ? n : 42;
}

const OUTCOME_CHIPS = {
  passed: '<span class="chip chip-passed">passed</span>',
  failed: '<span class="chip chip-failed">failed</span>',
  body_not_evaluable: '<span class="chip chip-unbindable">body not evaluable</span>',
  unbindable: '<span class="chip chip-unbindable">cannot be bound</span>',
};

function renderSynthesis(report, seed, elapsedMs) {
  const w = report.winner;
  currentRenders = w.renders;
  currentSpecName = report.spec_name;
  activeTarget = 'rust';

  synthStatusEl.textContent =
    `Synthesis of ${report.spec_name} with seed ${seed} ran locally in ${Math.round(elapsedMs)} ms.`;

  const unjudged = w.tests_total - w.tests_evaluable;
  let coverageNote;
  if (w.tests_total === 0) {
    coverageNote = 'This spec carries no in-spec tests, so there is no executed evidence either way.';
  } else if (w.tests_evaluable === 0) {
    coverageNote = 'The interpreter could not bind any of this spec\'s tests ' +
      '(see the per-test reasons below), so the fraction carries no executed ' +
      'evidence — the unjudged tests stay in the total, counted, not dropped.';
  } else if (unjudged > 0) {
    coverageNote = `The interpreter could not bind ${unjudged} of the ${w.tests_total} tests; ` +
      'they stay in the total, counted, not dropped.';
  } else {
    coverageNote = 'The interpreter judged every in-spec test.';
  }
  synthSummaryEl.innerHTML = `
    <div class="spec-block">
      <div class="spec-sig">Winner for ${escapeHtml(report.spec_name)}</div>
      <div class="fraction-line">Executed evidence:
        <strong class="fraction">${w.tests_passed} / ${w.tests_evaluable} / ${w.tests_total}</strong>
        <span class="spec-meta">passed / evaluable / total in-spec tests</span></div>
      <div class="spec-meta">${escapeHtml(coverageNote)}</div>
      <div class="spec-meta">Pool: ${report.candidates} candidates evaluated; the winner is
        shown in full (per-candidate detail is not exposed by the library API yet).
        Selection score ${w.score.toFixed(3)} · constraint score ${w.constraint_score.toFixed(3)}.</div>
    </div>`;

  verificationEl.innerHTML = `
    <div class="verification-panel">
      <strong>Formal verification unavailable in browser build.</strong>
      This build has no Z3 verifier and no compiled runner (both are native-only),
      so this winner carries no formal verdict. Winner status as reported by this
      build: <span class="status-word">${escapeHtml(w.verification_status)}</span>
      · verification available in this build: ${report.verification_available ? 'yes' : 'no'}.
      The executed-test fraction above is the evidence this page can offer.
    </div>`;

  if (w.test_outcomes.length === 0) {
    synthTestsEl.innerHTML = '<p class="empty">no in-spec tests</p>';
  } else {
    const items = w.test_outcomes.map((o) => `
      <li>
        <span class="test-name">${escapeHtml(o.name ?? `test #${o.index + 1}`)}</span>
        ${OUTCOME_CHIPS[o.outcome] ?? ''}
      </li>
      ${o.reason ? `<li class="reason-row">${escapeHtml(o.reason)}</li>` : ''}`).join('');
    synthTestsEl.innerHTML = `
      <div class="spec-block">
        <div class="spec-sig">Executed outcomes per test</div>
        <ul class="test-list">${items}</ul>
      </div>`;
  }

  renderWinnerBlock();
}

function renderWinnerBlock() {
  if (!currentRenders) { winnerBlockEl.innerHTML = ''; return; }
  const tabs = currentRenders.map((r) => `
    <button type="button" class="render-tab${r.target === activeTarget ? ' tab-active' : ''}"
      data-target="${r.target}">${escapeHtml(TARGET_LABELS[r.target] ?? r.target)}</button>`).join('');
  const active = currentRenders.find((r) => r.target === activeTarget) ?? currentRenders[0];
  const body = active.error
    ? `<pre class="render-pane render-error">${escapeHtml(active.error)}</pre>`
    : `<pre class="render-pane">${escapeHtml(active.text ?? '')}</pre>`;
  winnerBlockEl.innerHTML = `
    <div class="spec-block">
      <div class="spec-sig">Winner renders</div>
      <div class="render-tabs">${tabs}
        <span class="render-actions">
          <button type="button" id="copy-render">Copy</button>
          <button type="button" id="download-render">Download</button>
        </span>
      </div>
      ${body}
      <div class="spec-sig">Winner body (IR — the pipeline's own body identity)</div>
      <pre class="winner-body">${escapeHtml(currentBodyDebug ?? '')}</pre>
    </div>`;
  document.getElementById('copy-render').addEventListener('click', copyRender);
  document.getElementById('download-render').addEventListener('click', downloadRender);
}

let currentBodyDebug = null;

function activeRenderText() {
  const active = currentRenders?.find((r) => r.target === activeTarget);
  return active && !active.error ? active.text : null;
}

async function copyRender() {
  const text = activeRenderText();
  if (text == null) return;
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // Clipboard can be unavailable (permissions, non-secure
    // context): select-and-copy stays possible from the pane.
  }
}

function downloadRender() {
  const text = activeRenderText();
  if (text == null) return;
  const name = `${(currentSpecName ?? 'winner').replace(/[^\w.-]+/g, '_')}.${TARGET_EXT[activeTarget] ?? 'txt'}`;
  const url = URL.createObjectURL(new Blob([text], { type: 'text/plain' }));
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}

winnerBlockEl.addEventListener('click', (e) => {
  const tab = e.target.closest('.render-tab');
  if (!tab) return;
  activeTarget = tab.dataset.target;
  renderWinnerBlock();
});

function setSynthButtonsEnabled(enabled) {
  for (const b of specListEl.querySelectorAll('.synth-button')) {
    b.disabled = !enabled;
  }
}

function runSynthesis(specName) {
  if (!wasmReady || synthBusy) return;
  const seed = currentSeed();
  synthBusy = true;
  setSynthButtonsEnabled(false);
  synthStatusEl.textContent = `Synthesizing ${specName} with seed ${seed} — running locally…`;
  // Let the busy state paint before the synchronous WASM
  // call blocks the main thread.
  setTimeout(() => {
    const t0 = performance.now();
    let report = null;
    let failure = null;
    try {
      report = JSON.parse(synthesize_seeded_spec(editor.value, specName, seed));
    } catch (e) {
      failure = String(e);
    }
    const elapsed = performance.now() - t0;
    synthBusy = false;
    setSynthButtonsEnabled(true);
    if (failure || !report || !report.winner) {
      const stage = report?.stage ?? 'internal';
      const message = report?.message ?? failure ?? 'unknown error';
      clearSynthesis('');
      synthStatusEl.innerHTML =
        `<span class="verdict-err">✗ synthesis failed (${escapeHtml(stage)})</span> — ${escapeHtml(message)}`;
      return;
    }
    currentBodyDebug = report.winner.body_debug;
    renderSynthesis(report, seed, elapsed);
  }, 30);
}

specListEl.addEventListener('click', (e) => {
  const btn = e.target.closest('.synth-button');
  if (btn && !btn.disabled) runSynthesis(btn.dataset.spec);
});

// ── Wiring ──────────────────────────────────────────────
let debounce = null;
editor.addEventListener('input', () => {
  paintEditorChrome();
  clearTimeout(debounce);
  debounce = setTimeout(runCheck, 400);
});
document.getElementById('check-button').addEventListener('click', runCheck);

async function loadExample(file) {
  const response = await fetch(`examples/${file}`);
  editor.value = await response.text();
  errorLine = null;
  paintEditorChrome();
  runCheck();
}
picker.addEventListener('change', () => loadExample(picker.value));

// ── Boot ────────────────────────────────────────────────
await init();
wasmReady = true;
pill.textContent = 'ready';
const wanted = new URLSearchParams(location.search).get('example');
const initial = EXAMPLES.some((e) => e.file === wanted) ? wanted : EXAMPLES[0].file;
picker.value = initial;
await loadExample(initial);
