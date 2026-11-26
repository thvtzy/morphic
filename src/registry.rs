// ─────────────────────────────────────────────────────────────
//  THE REGISTRY CLIENT (Phase 164, 1.7 — DISEBAR_17_SCOPE §5)
//
//  The consumption half of registry (a): the `morphic
//  registry` verb family's library — add / list / remove /
//  verify over a catalog of PCC re-verification bundles
//  (src/pcc.rs), resolved against a static JSON index and
//  installed into a per-user directory as prelude entries
//  (discovery at the funnel is Phase 165's layer; this
//  module is the door entries arrive through).
//
//  THE INSTALL IS THE VERIFICATION (§5.4). `registry_add`
//  runs the frozen five steps, and each step's failure
//  writes NOTHING:
//
//    1. Resolve — fetch or read the index; find the
//       entry's row (absent, or a named version the index
//       does not carry → decline by name).
//    2. Fetch + bind — fetch the bundle bytes and check
//       their sha256 against the index row (the `sha2`
//       binding, §8: re-verification alone proves the
//       bytes are *a* proven function; only the hash
//       proves they are *the catalogued* one).
//    3. Parse + re-verify — `pcc::verify_bundle_text` on
//       the fetched bytes. Every standing PCC refusal is
//       the install's refusal, verbatim: unsupported
//       bundle version, incoherent claim, a body the
//       route does not crown, a moved battery count —
//       and in a no-Z3 build a Tier-1 bundle refused by
//       name (it stands unexamined; a Tier-2 bundle
//       installs on executed evidence).
//    4. Collision checks (§6) — a stdlib name refuses by
//       name (precedence user > stdlib > installed: an
//       installed bundle may never take a stdlib name);
//       an already-installed name refuses by name (the
//       update path is remove + add, so the ledger stays
//       an honest record of what was measured when).
//    5. Write + receipt — the bundle file lands in the
//       install directory; the receipt ledger gains a row
//       recording what the install's re-run MEASURED
//       (§5.3) — never the index's claims copied. (After a
//       successful `verify_bundle_text` the bundle's own
//       fields ARE the reproduced standings: the verb
//       compares the verdict and every battery count
//       against the reproduction, one by one.)
//
//  WHERE INSTALLS LIVE (§5.2): `~/.morphic/registry/`,
//  overridable by `MORPHIC_REGISTRY_DIR` — one directory,
//  one name; the verbs here and Phase 165's discovery
//  read the same override. Flat contents: one
//  `<entry>.bundle.json` per installed entry and one
//  `installed.json` receipt ledger. No other state.
//
//  FETCH SOURCES (§5.5): the default index is the
//  canonical URL (§3.3); `--index` accepts an HTTPS URL
//  or a local path, distinguished by scheme. The fetch
//  rides the standing `reqwest` (the `llm-client`
//  feature's client — no new HTTP dependency); a build
//  without it (the --no-default-features build) declines
//  a network fetch BY NAME while a local `--index
//  <path>` install works — the honest posture,
//  mirroring the no-Z3 Tier-1 refusal.
//
//  Native builds only, like the PCC surface it consumes:
//  the functions below are absent under the browser
//  profile (which carries no registry surface, §7), and
//  the CLI refuses by name there.
// ─────────────────────────────────────────────────────────────

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::pcc::BundleBattery;

/// The registry index format's name, carried in every
/// index (DISEBAR_17_SCOPE §3.2).
pub const INDEX_FORMAT: &str = "morphic-registry-index";

/// The index format's version. A reader refuses any
/// other version by name.
pub const INDEX_VERSION: u32 = 1;

/// The canonical index: the seed catalog's front door
/// (§3.3, R6). Any ref may be substituted in place of
/// `master` by naming another index.
pub const CANONICAL_INDEX_URL: &str =
    "https://raw.githubusercontent.com/thvtzy/morphic/master/registry/index.json";

/// The environment override for the install directory
/// (§5.2) — the one name the verbs and discovery read.
pub const REGISTRY_DIR_ENV: &str = "MORPHIC_REGISTRY_DIR";

/// The receipt ledger's file name inside the install
/// directory (§5.2).
pub const LEDGER_FILE: &str = "installed.json";

/// The ledger format's name, carried in the ledger file.
pub const LEDGER_FORMAT: &str = "morphic-registry-ledger";

// ── The hash (§8) ───────────────────────────────────────

/// The sha256 of `bytes`, lowercase hex — the install
/// flow's binding between an index row and the bytes it
/// catalogues. The product's own hash: before Phase 164
/// the tree had no SHA-256 anywhere and catalogue-time
/// hashes were computed with the system tool; the suites
/// pin this function against the catalogue's rows (and
/// the standard test vectors).
#[cfg(not(feature = "wasm-playground"))]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

// ── The index (§3.2) ────────────────────────────────────

/// One catalog row: the listing's facts about one
/// entry. `tier` / `verdict_basis` are labelled claims
/// (the bundle is the evidence); `sha256` is the
/// enforceable binding; `version` is the catalog's
/// fact — the bundle carries no version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub entry: String,
    pub version: String,
    /// The bundle's repo-relative path (R1).
    pub file: String,
    pub sha256: String,
    pub bytes: u64,
    pub tier: String,
    #[serde(rename = "verdictBasis")]
    pub verdict_basis: String,
}

/// The registry index: one JSON object, rows sorted by
/// entry (the catalogue discipline).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryIndex {
    pub format: String,
    #[serde(rename = "indexVersion")]
    pub index_version: u32,
    pub entries: Vec<IndexEntry>,
}

// ── The receipt (§5.3) ──────────────────────────────────

/// One install's receipt: what the install's re-run
/// measured — never what the index claimed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    /// The installed name.
    pub entry: String,
    /// The catalog version it was installed from.
    pub version: String,
    /// The installed file's hash (== the index row's,
    /// checked at install).
    pub sha256: String,
    /// As reproduced by the install's re-run.
    pub tier: String,
    #[serde(rename = "verdictBasis")]
    pub verdict_basis: String,
    pub verdict: String,
    #[serde(rename = "crownRoute")]
    pub crown_route: String,
    /// `{total, evaluable, passed}` as reproduced.
    pub battery: BundleBattery,
    /// The index the install resolved against (URL or
    /// path), as named on the command line.
    pub index: String,
    /// The install date (ISO, local clock's UTC day).
    pub installed: String,
}

/// The receipt ledger: `installed.json` in the install
/// directory, one row per install.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Ledger {
    format: String,
    entries: Vec<Receipt>,
}

impl Ledger {
    fn empty() -> Self {
        Ledger { format: LEDGER_FORMAT.to_string(), entries: Vec::new() }
    }
}

// ── Where installs live (§5.2) ──────────────────────────

/// The install directory: `MORPHIC_REGISTRY_DIR` when
/// set, else `~/.morphic/registry/` under the user's
/// home (HOME, falling back to USERPROFILE).
#[cfg(not(feature = "wasm-playground"))]
pub fn install_dir() -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var(REGISTRY_DIR_ENV) {
        if !dir.is_empty() {
            return Ok(PathBuf::from(dir));
        }
    }
    let home = std::env::var("HOME")
        .ok()
        .filter(|h| !h.is_empty())
        .or_else(|| std::env::var("USERPROFILE").ok().filter(|h| !h.is_empty()))
        .ok_or_else(|| {
            format!(
                "registry: no home directory (neither HOME nor USERPROFILE is \
                 set) — set {REGISTRY_DIR_ENV} to name the install directory"
            )
        })?;
    Ok(PathBuf::from(home).join(".morphic").join("registry"))
}

#[cfg(not(feature = "wasm-playground"))]
fn ledger_path(dir: &Path) -> PathBuf {
    dir.join(LEDGER_FILE)
}

#[cfg(not(feature = "wasm-playground"))]
fn read_ledger(dir: &Path) -> Result<Ledger, String> {
    let path = ledger_path(dir);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let ledger: Ledger = serde_json::from_str(&text).map_err(|e| {
                format!(
                    "registry: the receipt ledger at {} does not parse: {e} — \
                     it is the record of what installs measured; refusing \
                     rather than resetting it",
                    path.display()
                )
            })?;
            if ledger.format != LEDGER_FORMAT {
                return Err(format!(
                    "registry: the receipt ledger at {} carries format '{}', \
                     expected '{LEDGER_FORMAT}'",
                    path.display(),
                    ledger.format
                ));
            }
            Ok(ledger)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Ledger::empty()),
        Err(e) => Err(format!(
            "registry: cannot read the receipt ledger at {}: {e}",
            path.display()
        )),
    }
}

#[cfg(not(feature = "wasm-playground"))]
fn write_ledger(dir: &Path, ledger: &Ledger) -> Result<(), String> {
    let mut sorted = ledger.clone();
    sorted.entries.sort_by(|a, b| a.entry.cmp(&b.entry));
    let mut text = serde_json::to_string_pretty(&sorted)
        .map_err(|e| format!("registry: the receipt ledger does not serialize: {e}"))?;
    text.push('\n');
    std::fs::write(ledger_path(dir), text).map_err(|e| {
        format!(
            "registry: cannot write the receipt ledger at {}: {e}",
            ledger_path(dir).display()
        )
    })
}

// ── Fetch (§5.5) ────────────────────────────────────────

/// Where an index (and, by resolution, its bundles)
/// comes from: an HTTP(S) URL or a local path,
/// distinguished by scheme.
#[cfg(not(feature = "wasm-playground"))]
#[derive(Debug, Clone)]
enum IndexSource {
    Url(String),
    Local(PathBuf),
}

#[cfg(not(feature = "wasm-playground"))]
fn classify_index(spec: &str) -> IndexSource {
    if spec.starts_with("https://") || spec.starts_with("http://") {
        IndexSource::Url(spec.to_string())
    } else {
        IndexSource::Local(PathBuf::from(spec))
    }
}

/// Fetch one URL's bytes. The client is the standing
/// `reqwest` (the `llm-client` feature's dependency —
/// §5.5: no new HTTP dependency); a build without it
/// declines a network fetch by name, below.
#[cfg(all(not(feature = "wasm-playground"), feature = "llm-client"))]
fn fetch_url(url: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("registry: cannot build the HTTP client: {e}"))?;
    let response = client
        .get(url)
        .send()
        .map_err(|e| format!("registry: fetching {url} failed: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "registry: fetching {url} answered HTTP {status} — refusing"
        ));
    }
    response
        .bytes()
        .map(|b| b.to_vec())
        .map_err(|e| format!("registry: reading the answer from {url} failed: {e}"))
}

#[cfg(all(not(feature = "wasm-playground"), not(feature = "llm-client")))]
fn fetch_url(url: &str) -> Result<Vec<u8>, String> {
    let _ = url;
    Err(
        "registry: network fetch is unavailable in this build — it carries \
         no HTTP client (the --no-default-features build); pass a local \
         --index <path> instead"
            .to_string(),
    )
}

/// The repo root a local index's rows resolve against
/// (R1): the parent of the `registry/` directory holding
/// the index file — or, for an index file not in a
/// `registry/` directory, that directory itself.
#[cfg(not(feature = "wasm-playground"))]
fn local_root(index_path: &Path) -> PathBuf {
    let dir = index_path.parent().unwrap_or_else(|| Path::new("."));
    if dir.file_name().map(|n| n == "registry").unwrap_or(false) {
        dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| dir.to_path_buf())
    } else {
        dir.to_path_buf()
    }
}

/// The repo root a URL index's rows resolve against
/// (R1): the index URL with its `registry/<file>`
/// tail removed, keeping the root's trailing slash.
#[cfg(not(feature = "wasm-playground"))]
fn url_root(index_url: &str) -> String {
    let no_file = match index_url.rfind('/') {
        Some(i) => &index_url[..=i],
        None => index_url,
    };
    match no_file.strip_suffix("registry/") {
        Some(root) => root.to_string(),
        None => no_file.to_string(),
    }
}

/// Fetch one bundle's bytes by its index row, from the
/// same source the index came from.
#[cfg(not(feature = "wasm-playground"))]
fn fetch_bundle(source: &IndexSource, row: &IndexEntry) -> Result<Vec<u8>, String> {
    match source {
        IndexSource::Url(index_url) => {
            let url = format!("{}{}", url_root(index_url), row.file);
            fetch_url(&url).map_err(|e| {
                format!("registry: fetching the bundle for '{}': {e}", row.entry)
            })
        }
        IndexSource::Local(index_path) => {
            let path = local_root(index_path).join(&row.file);
            std::fs::read(&path).map_err(|e| {
                format!(
                    "registry: cannot read the bundle for '{}' at {}: {e}",
                    row.entry,
                    path.display()
                )
            })
        }
    }
}

/// Load + parse an index from a named source (the
/// canonical URL when `spec` is None), checking the
/// format's name and version by name.
#[cfg(not(feature = "wasm-playground"))]
fn load_index(
    spec: Option<&str>,
) -> Result<(RegistryIndex, IndexSource, String), String> {
    let display = spec.unwrap_or(CANONICAL_INDEX_URL).to_string();
    let source = classify_index(&display);
    let bytes = match &source {
        IndexSource::Url(url) => fetch_url(url)?,
        IndexSource::Local(path) => std::fs::read(path).map_err(|e| {
            format!("registry: cannot read the index at {}: {e}", path.display())
        })?,
    };
    let text = String::from_utf8(bytes).map_err(|_| {
        format!("registry: the index at {display} is not UTF-8 text — refusing")
    })?;
    let index: RegistryIndex = serde_json::from_str(&text).map_err(|e| {
        format!("registry: the index at {display} does not parse: {e}")
    })?;
    if index.format != INDEX_FORMAT {
        return Err(format!(
            "registry: the index at {display} is not a {INDEX_FORMAT} — its \
             format is '{}'",
            index.format
        ));
    }
    if index.index_version != INDEX_VERSION {
        return Err(format!(
            "registry: unsupported index version {} at {display} — this build \
             reads index version {INDEX_VERSION}",
            index.index_version
        ));
    }
    Ok((index, source, display))
}

// ── Dates ───────────────────────────────────────────────

/// Today's date, ISO form, from the system clock's UTC
/// day (the civil-from-days conversion; no date
/// dependency enters for one field).
#[cfg(not(feature = "wasm-playground"))]
fn today() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_secs() / 86_400) as i64)
        .unwrap_or(0);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }
    format!("{year:04}-{month:02}-{day:02}")
}

// ── add (§5.4) ──────────────────────────────────────────

/// Install one catalogued entry: `name` optionally
/// carrying `@<version>`, resolved against the index
/// named by `index_spec` (the canonical index by
/// default). The five frozen steps (§5.4); each step's
/// failure writes nothing. Ok carries the one-line
/// summary of what was installed, at its measured
/// standing.
#[cfg(not(feature = "wasm-playground"))]
pub fn registry_add(
    name_spec: &str,
    index_spec: Option<&str>,
) -> Result<String, String> {
    // The name[@version] form.
    let (name, want_version) = match name_spec.split_once('@') {
        Some((n, v)) => {
            if n.is_empty() || v.is_empty() || v.contains('@') {
                return Err(format!(
                    "registry: '{name_spec}' is not a name or name@version"
                ));
            }
            (n, Some(v))
        }
        None => (name_spec, None),
    };
    if name.is_empty() {
        return Err("registry: no entry name given".to_string());
    }

    // Step 1 — resolve.
    let (index, source, display) = load_index(index_spec)?;
    let row = index
        .entries
        .iter()
        .find(|e| e.entry == name)
        .ok_or_else(|| {
            format!("registry: the index at {display} carries no entry named '{name}'")
        })?;
    if let Some(v) = want_version {
        if row.version != v {
            return Err(format!(
                "registry: the index carries '{name}' at version {}, not {v}",
                row.version
            ));
        }
    }

    // Step 2 — fetch + bind (the hash is load-bearing:
    // only it binds name → the catalogued bytes).
    let bytes = fetch_bundle(&source, row)?;
    let digest = sha256_hex(&bytes);
    if digest != row.sha256 {
        return Err(format!(
            "registry: sha256 mismatch for '{name}': the index binds {}, but \
             the fetched bytes hash to {digest} — refusing, nothing is \
             installed",
            row.sha256
        ));
    }

    // Step 3 — parse + re-verify. The PCC verb's
    // refusals are the install's refusals, verbatim.
    let text = String::from_utf8(bytes).map_err(|_| {
        format!(
            "registry: the bundle fetched for '{name}' is not UTF-8 bundle \
             text — refusing"
        )
    })?;
    let bundle: crate::pcc::Bundle = serde_json::from_str(&text).map_err(|e| {
        format!("registry: the bundle fetched for '{name}' does not parse: {e}")
    })?;
    if bundle.entry != name {
        return Err(format!(
            "registry: the bundle fetched for '{name}' carries entry '{}' — \
             the catalogued bundle for a name is a bundle of that name; \
             refusing",
            bundle.entry
        ));
    }
    crate::pcc::verify_bundle_text(&text)?;

    // Step 4 — collision checks (§6).
    if crate::stdlib::prelude_specs().iter().any(|s| s.name == name) {
        return Err(format!(
            "registry: '{name}' is a standard-library entry — precedence \
             user > stdlib > installed means an installed bundle may never \
             take a stdlib name; refusing (the prelude already carries \
             '{name}')"
        ));
    }
    let dir = install_dir()?;
    let mut ledger = read_ledger(&dir)?;
    if let Some(prev) = ledger.entries.iter().find(|r| r.entry == name) {
        return Err(format!(
            "registry: '{name}' is already installed (version {}, installed \
             {}) — remove it first (morphic registry remove {name}); refusing",
            prev.version, prev.installed
        ));
    }
    let file_path = dir.join(format!("{name}.bundle.json"));
    if file_path.exists() {
        return Err(format!(
            "registry: a bundle file for '{name}' already sits in the \
             install directory with no receipt — remove it first (morphic \
             registry remove {name}); refusing"
        ));
    }

    // Step 5 — write + receipt. The receipt records
    // what the re-run measured: after a successful
    // re-verification the bundle's fields ARE the
    // reproduced standings (the verb compared the
    // verdict and every battery count, one by one).
    std::fs::create_dir_all(&dir).map_err(|e| {
        format!("registry: cannot create the install directory {}: {e}", dir.display())
    })?;
    std::fs::write(&file_path, &text).map_err(|e| {
        format!("registry: cannot write the bundle file {}: {e}", file_path.display())
    })?;
    let receipt = Receipt {
        entry: name.to_string(),
        version: row.version.clone(),
        sha256: digest.clone(),
        tier: bundle.tier.clone(),
        verdict_basis: bundle.verdict_basis.clone(),
        verdict: bundle.verdict.clone(),
        crown_route: bundle.crown_route.clone(),
        battery: bundle.battery,
        index: display,
        installed: today(),
    };
    ledger.entries.push(receipt);
    write_ledger(&dir, &ledger)?;
    Ok(format!(
        "installed {name} {} — {} / {} / {} via the {} route; battery {} \
         passed of {} evaluable ({} in spec); sha256 {}…",
        row.version,
        bundle.tier,
        bundle.verdict_basis,
        bundle.verdict,
        bundle.crown_route,
        bundle.battery.passed,
        bundle.battery.evaluable,
        bundle.battery.total,
        &digest[..12]
    ))
}

// ── list (§5.1) ─────────────────────────────────────────

/// The installed entries, from the receipt ledger —
/// each at the standing its install measured.
#[cfg(not(feature = "wasm-playground"))]
pub fn registry_list() -> Result<Vec<Receipt>, String> {
    let dir = install_dir()?;
    Ok(read_ledger(&dir)?.entries)
}

// ── remove (§5.1) ───────────────────────────────────────

/// Remove one installed entry: its bundle file + its
/// receipt row. Nothing else references the install
/// directory, so removal is complete by construction.
/// Removing an absent name declines by name.
#[cfg(not(feature = "wasm-playground"))]
pub fn registry_remove(name: &str) -> Result<String, String> {
    let dir = install_dir()?;
    let mut ledger = read_ledger(&dir)?;
    let row_at = ledger.entries.iter().position(|r| r.entry == name);
    let file_path = dir.join(format!("{name}.bundle.json"));
    let file_present = file_path.exists();
    if row_at.is_none() && !file_present {
        return Err(format!(
            "registry: '{name}' is not installed — nothing to remove"
        ));
    }
    if file_present {
        std::fs::remove_file(&file_path).map_err(|e| {
            format!(
                "registry: cannot remove the bundle file {}: {e}",
                file_path.display()
            )
        })?;
    }
    if let Some(at) = row_at {
        let receipt = ledger.entries.remove(at);
        write_ledger(&dir, &ledger)?;
        Ok(format!(
            "removed {name} {} (installed {}; sha256 {}…)",
            receipt.version,
            receipt.installed,
            &receipt.sha256[..12]
        ))
    } else {
        Ok(format!(
            "removed {name} — a receipt-less bundle file; it was never an \
             install"
        ))
    }
}

// ── verify (§5.1) ───────────────────────────────────────

/// Re-judge one installed entry against its receipt:
/// the file's hash must be the receipt's (a file edited
/// after install fails here, by name), the claim must
/// reproduce under `verify_bundle_text`, and the
/// reproduced standing must equal the receipt's recorded
/// standing field for field.
#[cfg(not(feature = "wasm-playground"))]
fn verify_receipt(dir: &Path, receipt: &Receipt) -> Result<String, String> {
    let entry = &receipt.entry;
    let file_path = dir.join(format!("{entry}.bundle.json"));
    let bytes = std::fs::read(&file_path).map_err(|e| {
        format!(
            "registry: '{entry}' is recorded as installed but its bundle \
             file at {} cannot be read: {e}",
            file_path.display()
        )
    })?;
    let digest = sha256_hex(&bytes);
    if digest != receipt.sha256 {
        return Err(format!(
            "registry: '{entry}': the installed file's sha256 is {digest}, \
             but the receipt records {} — the file changed after install; \
             refusing",
            receipt.sha256
        ));
    }
    let text = String::from_utf8(bytes).map_err(|_| {
        format!("registry: '{entry}': the installed bundle is not UTF-8 text")
    })?;
    crate::pcc::verify_bundle_text(&text)?;
    let bundle: crate::pcc::Bundle = serde_json::from_str(&text).map_err(|e| {
        format!("registry: '{entry}': the installed bundle does not parse: {e}")
    })?;
    if bundle.tier != receipt.tier
        || bundle.verdict_basis != receipt.verdict_basis
        || bundle.verdict != receipt.verdict
        || bundle.crown_route != receipt.crown_route
        || bundle.battery != receipt.battery
    {
        return Err(format!(
            "registry: '{entry}': the installed bundle's standing ({} / {} / \
             {} via {}, battery {} passed of {} evaluable of {}) differs \
             from the receipt's recorded standing ({} / {} / {} via {}, \
             battery {} passed of {} evaluable of {}) — refusing",
            bundle.tier,
            bundle.verdict_basis,
            bundle.verdict,
            bundle.crown_route,
            bundle.battery.passed,
            bundle.battery.evaluable,
            bundle.battery.total,
            receipt.tier,
            receipt.verdict_basis,
            receipt.verdict,
            receipt.crown_route,
            receipt.battery.passed,
            receipt.battery.evaluable,
            receipt.battery.total
        ));
    }
    Ok(format!(
        "{entry}: the installed bundle reproduces its receipt — {} / {} / {} \
         via the {} route; battery {} passed of {} evaluable ({} in spec)",
        receipt.tier,
        receipt.verdict_basis,
        receipt.verdict,
        receipt.crown_route,
        receipt.battery.passed,
        receipt.battery.evaluable,
        receipt.battery.total
    ))
}

/// Verify one installed entry by name, or — with no
/// name — sweep the whole install directory: every
/// receipt re-judged, and every bundle file without a
/// receipt reported as a named refusal (it is not an
/// install and cannot be verified as one). This is the
/// `verify_library` sibling (§5.1): it iterates the
/// install dir calling the per-bundle verb, exactly as
/// `verify_library` iterates the shipped inputs calling
/// the crown checks. Ok carries one line per entry.
#[cfg(not(feature = "wasm-playground"))]
pub fn registry_verify(name: Option<&str>) -> Result<Vec<String>, String> {
    let dir = install_dir()?;
    let ledger = read_ledger(&dir)?;
    if let Some(name) = name {
        if let Some(receipt) = ledger.entries.iter().find(|r| r.entry == name) {
            return verify_receipt(&dir, receipt).map(|line| vec![line]);
        }
        if dir.join(format!("{name}.bundle.json")).exists() {
            return Err(format!(
                "registry: a bundle file for '{name}' sits in the install \
                 directory with no receipt — it is not an install and \
                 cannot be verified as one"
            ));
        }
        return Err(format!(
            "registry: '{name}' is not installed — nothing to verify"
        ));
    }
    let mut lines = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    for receipt in &ledger.entries {
        match verify_receipt(&dir, receipt) {
            Ok(line) => lines.push(line),
            Err(reason) => failures.push(reason),
        }
    }
    // Stray bundle files: in the directory, but not
    // installs — named, never silently passed over.
    if let Ok(read) = std::fs::read_dir(&dir) {
        for entry in read.flatten() {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if let Some(stem) = file_name.strip_suffix(".bundle.json") {
                if !ledger.entries.iter().any(|r| r.entry == stem) {
                    failures.push(format!(
                        "registry: a bundle file '{file_name}' sits in the \
                         install directory with no receipt — it is not an \
                         install and cannot be verified as one"
                    ));
                }
            }
        }
    }
    if failures.is_empty() {
        Ok(lines)
    } else {
        Err(failures.join("\n"))
    }
}
