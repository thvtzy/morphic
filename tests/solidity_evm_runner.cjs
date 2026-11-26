#!/usr/bin/env node
// ─────────────────────────────────────────────────────────────
//  MORPHIC SOLIDITY + EVM HARNESS (Phase 170, IKONIK_18_SCOPE §7)
//
//  Compiles generated Solidity contracts via pinned `solc` (solc-js),
//  asserts ZERO errors and ZERO warnings, deploys creation bytecode
//  into a local `@ethereumjs/evm` instance, executes ABI-encoded
//  calls against every battery point, and verifies runtime
//  `require(...)` / Panic reverts on out-of-domain inputs.
//
//  Dependencies live ONLY in a scratch directory under `target/`
//  (never global, never committed `node_modules/`), passed via
//  argv[2] (`<harness-dir>`) and argv[3] (`<plan.json>`).
// ─────────────────────────────────────────────────────────────

'use strict';

const fs = require('fs');
const path = require('path');

function usage() {
  console.error('Usage: node tests/solidity_evm_runner.cjs <harness-dir> <plan.json>');
  process.exit(2);
}

const harnessDir = process.argv[2];
const planPath = process.argv[3];
if (!harnessDir || !planPath) usage();

const nm = path.resolve(harnessDir, 'node_modules');
const solc = require(path.join(nm, 'solc'));
const { EVM } = require(path.join(nm, '@ethereumjs', 'evm'));
const { ethers } = require(path.join(nm, 'ethers'));

function parseArg(val) {
  if (Array.isArray(val)) return val.map(parseArg);
  if (typeof val === 'boolean') return val;
  if (typeof val === 'number') return BigInt(val);
  if (typeof val === 'object' && val !== null && 'str' in val) return String(val.str);
  if (typeof val === 'string') return BigInt(val);
  throw new Error(`Unsupported arg JSON shape: ${JSON.stringify(val)}`);
}

function normalizeResult(val) {
  if (typeof val === 'bigint') return val.toString();
  if (typeof val === 'boolean') return val;
  if (typeof val === 'string') return { str: val };
  if (Array.isArray(val)) return Array.from(val, normalizeResult);
  return String(val);
}

function decodeRevert(iface, returnValue) {
  if (!returnValue || returnValue.length === 0) {
    return { kind: 'empty', reason: '' };
  }
  const hex = ethers.hexlify(returnValue);
  // Standard Error(string) selector = 0x08c379a0
  if (hex.startsWith('0x08c379a0')) {
    const [reason] = ethers.AbiCoder.defaultAbiCoder().decode(
      ['string'],
      '0x' + hex.slice(10)
    );
    return { kind: 'Error', reason };
  }
  // Standard Panic(uint256) selector = 0x4e487b71
  if (hex.startsWith('0x4e487b71')) {
    const [code] = ethers.AbiCoder.defaultAbiCoder().decode(
      ['uint256'],
      '0x' + hex.slice(10)
    );
    return { kind: 'Panic', code: '0x' + code.toString(16) };
  }
  try {
    const parsed = iface.parseError(hex);
    if (parsed) return { kind: parsed.name, reason: String(parsed.args) };
  } catch (_) {}
  return { kind: 'raw', hex };
}

async function runContract(spec) {
  const input = {
    language: 'Solidity',
    sources: {
      [spec.file]: { content: spec.source },
    },
    settings: {
      evmVersion: 'shanghai',
      optimizer: { enabled: false },
      outputSelection: {
        '*': {
          '*': ['abi', 'evm.bytecode.object'],
        },
      },
    },
  };

  const output = JSON.parse(solc.compile(JSON.stringify(input)));
  const diagnostics = output.errors || [];
  if (diagnostics.length > 0) {
    const formatted = diagnostics
      .map((d) => `[${d.severity}${d.errorCode ? ' ' + d.errorCode : ''}] ${d.formattedMessage || d.message}`)
      .join('\n');
    throw new Error(`${spec.file}: solc reported ${diagnostics.length} diagnostic(s) (0 allowed):\n${formatted}`);
  }

  const fileContracts = output.contracts && output.contracts[spec.file];
  if (!fileContracts || !fileContracts[spec.contract]) {
    const found = fileContracts ? Object.keys(fileContracts).join(', ') : '(none)';
    throw new Error(`${spec.file}: contract '${spec.contract}' not found in solc output (found: ${found})`);
  }

  const compiled = fileContracts[spec.contract];
  const bytecodeHex = compiled.evm.bytecode.object;
  if (!bytecodeHex || bytecodeHex.length === 0) {
    throw new Error(`${spec.file}: empty bytecode for '${spec.contract}'`);
  }

  const evm = await EVM.create();
  const deploy = await evm.runCall({
    data: ethers.getBytes('0x' + bytecodeHex),
    gasLimit: 30_000_000n,
  });
  if (deploy.execResult.exceptionError || !deploy.createdAddress) {
    throw new Error(
      `${spec.file}: deployment failed: ${deploy.execResult.exceptionError?.error || 'no createdAddress'}`
    );
  }

  const addr = deploy.createdAddress;
  const iface = new ethers.Interface(compiled.abi);

  let callCount = 0;
  for (const c of spec.calls || []) {
    const args = (c.args || []).map(parseArg);
    const calldata = iface.encodeFunctionData(c.fn, args);
    const res = await evm.runCall({
      to: addr,
      data: ethers.getBytes(calldata),
      gasLimit: 30_000_000n,
    });
    if (res.execResult.exceptionError) {
      const rev = decodeRevert(iface, res.execResult.returnValue);
      throw new Error(
        `${spec.file}::${c.fn}(${JSON.stringify(c.args)}) unexpectedly reverted: ${res.execResult.exceptionError.error} (${JSON.stringify(rev)})`
      );
    }
    const decoded = iface.decodeFunctionResult(c.fn, res.execResult.returnValue);
    const actual = normalizeResult(decoded[0]);
    const expected = c.expect;
    if (JSON.stringify(actual) !== JSON.stringify(expected)) {
      throw new Error(
        `${spec.file}::${c.fn}(${JSON.stringify(c.args)}) mismatch: actual=${JSON.stringify(actual)} expected=${JSON.stringify(expected)}`
      );
    }
    callCount += 1;
  }

  let revertCount = 0;
  for (const r of spec.reverts || []) {
    const args = (r.args || []).map(parseArg);
    const calldata = iface.encodeFunctionData(r.fn, args);
    const res = await evm.runCall({
      to: addr,
      data: ethers.getBytes(calldata),
      gasLimit: 30_000_000n,
    });
    if (!res.execResult.exceptionError) {
      const decoded = iface.decodeFunctionResult(r.fn, res.execResult.returnValue);
      throw new Error(
        `${spec.file}::${r.fn}(${JSON.stringify(r.args)}) expected revert, but succeeded with ${JSON.stringify(normalizeResult(decoded[0]))}`
      );
    }
    const rev = decodeRevert(iface, res.execResult.returnValue);
    if (r.expectReason !== undefined) {
      if (rev.kind !== 'Error' || !rev.reason.includes(r.expectReason)) {
        throw new Error(
          `${spec.file}::${r.fn}(${JSON.stringify(r.args)}) revert reason mismatch: got ${JSON.stringify(rev)}, expected reason containing ${JSON.stringify(r.expectReason)}`
        );
      }
    }
    if (r.expectPanic !== undefined) {
      if (rev.kind !== 'Panic' || rev.code !== r.expectPanic) {
        throw new Error(
          `${spec.file}::${r.fn}(${JSON.stringify(r.args)}) panic mismatch: got ${JSON.stringify(rev)}, expected Panic(${r.expectPanic})`
        );
      }
    }
    revertCount += 1;
  }

  return { callCount, revertCount };
}

(async () => {
  const plan = JSON.parse(fs.readFileSync(planPath, 'utf8'));
  let totalCalls = 0;
  let totalReverts = 0;
  for (const spec of plan.contracts) {
    const { callCount, revertCount } = await runContract(spec);
    totalCalls += callCount;
    totalReverts += revertCount;
  }
  console.log(
    `EVM-OK solc=${solc.version()} contracts=${plan.contracts.length} calls=${totalCalls} reverts=${totalReverts} warnings=0`
  );
})().catch((err) => {
  console.error('EVM-FAIL:', err && err.stack ? err.stack : String(err));
  process.exit(1);
});
