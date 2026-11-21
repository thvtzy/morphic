// ─────────────────────────────────────────────────────────
//  MORPHIC PLAYGROUND (v0.6 W2) — check-only MVP.
//
//  Drives the WASM build of the Morphic compiler (the
//  pkg/ package generated from crates/morphic-wasm) to
//  parse + typecheck the editor's source via the library
//  API's `check`. Everything runs locally; the source
//  never leaves the page.
//
//  Honesty rules (v0.6 anti-patterns, applied to UI copy):
//  the test panel is an INVENTORY with bindability — a
//  check executes nothing, and no wording here claims
//  otherwise; no verification language appears, because
//  this build has no verifier (Z3 is native-only).
// ─────────────────────────────────────────────────────────
import init, { check } from '../pkg/morphic_wasm.js';

const EXAMPLES = [
  { file: 'validator.morph', label: 'Validator module — valid (7 specs)' },
  { file: 'tokenizer.morph', label: 'Functional tokenizer — valid (8 specs)' },
  { file: 'sort.morph', label: 'Sort — valid; tests the interpreter cannot bind' },
  { file: 'invalid-syntax.morph', label: 'Syntax error — invalid (deliberate)' },
  { file: 'invalid-type.morph', label: 'Type error — invalid (deliberate)' },
];

const editor = document.getElementById('editor');
const gutter = document.getElementById('gutter');
const backdrop = document.getElementById('backdrop');
const backdropCode = document.getElementById('backdrop-code');
const picker = document.getElementById('example-picker');
const pill = document.getElementById('status-pill');
const verdictEl = document.getElementById('verdict');
const diagnosticEl = document.getElementById('diagnostic');
const specListEl = document.getElementById('spec-list');
const testListEl = document.getElementById('test-list');

let errorLine = null; // 1-based, from the diagnostic when it carries one
let wasmReady = false;

for (const ex of EXAMPLES) {
  const opt = document.createElement('option');
  opt.value = ex.file;
  opt.textContent = ex.label;
  picker.appendChild(opt);
}

function escapeHtml(s) {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
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
        <div class="spec-sig">${escapeHtml(signature(spec))}</div>
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
}

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
