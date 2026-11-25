# Morphic for VS Code

The thin client for the Morphic language server: open a
`.morph` file and the diagnostics the Morphic compiler
computes appear in the editor — parse errors at their
true positions, typecheck errors as one file-level
diagnostic whose message begins `unpositioned: ` (the
typechecker's errors carry no position; the server says
so rather than inventing one).

Everything diagnostic comes from the server — the
`morphic lsp` subcommand of the Morphic compiler,
speaking LSP over stdio with full-document sync. This
extension contributes no intelligence of its own:

- the `morphic` language for `.morph` files, with a
  language configuration (comment tokens and brackets
  from the language's own grammar);
- a minimal TextMate grammar for **highlighting only**
  — decoration, with no honesty surface; it never
  produces a diagnostic;
- one setting, below;
- a TypeScript entry (~70 lines) that starts VS Code's
  generic LanguageClient against the server and stops
  it on deactivate.

Diagnostics-only means diagnostics-only: no hover (the
manifest/bundle evidence story is not surfaced
in-editor), no completion, no definition, no
formatting, no code actions, no commands, no views.

## Setting

- `morphic.serverPath` — path to the `morphic` binary
  whose `lsp` subcommand serves the session. Default:
  `morphic` (looked up on the PATH). Point it at a
  local build, e.g. `/path/to/morphic/target/debug/morphic`.

## Install (sideload)

The extension is distributed as a `.vsix`, installed
with no account:

```sh
cd packages/vscode
npm ci
npm run package          # writes morphic-<version>.vsix
code --install-extension morphic-<version>.vsix
```

Publishing to the Visual Studio Marketplace is the
project owner's later act; nothing in this repository
publishes the extension anywhere.

## Version

The extension's version tracks the Morphic lockstep:
it reads as the compiler's version, and a release
stamp moves both together.

## License

Dual-licensed under MIT OR Apache-2.0, like the rest
of Morphic (see LICENSE-MIT and LICENSE-APACHE).
