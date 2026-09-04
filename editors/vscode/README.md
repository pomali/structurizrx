# StructurizrX DSL

Language support for modeling C4 using [Structurizr DSL](https://docs.structurizr.com/dsl)
files (`.dsl`, or the unambiguous `.c4sx` extension), powered by
[structurizrx](https://github.com/pomali/structurizrx), a Rust
re-implementation of Structurizr. The language server ships bundled as
WebAssembly, so the extension works immediately after install — **no binary,
runtime or extra setup required**.

For more documentation about StructurizrX see [StructurizrX Docs](https://pomali.github.io/structurizrx/).

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
