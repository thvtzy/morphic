// ┌────────────────────────────────────────────────────────────┐
// │  MORPHIC — Self-Synthesizing Programming Language         │
// │  "Write specs. The compiler writes the implementation."   │
// │                                                            │
// │  CLI shell (Phase 66, v0.6 W1): the compiler core lives   │
// │  in the library (src/lib.rs); this binary is argument     │
// │  parsing, the filesystem contract, and presentation       │
// │  over it. The core modules are re-exported at this root   │
// │  so the test/measurement modules declared below keep      │
// │  their `crate::…` paths unchanged. The `llm` module       │
// │  stays in this target: nothing in the pipeline imports    │
// │  it, and its HTTP client must never enter the library's   │
// │  (hence the browser's) dependency graph.                  │
// └────────────────────────────────────────────────────────────┘

pub use morphic::{codegen, spec, synthesis, verify};

#[cfg(feature = "llm-client")]
mod llm;
mod m2_synthesis_probe;
mod m2_cross_validation;
mod m3_synthesis_probe;
mod m3_cross_validation;
mod tokenizer_synthesis_probe;
mod tokenizer_cross_validation;
mod tokenizer_benchmark;
mod tokenizer_profile;
mod phase53_measurement;
// The Phase 58–62 measurement instruments measure the
// compiled-runner stage; without that stage (browser
// profile) they have nothing to measure and are not
// compiled.
#[cfg(feature = "compiled-runner")]
mod phase58_measurement;
#[cfg(feature = "compiled-runner")]
mod phase59_measurement;
#[cfg(feature = "compiled-runner")]
mod phase60_measurement;
#[cfg(feature = "compiled-runner")]
mod phase61_measurement;
#[cfg(feature = "compiled-runner")]
mod phase62_measurement;


use clap::{Parser, Subcommand};
use colored::*;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "morphic")]
#[command(about = "Self-Synthesizing Programming Language Compiler")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compile a Morphic specification into an optimized implementation
    Build {
        /// Path to .morph spec file
        input: Option<PathBuf>,

        /// Assemble the npm package (@thvtzy/morphic) from
        /// the standard library's shipped bodies instead of
        /// compiling a spec file (Phase 122: the assembly
        /// re-runs the crowning checks and refuses on a
        /// moved crown; takes no input file)
        #[arg(long)]
        package: bool,

        /// Assemble the Rust facade crate (crates.io
        /// `morphic`) from the standard library's shipped
        /// bodies instead of compiling a spec file
        /// (Phase 123: the assembly re-runs the crowning
        /// checks and refuses on a moved crown; takes no
        /// input file)
        #[arg(long)]
        facade: bool,

        /// Produce the re-verification bundles
        /// (Phase 150, DISEBAR_SCOPE §4): one bundle per
        /// standard-library entry — spec + body in IR text
        /// + claimed verdict/tier + battery + crown route —
        /// through the checked route (the crowning surface
        /// re-runs first, as assembly does). Written to
        /// packages/bundles, a sibling of both packages,
        /// in neither package's file set. Takes no input
        /// file.
        #[arg(long)]
        bundles: bool,

        /// Assemble an npm package from a USER module's
        /// specs instead of compiling one spec (Phase 154,
        /// DISEBAR_15_SCOPE §4): each spec is synthesized
        /// by the seeded bounded LLM-free profile and
        /// admitted only on the rows that run itself
        /// measures — a spec whose crown stands neither
        /// Verified nor executed-exact is refused by name
        /// and nothing is written. No flag supplies a
        /// standing; the run is the only source of one.
        #[arg(long)]
        package_from: Option<PathBuf>,

        /// Produce re-verification bundles from a USER
        /// module's specs instead of compiling one spec
        /// (Phase 163, DISEBAR_17_SCOPE §4): the
        /// --package-from admission loop (seeded profile,
        /// measured rows only, the same refusal shapes),
        /// emitting one PCC bundle per admitted spec
        /// through produce_bundle's checked route. A spec
        /// whose constraints name a sibling spec refuses
        /// the whole production by name — a bundle is one
        /// prelude-self-contained function. No flag
        /// supplies a standing; the run is the only
        /// source of one.
        #[arg(long)]
        bundles_from: Option<PathBuf>,

        /// Package name for --package-from
        /// (default: the module file's stem)
        #[arg(long)]
        package_name: Option<String>,

        /// Package version for --package-from
        /// (default: this Morphic version)
        #[arg(long)]
        package_version: Option<String>,

        /// Seed for the --package-from / --bundles-from
        /// admission runs (default: 42, the standing
        /// seeded discipline)
        #[arg(long)]
        seed: Option<u64>,

        /// Directory the assembled package is written to
        /// (with --package or --facade; default: the
        /// package's repo home, packages/npm resp.
        /// packages/rust)
        #[arg(long)]
        out: Option<PathBuf>,

        /// Output language (rust, c, wasm, python, javascript, solidity, ir)
        #[arg(short, long, default_value = "rust")]
        target: String,

        /// Optimization level (1-4)
        #[arg(short = 'O', long, default_value = "3")]
        opt_level: u8,

        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Verify a spec without generating implementation
    Check {
        /// Path to .morph spec file
        input: PathBuf,
    },

    /// Re-verify a proof-carrying bundle (Phase 150,
    /// DISEBAR_SCOPE §4): re-run the crown by the bundle's
    /// stated route from (spec, body) alone, re-run the
    /// executed battery, and answer whether the claimed
    /// verdict reproduces. Tamper fails by name.
    VerifyBundle {
        /// Path to the .bundle.json file
        file: PathBuf,
    },

    /// The standard-library self-check (Phase 150,
    /// DISEBAR_SCOPE §4.3): re-verify every shipped
    /// entry's crown from the shipped data alone and
    /// report per-entry standings. (`verify` with no
    /// target refuses by name — spec-file verification
    /// is `check` / the synthesis path.)
    Verify {
        /// Re-verify the whole shipped standard library
        #[arg(long)]
        library: bool,
    },

    /// The registry client (Phase 164, DISEBAR_17_SCOPE
    /// §5): fetch catalogued re-verification bundles and
    /// install them as prelude entries — the install
    /// re-runs the bundle's proof obligation (sha256
    /// against the index, then `verify-bundle`'s
    /// re-judgment) before anything is written, and the
    /// receipt records what the re-run measured.
    Registry {
        #[command(subcommand)]
        action: RegistryAction,
    },

    /// Synthesize interactively — see candidates in real-time
    Synthesize {
        /// Path to .morph spec file
        input: PathBuf,

        /// Maximum number of synthesis iterations
        #[arg(short, long, default_value = "10000")]
        iterations: usize,
    },

    /// Serve the Morphic language server (LSP,
    /// diagnostics-only) over stdio
    Lsp,

    /// Serve the Model Context Protocol over stdio
    /// (Phase 149, DISEBAR_SCOPE §3): the seeded,
    /// bounded, LLM-free tool surface over the compiler's
    /// DTO API — check / synthesize / verify / library.
    /// Ships only in the Z3-enabled native build.
    Mcp,

    /// Initialize a new Morphic project
    Init {
        /// Project name
        name: String,
    },
    /// Print the standard library as a JSON asset
    /// (playground build tooling, Phase 112: regenerates
    /// playground/stdlib-asset.bin from the embedded
    /// library data)
    #[command(hide = true)]
    StdlibAsset,
}

/// The registry verb family (Phase 164, DISEBAR_17_SCOPE
/// §5.1): one subcommand group for the catalog's client,
/// keeping the flat top level for the compiler's own
/// verbs.
#[derive(Subcommand)]
enum RegistryAction {
    /// Install a catalogued bundle by name (optionally
    /// name@version): resolve against the index, fetch,
    /// sha256-check, re-verify, then write + receipt
    Add {
        /// Entry name, optionally @<catalog version>
        name: String,

        /// The index to resolve against: an HTTPS URL or
        /// a local path (default: the canonical catalog
        /// index)
        #[arg(long)]
        index: Option<String>,
    },

    /// List the installed entries, at the standings
    /// their installs measured
    List,

    /// Remove an installed entry (its bundle file and
    /// its receipt row)
    Remove {
        /// Entry name
        name: String,
    },

    /// Re-verify an installed entry against its receipt
    /// (with no name: sweep the whole install directory)
    Verify {
        /// Entry name (omit to sweep)
        name: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let banner = r#"
    ╔══════════════════════════════════════════════╗
    ║  ███╗   ███╗ ██████╗ ██████╗ ██████╗ ██╗  ██╗ ██╗ ██████╗
    ║  ████╗ ████║██╔═══██╗██╔══██╗██╔══██╗██║  ██║███║██╔════╝
    ║  ██╔████╔██║██║   ██║██████╔╝██████╔╝███████║╚██║██║
    ║  ██║╚██╔╝██║██║   ██║██╔══██╗██╔═══╝ ██╔══██║ ██║██║
    ║  ██║ ╚═╝ ██║╚██████╔╝██║  ██║██║     ██║  ██║ ██║╚██████╗
    ║  ╚═╝     ╚═╝ ╚═════╝ ╚═╝  ╚═╝╚═╝     ╚═╝  ╚═╝ ╚═╝ ╚═════╝
    ║                                                  v0.1.0
    ║     Self-Synthesizing Programming Language
    ║     "Don't write code. Write what you want."
    ╚══════════════════════════════════════════════╝
    "#;

    let cli = Cli::parse();

    // The stdlib-asset subcommand's stdout IS the asset
    // file (Phase 112) — the banner would corrupt it, so
    // that one command runs without it. The mcp
    // subcommand's stdout IS the JSON-RPC stream
    // (Phase 149) — same discipline. The lsp
    // subcommand's stdout IS the LSP stream (Phase 155)
    // — same discipline again.
    if !matches!(cli.command, Command::StdlibAsset | Command::Mcp | Command::Lsp) {
        println!("{}", banner.cyan());
    }

    // Phase 165 (DISEBAR_17_SCOPE §7): the shells that
    // consult the prelude for a user module name the
    // installed layer's degradations, if any — on
    // stderr, so no stdout shape moves. Degradation is
    // named, never silent; a whole layer raises nothing.
    if matches!(
        cli.command,
        Command::Check { .. } | Command::Build { .. } | Command::Synthesize { .. }
    ) {
        for line in morphic::stdlib::installed_diagnostics() {
            eprintln!("morphic: {line}");
        }
    }

    match cli.command {
        Command::Build { package: true, input, out, package_from, bundles_from, .. } => {
            // Phase 122: `morphic build --package` — assemble
            // the npm package (@thvtzy/morphic) from the
            // standard library's shipped bodies. The assembly
            // re-runs the crowning checks before generating
            // (codegen::package); a moved crown fails here and
            // nothing is written.
            if package_from.is_some() {
                anyhow::bail!(
                    "--package-from assembles a user module's package; \
                     it does not combine with --package"
                );
            }
            if bundles_from.is_some() {
                anyhow::bail!(
                    "--bundles-from produces a user module's \
                     re-verification bundles; \
                     it does not combine with --package"
                );
            }
            if input.is_some() {
                anyhow::bail!(
                    "build --package assembles the standard-library package \
                     and takes no input file"
                );
            }
            // The assembly reads the EMBEDDED library data,
            // which exists only in native builds — under the
            // `wasm-playground` feature (which a workspace
            // --all-targets build unifies into this bin) the
            // command refuses by name instead, the StdlibAsset
            // precedent.
            #[cfg(not(feature = "wasm-playground"))]
            {
                let files = codegen::package::assemble_npm_package_checked()?;
                let mass = codegen::package::unpacked_mass(&files);
                if mass > codegen::package::NPM_UNPACKED_BUDGET {
                    anyhow::bail!(
                        "the assembled package is {mass} B unpacked, over the \
                         frozen budget of {} B (EMBED_SCOPE §4) — refusing to write",
                        codegen::package::NPM_UNPACKED_BUDGET
                    );
                }
                let dir =
                    out.unwrap_or_else(|| PathBuf::from(codegen::package::NPM_PACKAGE_DIR));
                std::fs::create_dir_all(&dir)?;
                for f in &files {
                    std::fs::write(dir.join(&f.path), &f.contents)?;
                }
                println!(
                    "{} Assembled {} — {} files, {mass} B unpacked (budget {} B)",
                    "✓".green().bold(),
                    codegen::bindings::PACKAGE_NAME,
                    files.len(),
                    codegen::package::NPM_UNPACKED_BUDGET
                );
                println!("  {} {}", "•".dimmed(), dir.display());
            }
            #[cfg(feature = "wasm-playground")]
            {
                let _ = out;
                anyhow::bail!(
                    "package assembly runs from a native build \
                     (this build carries no embedded library data)"
                );
            }
        }

        Command::Build { facade: true, input, out, package_from, bundles_from, .. } => {
            // Phase 123: `morphic build --facade` — assemble
            // the Rust facade crate (crates.io `morphic`)
            // from the standard library's shipped bodies.
            // The assembly re-runs the crowning checks
            // before generating (codegen::facade, sharing
            // codegen::package's crowning surface); a moved
            // crown fails here and nothing is written. The
            // crate carries no size budget (EMBED_SCOPE R8).
            if package_from.is_some() {
                anyhow::bail!(
                    "--package-from assembles a user module's package; \
                     it does not combine with --facade"
                );
            }
            if bundles_from.is_some() {
                anyhow::bail!(
                    "--bundles-from produces a user module's \
                     re-verification bundles; \
                     it does not combine with --facade"
                );
            }
            if input.is_some() {
                anyhow::bail!(
                    "build --facade assembles the standard-library crate \
                     and takes no input file"
                );
            }
            // The assembly reads the EMBEDDED library data,
            // which exists only in native builds — under the
            // `wasm-playground` feature the command refuses
            // by name instead, the --package precedent.
            #[cfg(not(feature = "wasm-playground"))]
            {
                let files = codegen::facade::assemble_rust_facade_checked()?;
                let total: usize = files.iter().map(|f| f.contents.len()).sum();
                let dir =
                    out.unwrap_or_else(|| PathBuf::from(codegen::facade::RUST_PACKAGE_DIR));
                for f in &files {
                    let path = dir.join(&f.path);
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&path, &f.contents)?;
                }
                println!(
                    "{} Assembled {} (Rust facade crate) — {} files, {total} B \
                     (no size budget, EMBED_SCOPE R8)",
                    "✓".green().bold(),
                    codegen::facade::CRATE_NAME,
                    files.len()
                );
                println!("  {} {}", "•".dimmed(), dir.display());
            }
            #[cfg(feature = "wasm-playground")]
            {
                let _ = out;
                anyhow::bail!(
                    "facade assembly runs from a native build \
                     (this build carries no embedded library data)"
                );
            }
        }

        Command::Build { bundles: true, input, out, package_from, bundles_from, .. } => {
            // Phase 150: `morphic build --bundles` — produce
            // the re-verification bundles through the checked
            // route (src/pcc.rs): the whole crowning surface
            // re-runs first, as assembly does, and a moved
            // crown fails here with nothing written. The
            // bundles are a sibling artifact (packages/
            // bundles) — in neither package's file set, so
            // neither package's budget is charged for them.
            if package_from.is_some() {
                anyhow::bail!(
                    "--package-from assembles a user module's package; \
                     it does not combine with --bundles"
                );
            }
            if bundles_from.is_some() {
                anyhow::bail!(
                    "--bundles-from produces a user module's \
                     re-verification bundles; \
                     it does not combine with --bundles"
                );
            }
            if input.is_some() {
                anyhow::bail!(
                    "build --bundles produces the standard library's \
                     re-verification bundles and takes no input file"
                );
            }
            #[cfg(not(feature = "wasm-playground"))]
            {
                let files = morphic::pcc::produce_all_bundles_checked()
                    .map_err(anyhow::Error::msg)?;
                let total: usize = files.iter().map(|(_, c)| c.len()).sum();
                let dir = out.unwrap_or_else(|| PathBuf::from(morphic::pcc::BUNDLES_DIR));
                std::fs::create_dir_all(&dir)?;
                for (name, contents) in &files {
                    std::fs::write(dir.join(name), contents)?;
                }
                println!(
                    "{} Produced {} re-verification bundles — {total} B total \
                     (sibling artifact; no package budget charged)",
                    "✓".green().bold(),
                    files.len()
                );
                println!("  {} {}", "•".dimmed(), dir.display());
            }
            #[cfg(feature = "wasm-playground")]
            {
                let _ = out;
                anyhow::bail!(
                    "bundle production runs from a native build \
                     (this build carries no embedded library data)"
                );
            }
        }

        Command::VerifyBundle { file } => {
            // Phase 150: `morphic verify-bundle <file>` —
            // the consumer verb. The judgment is the
            // library's (src/pcc.rs); this arm is the shell.
            #[cfg(not(feature = "wasm-playground"))]
            {
                let text = std::fs::read_to_string(&file)?;
                match morphic::pcc::verify_bundle_text(&text) {
                    Ok(summary) => println!("{} {summary}", "✓".green().bold()),
                    Err(reason) => anyhow::bail!("bundle refused: {reason}"),
                }
            }
            #[cfg(feature = "wasm-playground")]
            {
                let _ = &file;
                anyhow::bail!(
                    "bundle re-verification runs from a native build \
                     (this build carries no embedded library data)"
                );
            }
        }

        Command::Verify { library } => {
            // Phase 150: `morphic verify --library` — the
            // stdlib self-check; the library function does
            // the work (src/pcc.rs `verify_library`), this
            // arm reports its standings.
            if !library {
                anyhow::bail!(
                    "verify needs a target: --library re-verifies the shipped \
                     standard library; a bundle is verified with verify-bundle"
                );
            }
            #[cfg(not(feature = "wasm-playground"))]
            {
                let standings =
                    morphic::pcc::verify_library().map_err(anyhow::Error::msg)?;
                let mut failed = 0usize;
                for s in &standings {
                    if s.reproduced {
                        println!(
                            "  {} {} — {} via {}: {}",
                            "✓".green().bold(),
                            s.name,
                            s.tier,
                            s.crown_route,
                            s.detail
                        );
                    } else {
                        failed += 1;
                        println!(
                            "  {} {} — {} via {}: {}",
                            "✗".red().bold(),
                            s.name,
                            s.tier,
                            s.crown_route,
                            s.detail
                        );
                    }
                }
                println!(
                    "{} {} of {} shipped crowns reproduce from the shipped data",
                    if failed == 0 { "✓".green().bold() } else { "✗".red().bold() },
                    standings.len() - failed,
                    standings.len()
                );
                if failed > 0 {
                    anyhow::bail!("{failed} shipped crown(s) do not reproduce");
                }
            }
            #[cfg(feature = "wasm-playground")]
            {
                anyhow::bail!(
                    "the library self-check runs from a native build \
                     (this build carries no embedded library data)"
                );
            }
        }

        Command::Build {
            package_from: Some(module_path),
            package_name,
            package_version,
            seed,
            out,
            input,
            bundles_from,
            ..
        } => {
            // Phase 154: `morphic build --package-from
            // <file.morph>` — the user-package verb
            // (DISEBAR_15_SCOPE §4): the seeded profile
            // measures every spec of the user's module,
            // admission is built only from those measured
            // rows (codegen::package::assemble_user_package),
            // and a refused spec fails the whole assembly
            // here with nothing written — the --package
            // discipline carried to user modules. Naming
            // defaults: the package name is the module
            // file's stem, the version is this Morphic
            // version, the seed is the standing 42, and
            // the output directory is `<stem>-package`.
            if bundles_from.is_some() {
                anyhow::bail!(
                    "--bundles-from produces a user module's \
                     re-verification bundles; \
                     it does not combine with --package-from"
                );
            }
            if input.is_some() {
                anyhow::bail!(
                    "build --package-from names its module with \
                     --package-from itself and takes no positional input file"
                );
            }
            #[cfg(not(feature = "wasm-playground"))]
            {
                let source = std::fs::read_to_string(&module_path)?;
                let stem = module_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "morphic-module".to_string());
                let scope = module_path
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "module.morph".to_string());
                let name = package_name.unwrap_or_else(|| stem.clone());
                let version =
                    package_version.unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
                let seed = seed.unwrap_or(morphic::api::PLAYGROUND_DEFAULT_SEED);
                let assembly = codegen::package::assemble_user_package(
                    &source, &name, &version, &scope, seed,
                )
                .map_err(anyhow::Error::msg)?;
                let mass = codegen::package::unpacked_mass(&assembly.files);
                if mass > codegen::package::NPM_UNPACKED_BUDGET {
                    anyhow::bail!(
                        "the assembled package is {mass} B unpacked, over the \
                         frozen budget of {} B (DISEBAR_15_SCOPE §7) — refusing to write",
                        codegen::package::NPM_UNPACKED_BUDGET
                    );
                }
                for s in &assembly.standings {
                    println!(
                        "  {} {} — {} ({}): {} at {}/{} executed",
                        "✓".green().bold(),
                        s.name,
                        s.tier,
                        s.verdict_basis,
                        s.status,
                        s.passed,
                        s.total
                    );
                }
                let dir = out.unwrap_or_else(|| PathBuf::from(format!("{stem}-package")));
                std::fs::create_dir_all(&dir)?;
                for f in &assembly.files {
                    std::fs::write(dir.join(&f.path), &f.contents)?;
                }
                println!(
                    "{} Assembled {} — {} files, {mass} B unpacked (budget {} B)",
                    "✓".green().bold(),
                    name,
                    assembly.files.len(),
                    codegen::package::NPM_UNPACKED_BUDGET
                );
                println!("  {} {}", "•".dimmed(), dir.display());
            }
            #[cfg(feature = "wasm-playground")]
            {
                let _ = (module_path, package_name, package_version, seed, out);
                anyhow::bail!(
                    "user-package assembly runs from a native build \
                     (assembly verbs are native-build acts in this tree)"
                );
            }
        }

        Command::Build {
            bundles_from: Some(module_path),
            seed,
            out,
            input,
            package_from,
            ..
        } => {
            // Phase 163: `morphic build --bundles-from
            // <file.morph>` — the registry's production
            // verb (DISEBAR_17_SCOPE §4): the
            // --package-from admission loop over the
            // user's module (pcc::assemble_user_bundles),
            // emitting one re-verification bundle per
            // admitted spec through produce_bundle's
            // checked route. A refused spec refuses the
            // whole production here with nothing written —
            // and so does production itself: a spec whose
            // constraints name a sibling spec is admitted
            // in context but cannot be bundled standalone
            // (the self-containment boundary), and a moved
            // crown is refused by the route's own reason.
            // Naming defaults mirror the sibling verb:
            // the seed is the standing 42 and the output
            // directory is `<stem>-bundles`.
            if package_from.is_some() {
                anyhow::bail!(
                    "--package-from assembles a user module's package; \
                     it does not combine with --bundles-from"
                );
            }
            if input.is_some() {
                anyhow::bail!(
                    "build --bundles-from names its module with \
                     --bundles-from itself and takes no positional input file"
                );
            }
            #[cfg(not(feature = "wasm-playground"))]
            {
                let source = std::fs::read_to_string(&module_path)?;
                let stem = module_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "morphic-module".to_string());
                let seed = seed.unwrap_or(morphic::api::PLAYGROUND_DEFAULT_SEED);
                let assembly = morphic::pcc::assemble_user_bundles(&source, seed)
                    .map_err(anyhow::Error::msg)?;
                for s in &assembly.standings {
                    println!(
                        "  {} {} — {} ({}): {} at {}/{} executed",
                        "✓".green().bold(),
                        s.name,
                        s.tier,
                        s.verdict_basis,
                        s.status,
                        s.passed,
                        s.total
                    );
                }
                let total: usize =
                    assembly.files.iter().map(|(_, c)| c.len()).sum();
                let dir = out.unwrap_or_else(|| PathBuf::from(format!("{stem}-bundles")));
                std::fs::create_dir_all(&dir)?;
                for (name, contents) in &assembly.files {
                    std::fs::write(dir.join(name), contents)?;
                }
                println!(
                    "{} Produced {} re-verification bundles — {total} B total \
                     (registry production; no package budget charged)",
                    "✓".green().bold(),
                    assembly.files.len()
                );
                println!("  {} {}", "•".dimmed(), dir.display());
            }
            #[cfg(feature = "wasm-playground")]
            {
                let _ = (module_path, seed, out);
                anyhow::bail!(
                    "bundle production runs from a native build \
                     (this build carries no embedded library data)"
                );
            }
        }

        Command::Build { input, target, opt_level, output, .. } => {
            let input = input.ok_or_else(|| {
                anyhow::anyhow!("build requires a .morph input file (or --package)")
            })?;
            println!("{} Compiling {}...", "⚙".bold(), input.display());
            let source = std::fs::read_to_string(&input)?;

            // Phase 1: Parse
            println!("  {} Parsing specification...", "→".dimmed());
            let spec = spec::parser::parse(&source)?;

            // Phase 2: Type-check
            println!("  {} Type checking...", "→".dimmed());
            let type_checked = spec::typeck::check(spec)?;

            // The whole module stays in scope for synthesis and
            // verification: a spec's constraints may call sibling
            // specs, and those calls resolve against the module
            // (SpecIndex) — the same module-aware pipeline the M2
            // probe and cross-validation harness run. We still
            // build one spec at a time: the file's first spec.
            let functions = &type_checked.spec.functions;
            let spec = functions.first()
                .ok_or_else(|| anyhow::anyhow!("No spec functions found"))?
                .clone();

            // Phase 3: Synthesize
            println!("  {} Synthesizing implementations...", "→".dimmed());
            let candidates = synthesis::engine::synthesize_with_module(&spec, opt_level, functions)?;

            // Phase 4: Verify
            println!("  {} Verifying candidates...", "→".dimmed());
            let verified = verify::verifier::Verifier::with_specs(functions)
                .verify_all(candidates, &spec)?;

            if verified.is_empty() {
                anyhow::bail!("No implementation satisfied all constraints");
            }

            // Phase 4b (Phase 58, G1; extended Phase 60, G3):
            // compiled-runner final scoring. For specs whose
            // in-spec tests the IR interpreter cannot fully
            // bind (the lambda-carrying tuple residue — sort,
            // binary_search), the FALLBACK test_pass_ratio 1.0
            // is replaced for a bounded finalist set by a
            // measured compiled-runner ratio before selection;
            // since Phase 60 the stage likewise judges a
            // constraint-constructed seed the interpreter
            // cannot evaluate at all (the forall-fold scan —
            // count_matching), whose interpreter ratio is 0.0
            // by coverage rather than behavior. A no-op for
            // every other spec; infrastructure failure
            // degrades to the fallback behavior — it never
            // fails the build.
            let verified = synthesis::runner::rescore_finalists(verified, &spec);

            // Phase 5: Optimize & select
            println!("  {} Selecting optimal implementation...", "→".dimmed());
            let winner = synthesis::selector::select_best(verified, &spec)?;

            // Phase 6: Codegen
            println!("  {} Generating {} code...", "→".dimmed(), target);
            let code = codegen::generate(&winner, &target)?;

            // Phase 7: Output
            let out_path = output.unwrap_or_else(|| {
                let ext = match target.as_str() {
                    "solidity" | "sol" => "sol",
                    other => other,
                };
                input.with_extension(ext)
            });
            std::fs::write(&out_path, &code)?;

            println!("\n{} {}", "✓".green().bold(), format!("Generated: {}", out_path.display()).green());
            println!("  {} {} candidates evaluated", "•".dimmed(), synthesis::engine::candidate_count());
            println!("  {} Synthesis time: {}ms", "•".dimmed(), synthesis::engine::elapsed_ms());
        }

        Command::Check { input } => {
            println!("{} Checking {}...", "✓".bold(), input.display());
            let source = std::fs::read_to_string(&input)?;
            let spec = spec::parser::parse(&source)?;
            let _spec = spec::typeck::check(spec)?;
            println!("{} Specification is valid.", "✓".green().bold());
        }

        Command::Synthesize { input, iterations } => {
            println!("{} Synthesizing interactively ({} max iterations)...",
                "🔍".bold(), iterations);
            // Interactive mode: stream candidates in real time.
            // The TUI is native-only furniture (the
            // `interactive` feature); a build without it
            // reports the fact rather than failing to compile.
            #[cfg(feature = "interactive")]
            {
                let source = std::fs::read_to_string(&input)?;
                let parsed = spec::parser::parse(&source)?;
                let type_checked = spec::typeck::check(parsed)?;
                let func_spec = type_checked.spec.functions.first()
                    .ok_or_else(|| anyhow::anyhow!("No spec functions found"))?;
                synthesis::interactive::run(func_spec, iterations)?;
            }
            #[cfg(not(feature = "interactive"))]
            {
                let _ = (input, iterations);
                anyhow::bail!(
                    "interactive synthesis is not available in this build \
                     (feature `interactive` is off)"
                );
            }
        }

        Command::Lsp => {
            // Phase 155 (DISEBAR_15_SCOPE §5): the real
            // LSP diagnostics-only server — Content-Length
            // framed JSON-RPC 2.0 over stdio (src/lsp.rs):
            // initialize / shutdown / exit, full-document
            // sync, push diagnostics from api::check.
            // Stdout IS the protocol stream (the banner is
            // suppressed above, and nothing else prints);
            // the process takes the lifecycle's own exit
            // code — 0 after shutdown, 1 without.
            std::process::exit(morphic::lsp::run_stdio()?);
        }

        Command::Mcp => {
            // Phase 149 (DISEBAR_SCOPE §3): the MCP server
            // — hand-rolled stdio JSON-RPC over the DTO
            // API (src/mcp.rs). It ships only in the
            // Z3-enabled native build (§3 R3): the
            // verdicts it serves are the verifier's own,
            // and a build without the verifier could only
            // dress executed evidence as verdicts — so
            // other builds refuse by name instead.
            #[cfg(feature = "z3-support")]
            {
                morphic::mcp::run_stdio()?;
            }
            #[cfg(not(feature = "z3-support"))]
            {
                anyhow::bail!(
                    "the MCP server ships only in the Z3-enabled native build \
                     (DISEBAR_SCOPE §3 R3) — this build has no verifier to serve \
                     verdicts from"
                );
            }
        }

        Command::Registry { action } => {
            // Phase 164: `morphic registry …` — the
            // registry client (DISEBAR_17_SCOPE §5). The
            // judgments are the library's (src/registry.rs
            // over src/pcc.rs); this arm is the shell. The
            // registry has no browser surface (§7), so the
            // wasm-playground profile refuses by name — the
            // --package precedent.
            #[cfg(not(feature = "wasm-playground"))]
            {
                match action {
                    RegistryAction::Add { name, index } => {
                        let line = morphic::registry::registry_add(
                            &name,
                            index.as_deref(),
                        )
                        .map_err(anyhow::Error::msg)?;
                        println!("{} {line}", "✓".green().bold());
                    }
                    RegistryAction::List => {
                        let receipts = morphic::registry::registry_list()
                            .map_err(anyhow::Error::msg)?;
                        if receipts.is_empty() {
                            println!("  (no registry entries installed)");
                        }
                        for r in &receipts {
                            println!(
                                "  {} {} — {} ({}), sha256 {}…",
                                r.entry,
                                r.version,
                                r.tier,
                                r.verdict_basis,
                                &r.sha256[..12]
                            );
                        }
                    }
                    RegistryAction::Remove { name } => {
                        let line = morphic::registry::registry_remove(&name)
                            .map_err(anyhow::Error::msg)?;
                        println!("{} {line}", "✓".green().bold());
                    }
                    RegistryAction::Verify { name } => {
                        let lines = morphic::registry::registry_verify(
                            name.as_deref(),
                        )
                        .map_err(anyhow::Error::msg)?;
                        if lines.is_empty() {
                            println!("  (no registry entries installed)");
                        }
                        for line in &lines {
                            println!("{} {line}", "✓".green().bold());
                        }
                    }
                }
            }
            #[cfg(feature = "wasm-playground")]
            {
                let _ = action;
                anyhow::bail!(
                    "the registry client runs from a native build \
                     (this build carries no registry surface)"
                );
            }
        }

        Command::StdlibAsset => {
            // Phase 112: the asset generator. Stdout is
            // exactly the asset bytes (no banner, nothing
            // else): `morphic stdlib-asset >
            // playground/stdlib-asset.bin` regenerates the
            // checked-in file byte-for-byte. The generator
            // reads the EMBEDDED library data, which exists
            // only in native builds — under the
            // `wasm-playground` feature (which a workspace
            // --all-targets build unifies into this bin)
            // the subcommand refuses by name instead.
            #[cfg(not(feature = "wasm-playground"))]
            {
                use std::io::Write;
                std::io::stdout().write_all(&morphic::stdlib::library_asset_bytes())?;
            }
            #[cfg(feature = "wasm-playground")]
            {
                anyhow::bail!(
                    "stdlib-asset is generated from a native build \
                     (this build carries no embedded library data)"
                );
            }
        }

        Command::Init { name } => {
            let dir = PathBuf::from(&name);
            std::fs::create_dir_all(&dir)?;

            // Phase 110: the template corrected to reality.
            // There is no `morphic-std` crate: the standard
            // library ships with the compiler and is merged
            // into every module as a prelude (decision D2a),
            // so there is no dependency to declare and no
            // import to write — and the generated spec is a
            // real module that parses and checks, calling a
            // library entry by its plain name.
            let cargo_toml = format!(r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

# The standard library ships with the compiler (a prelude
# merged into every module) — there is no library
# dependency to declare.

[[bin]]
name = "{}"
path = "main.morph"
"#, name, name);

            let spec = format!(r#"// {} — Morphic Project
// Generated by morphic init
//
// The standard library is built in: its entries (such as
// int_abs) are available in every module by plain name —
// no import, no dependency.

{} {{
spec main {{
    input: x: Int
    output: Int

    constraint: output == int_abs(x)

    test {{
        input: (0 - 3)
        expect: 3
    }}
}}
}}
"#, name, name);

            std::fs::write(dir.join("Morphic.toml"), cargo_toml)?;
            std::fs::write(dir.join("main.morph"), spec)?;

            println!("{} Created new Morphic project: {}", "✓".green().bold(), name);
            println!("  cd {}", name);
            println!("  morphic build main.morph");
        }
    }

    Ok(())
}
