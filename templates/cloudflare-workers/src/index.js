// Morphic at the edge — a Cloudflare Workers template.
//
// A module worker whose fetch handler routes
// `POST /fn/<name>` to the named export of the shipped
// Morphic package (`@thvtzy/morphic`): functions the
// Morphic pipeline verified (Tier-1) or proved
// executed-exact (Tier-2), served as plain JavaScript.
//
// The handler and wrangler.toml are this template's only
// platform-shaped code. Everything below the routing is
// the one boundary the package's embedding guide names:
// JSON cannot carry BigInt, and Morphic Int is BigInt
// end-to-end — so the boundary ships as code (§3.2 of
// DISEBAR_15_SCOPE.md), not as a warning.

import * as morphic from "@thvtzy/morphic";

const JSON_HEADERS = { "content-type": "application/json; charset=utf-8" };

class RequestProblem extends Error {
  constructor(status, code, message) {
    super(message);
    this.status = status;
    this.code = code;
  }
}

function problem(status, code, message) {
  return new RequestProblem(status, code, message);
}

// ── The JSON ⇄ BigInt boundary ──────────────────────────
//
// In: a JSON number at this boundary is a Morphic Int
// (the package has no Float exports), so numbers decode
// to BigInt — recursively through arrays and through
// Result values ({tag, value|error}). Strings, booleans,
// null, and functions (a host-supplied callback, in
// process) pass through untouched.
// Out: BigInts serialize as decimal strings, recursively;
// everything else keeps its JSON shape (Option is the
// value itself or null; Result keeps its tag object).

function decodeValue(value) {
  if (typeof value === "number") {
    if (!Number.isInteger(value)) {
      throw problem(
        400,
        "invalid_arguments",
        `expected an integer argument, got the non-integer number ${value}`,
      );
    }
    return BigInt(value);
  }
  if (Array.isArray(value)) {
    return value.map(decodeValue);
  }
  if (value !== null && typeof value === "object") {
    const out = {};
    for (const [key, inner] of Object.entries(value)) {
      out[key] = decodeValue(inner);
    }
    return out;
  }
  return value;
}

function encodeValue(value) {
  if (typeof value === "bigint") {
    return value.toString();
  }
  if (Array.isArray(value)) {
    return value.map(encodeValue);
  }
  if (value !== null && typeof value === "object") {
    const out = {};
    for (const [key, inner] of Object.entries(value)) {
      out[key] = encodeValue(inner);
    }
    return out;
  }
  return value === undefined ? null : value;
}

// ── Responses ───────────────────────────────────────────
//
// On Workers the answer is a native Response. In a bare
// sandbox with no platform globals (the template's local
// smoke), the same payload returns as a plain
// {status, headers, body} descriptor — the bytes a
// Response would carry.

function respond(status, payload) {
  const body = JSON.stringify(payload);
  if (typeof Response === "function") {
    return new Response(body, { status, headers: JSON_HEADERS });
  }
  return { status, headers: { ...JSON_HEADERS }, body };
}

function errorResponse(status, code, message) {
  return respond(status, { error: { code, message } });
}

// ── Request reading ─────────────────────────────────────
//
// A real Request is read through .json(); a synthetic
// fetch event in the local smoke may carry an already
// decoded body object (the only way a host callback can
// ride along — JSON itself cannot carry a function).

async function readBody(request) {
  try {
    if (typeof request.json === "function") {
      const data = await request.json();
      return typeof data === "string" ? JSON.parse(data) : data;
    }
    if (typeof request.text === "function") {
      return JSON.parse(await request.text());
    }
    if (typeof request.body === "string") {
      return JSON.parse(request.body);
    }
    if (request.body !== null && typeof request.body === "object") {
      return request.body;
    }
    if (Array.isArray(request.args)) {
      return request;
    }
  } catch (err) {
    if (err instanceof RequestProblem) throw err;
    throw problem(400, "malformed_request", `request body is not valid JSON: ${err.message}`);
  }
  throw problem(400, "malformed_request", "request carries no readable body");
}

function routeOf(url) {
  const path = String(url).replace(/^[A-Za-z][A-Za-z0-9+.-]*:\/\/[^/]+/, "");
  return path.split(/[?#]/, 1)[0];
}

// ── The handler ─────────────────────────────────────────

export async function handleRequest(request) {
  try {
    const route = routeOf(request && request.url);
    if (route === "/" || route === "") {
      return respond(200, {
        service: "morphic-functions",
        usage: "POST /fn/<name> with a JSON body {\"args\": [...]}",
      });
    }
    const match = /^\/fn\/([A-Za-z_][A-Za-z0-9_]*)\/?$/.exec(route);
    if (!match) {
      throw problem(404, "unknown_route", `no route matches ${route || "(empty)"}`);
    }
    if (String(request.method || "GET").toUpperCase() !== "POST") {
      throw problem(405, "method_not_allowed", `route ${route} answers POST only`);
    }
    const name = match[1];
    const fn = morphic[name];
    if (typeof fn !== "function") {
      throw problem(404, "unknown_function", `the Morphic package has no export named '${name}'`);
    }
    const body = await readBody(request);
    if (body === null || typeof body !== "object" || !Array.isArray(body.args)) {
      throw problem(
        400,
        "malformed_request",
        "request body must be a JSON object of the form {\"args\": [...]}",
      );
    }
    let result;
    try {
      result = await fn(...body.args.map(decodeValue));
    } catch (err) {
      if (err instanceof RequestProblem) throw err;
      throw problem(400, "invalid_arguments", `'${name}' rejected its arguments: ${err.message}`);
    }
    return respond(200, { result: encodeValue(result) });
  } catch (err) {
    if (err instanceof RequestProblem) {
      return errorResponse(err.status, err.code, err.message);
    }
    return errorResponse(500, "internal_error", `unexpected handler failure: ${err.message}`);
  }
}

export default {
  fetch(request, _env, _ctx) {
    return handleRequest(request);
  },
};
