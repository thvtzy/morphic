# Morphic functions on Cloudflare Workers

Serve Morphic's proven functions at the edge: the shipped
package [`@thvtzy/morphic`](https://www.npmjs.com/package/@thvtzy/morphic)
— functions the Morphic pipeline **verified** (Tier-1) or
proved **executed-exact** (Tier-2) — behind one small
fetch handler. Copy this directory, install, run.

## Copy steps

```sh
cp -R templates/cloudflare-workers my-worker
cd my-worker
npm install
npx wrangler dev      # local, on your machine
```

`wrangler` is not a dependency of this template or of the
Morphic repo — `npx` fetches it when you run it.

## Deploy — your account act

```sh
npx wrangler deploy
```

Deploying needs **your** Cloudflare account, your login,
and your route/DNS choices. Nothing in the Morphic repo
deploys anything, and no Morphic phase touches a
Cloudflare account. What this template gives you is the
code and config; the deploy — and any live evidence from
it (cold start, real-account behaviour) — is yours.

## Calling it

One route: `POST /fn/<name>`, where `<name>` is any export
of the package (`int_clamp`, `list_sort`, `range`, …).
The body is a JSON object carrying the argument list:

```sh
curl -X POST https://<your-worker>/fn/int_clamp \
  -H 'content-type: application/json' \
  -d '{"args": [15, 0, 10]}'
# → {"result": "10"}

curl -X POST https://<your-worker>/fn/list_sort \
  -H 'content-type: application/json' \
  -d '{"args": [[3, 1, 2]]}'
# → {"result": ["1", "2", "3"]}
```

`GET /` answers a small service note. Errors are answered,
never thrown across the fetch boundary, in one named
shape:

```json
{"error": {"code": "unknown_function", "message": "the Morphic package has no export named 'nope'"}}
```

with codes `unknown_route` (404), `unknown_function`
(404), `method_not_allowed` (405), `malformed_request`
(400), `invalid_arguments` (400), and `internal_error`
(500).

## The BigInt / JSON boundary — read this once

Morphic `Int` is a JavaScript **BigInt** end-to-end; JSON
cannot carry BigInt. The handler ships the boundary as
code (it is not a warning you have to remember):

- **In:** send Int arguments as JSON numbers (the package
  has no Float exports, so a JSON number here is an Int).
  The handler converts numbers to BigInt — recursively
  through arrays and Result values — before calling the
  function. Strings, booleans, and `null` pass through
  as themselves (Char/String, Bool, and `None`).
- **Out:** BigInt results come back as **decimal
  strings** (`"10"`, not `10`), recursively. A client
  that needs the number back does `BigInt(result)` on
  its side. Option results are the value itself or
  `null`; Result results keep their
  `{tag: "Ok", value}` / `{tag: "Err", error}` shape.

One consequence, stated plainly: **callback-taking
exports cannot be called over JSON.** `list_map`,
`list_filter`, `list_fold`, `list_all`, and `list_any`
take a function argument, and no JSON body can carry
one. Code running *inside* the worker can import the
package and supply a callback in-process — the
template's own smoke test does exactly that through the
handler — but the fetch route serves data arguments.

## Tiers — what "proven" means here

Every export's manifest row
(`@thvtzy/morphic/morphic.manifest.json`) states its
tier: **Tier-1** functions are Z3-verified against their
specifications; **Tier-2** functions (`list_sort`,
`list_binary_search`) are executed-exact on their full
frozen batteries, and are named Tier-2 everywhere they
appear. The manifest row is a claim; its evidence is the
re-verification bundle shipped with the Morphic repo
(`packages/bundles/`), re-judgeable with
`morphic verify-bundle` / `morphic verify --library`.

## Honest limits

- One function per request; no batching, no streaming,
  no authentication — put the worker behind your own
  gateway if you need those.
- The whole package is plain ESM with zero dependencies
  and no Node builtins (17,278 B of JS), which is why it
  fits a Workers isolate with room to spare; it also
  loads in a bare V8 context with no platform globals at
  all — the local sandbox smoke pinned in the Morphic
  repo (`tests/phase153_edge_template.rs`) is the
  evidence for that claim, and it is a proxy: it does
  not measure workerd cold start.
