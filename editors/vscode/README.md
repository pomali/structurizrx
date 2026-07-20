# Structurizr DSL (VS Code extension)

Syntax highlighting plus language-server features (diagnostics, hover,
completion, go-to-definition, outline) for `.dsl` files, backed by the
`structurizrx lsp` subcommand from this repo's Rust workspace.

## Prerequisites

Build the CLI once:

```sh
cd ../../rust
cargo build -p structurizr-cli
```

By default the extension looks for a `structurizrx` binary on your `PATH`. If
it isn't there, either add `rust/target/debug` to `PATH`, or point the
extension at it directly via the `structurizrDsl.serverPath` setting.

## Running the extension

```sh
cd editors/vscode
npm install
npm run compile
```

Then open this `editors/vscode` folder in VS Code and press F5 to launch an
Extension Development Host. Open any `.dsl` file (e.g. one of the fixtures
under `../../original-java/structurizr-dsl/src/test/resources/dsl/`) to see
highlighting, diagnostics, hover, go-to-definition and the outline view.

## Known v1 limitations

- Diagnostics from `structurizr_model::validation::validate` fall back to the
  top of the document when the underlying error can't be matched back to a
  declared element's position (validation errors don't carry a span today).
- No semantic tokens yet — highlighting is TextMate-grammar based.
- Hover/outline cover people, software systems, containers and components;
  deployment nodes and custom elements aren't covered yet.
