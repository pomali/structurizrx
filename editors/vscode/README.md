# StructurizrX DSL

Language support for [Structurizr DSL](https://docs.structurizr.com/dsl)
files (`.dsl`, or the unambiguous `.c4sx` extension), powered by
[structurizrx](https://github.com/pomali/structurizrx), a Rust
re-implementation of Structurizr. The language server ships bundled as
WebAssembly, so the extension works immediately after install — **no binary,
runtime or extra setup required**.

## Features

- **Diagnostics** — syntax and model-validation errors as you type.
- **Hover** — kind, description, technology and tags for any element identifier.
- **Completion** — scoped to the enclosing block, so a `views` body offers view
  keywords and a `container` body offers container keywords. After `->` only
  element identifiers are offered.
- **Go to definition**, **find all references** and **highlight occurrences**
  for element identifiers.
- **Rename symbol** — rewrites every reference to an element. Only offered on
  declared identifiers, so keywords and text inside quoted strings are left
  alone.
- **Semantic highlighting** — the lexer distinguishes keywords from identifiers
  and `!directive`s, which the TextMate grammar can only guess at. Everything
  else stays TextMate-coloured.
- **Outline** — people, software systems, containers and components.

See [PLAN.md](PLAN.md) for what's implemented and what's planned.

## Requirements

None — the bundled WebAssembly language server runs in-process. Installing the
`structurizrx` binary is optional; see below if you want it.

## Language server runtime

The same language server ships two ways, selected by `structurizrDsl.runtime`:

| `runtime` | Behaviour |
|---|---|
| `auto` (default) | Use the `structurizrx` binary if one is found, else the bundled WebAssembly build. |
| `binary` | Only use the `structurizrx` binary; error if it isn't found. |
| `wasm` | Always use the bundled WebAssembly build. |

The WASM build (`rust/structurizr-lsp-wasm`) runs in-process in the extension
host, so **nothing has to be installed** — no binary, no subprocess. The native
binary is faster on large workspaces and always matches your installed CLI, so
`auto` prefers it when present.

To use a binary, build it once:

```sh
cd ../../rust
cargo build -p structurizr-cli
```

The extension looks for `structurizrx` on your `PATH`; either add
`rust/target/debug` to `PATH`, or point at it directly with
`structurizrDsl.serverPath`.

## Building the extension

```sh
cd editors/vscode
npm install
npm run build:wasm   # needs `cargo install wasm-pack`; writes wasm/
npm run compile      # esbuild -> out/extension.js
```

`build:wasm` is only needed for the WASM runtime, and is run automatically by
`vscode:prepublish` when packaging. Without it, set `structurizrDsl.runtime` to
`binary`.

`npm run compile` bundles `src/` into a single `out/extension.js` with esbuild;
`npm run watch` rebuilds on change. esbuild does not type-check, so run
`npm run check-types` (or rely on your editor) for that — packaging runs it.
The wasm-bindgen glue in `wasm/` is deliberately left out of the bundle,
because it loads `structurizr_lsp_wasm_bg.wasm` relative to its own directory.

Then open this `editors/vscode` folder in VS Code and press F5 to launch an
Extension Development Host. Open any `.dsl` file (e.g. one of the fixtures
under `../../original-java/structurizr-dsl/src/test/resources/dsl/`) to see
highlighting, diagnostics, hover, go-to-definition and the outline view.

## Packaging a `.vsix`

```sh
cd editors/vscode
npm install
npx @vscode/vsce package          # -> structurizr-dsl-<version>.vsix
code --install-extension structurizr-dsl-0.3.0.vsix
```

Packaging runs `vscode:prepublish`, which builds the WASM language server,
type-checks, and bundles the extension, so the installed `.vsix` works with
nothing else installed. The `structurizrx` binary is *not* bundled; install it
separately to use the native runtime.

## Releasing

`.github/workflows/vscode-extension.yml` builds the `.vsix` on every push and
pull request, and publishes when a `vscode-v*` tag is pushed.

One-time setup:

1. Claim the `structurizrx` publisher at
   <https://marketplace.visualstudio.com/manage> (needs a Microsoft account and
   an Azure DevOps organization).
2. Create an Azure DevOps personal access token with organization
   "All accessible organizations" and the scope **Marketplace → Manage**. Store
   it as the `VSCE_PAT` repository secret.
3. Optional, for VSCodium/Cursor users: create an
   [Open VSX](https://open-vsx.org/user-settings/tokens) token and store it as
   `OVSX_PAT`. The Open VSX step is skipped when the secret is absent.

To cut a release, bump `version` in `package.json`, move the `Unreleased`
entries in `CHANGELOG.md` under the new version, then:

```sh
git tag vscode-v0.3.1
git push origin vscode-v0.3.1
```

The workflow refuses to publish if the tag and `package.json` disagree. It
publishes the exact `.vsix` it built and attaches it to a GitHub release.

## Known limitations

- Diagnostics from `structurizr_model::validation::validate` fall back to the
  top of the document when the underlying error can't be matched back to a
  declared element's position (validation errors don't carry a span today).
- Rename and find-references match whole identifier tokens only. A hierarchical
  reference (`web.api`) lexes as one word, so renaming `web` won't rewrite it —
  that needs the parser's scoping rules rather than a token scan.
- Rename is single-file; it doesn't follow `!include`.
- Hover/outline cover people, software systems, containers and components;
  deployment nodes and custom elements aren't covered yet.
- The WASM runtime is a desktop (Node extension host) build; it isn't wired up
  for vscode.dev / github.dev web hosts yet.
