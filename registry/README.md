# The Morphic registry — a catalog of proven functions

This directory is the **1.7 registry** (DISEBAR_17_SCOPE.md):
a catalog of *prelude bundles* — one function's spec +
body in the PCC re-verification format
(`morphic-ir/1`), plus its claimed verdict, tier,
battery and crown route — that a consumer can fetch,
install as prelude entries their own specs call by
plain name, and **re-verify at install time** before
anything is trusted.

## Layout

- `index.json` — the catalog's listing: one row per
  entry (`entry`, `version`, `file`, `sha256`,
  `bytes`, `tier`, `verdictBasis`). The index is
  served over HTTPS raw from this repo:
  `https://raw.githubusercontent.com/thvtzy/morphic/master/registry/index.json`
  (any ref — branch, tag, commit — may be named in
  place of `master`).
- `bundles/<entry>.bundle.json` — the bundle files
  themselves, one per entry, named by the index's
  `file` rows (repo-relative paths).

The seed catalog (37 entries) is the 35 standard
library bundles — byte-identical copies of the files
at `packages/bundles/`, the same artifact in its
fetchable home — plus two worked examples produced
from a user module by the production verb:
`is_widget_code` (Tier-1) and `weights_sorted`
(Tier-2).

## How a bundle gets in

Publishing is a **pull request** to this repo — the
only review flow that exists at this scale. Curation
is a *listing* decision, never a trust decision (see
the trust model below).

1. Produce your bundle with the shipped binary:

   ```
   morphic build --bundles-from your_module.morph
   ```

   The verb runs the seeded admission over your
   module (measured rows only — a spec whose crown
   stands neither Verified nor executed-exact is
   refused by name) and emits one
   `<entry>.bundle.json` per admitted spec. A bundle
   is **one prelude-self-contained function**: a spec
   whose constraints name a sibling spec of your
   module refuses at production, by name — its spec
   and body may name builtins and prelude entries,
   nothing else.
2. Add the bundle file under `bundles/` and a row to
   `index.json`: your entry's name, its catalog
   version, the repo-relative `file` path, the
   bundle file's **sha256** and byte count, and the
   tier / verdictBasis the bundle itself states.
   Entry names are a flat namespace, first-come: a
   name enters the index once.
3. CI re-verifies the catalog (every row's file
   exists, hashes to the row's sha256, and reproduces
   under the shipped binary), so a lying or stale
   bundle fails before merge.

## The trust model

The index's tier and verdictBasis fields are
**labels — claims**, duplicated from the bundle so a
human can browse. The bundle is the evidence, and
nobody has to take either one's word for it: install
(`morphic registry add`, Phase 164) fetches the
bundle, checks its **sha256 against the index row**
— re-verification alone proves the bytes are *a*
proven function; only the hash binds them as *the
catalogued* one — and then **re-runs the bundle's
proof obligation** (`verify_bundle_text`) before
writing anything. A substituted, tampered or stale
bundle is refused by name at install, and the
install's receipt records what the re-run *measured*,
not what the index claimed. A hostile mirror can
serve a wrong bundle; it cannot serve a trusted one.
