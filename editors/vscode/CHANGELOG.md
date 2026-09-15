# Changelog

All notable changes to the StructurizrX DSL extension are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.4.0]

### Added

- The outline lists every element, including ones declared without an
  identifier, plus ports, relationships (`User → API`) and views. Each symbol
  spans its whole statement, and nesting follows the text: a relationship
  declared inside an element's body sits under that element.

### Fixed

- With the `structurizrx` binary (0.2.0 or later) as the runtime, `!include`d
  files are resolved against the open file's directory, so identifiers declared
  in them no longer show as unknown; go-to-definition follows them into the
  included file, and an error inside an included file is marked on its
  `!include` line. The bundled WebAssembly server has no file system access and
  still parses the open file on its own.

## [0.3.0]

### Changed

- The extension is now bundled with esbuild into a single `out/extension.js`,
  instead of shipping `node_modules`. The `.vsix` drops from 368 files to a
  handful, which is what the extension host wants for activation time.
- Builds and releases run on GitHub Actions; pushing a `vscode-v*` tag
  publishes to the VS Code Marketplace and Open VSX.

### Added

- Semantic highlighting, driven by the DSL lexer, so `!include` directives,
  expressions and multi-line descriptions colour correctly where the TextMate
  grammar could only guess.
- Context-aware completion: a `views` body offers view keywords, a `container`
  body offers container keywords, and after `->` only element identifiers are
  offered.
- Rename symbol across every reference to a declared element.

## [0.2.0]

### Added

- Bundled WebAssembly language server, so the extension works with no
  `structurizrx` binary installed. The `structurizrDsl.runtime` setting picks
  between `auto`, `binary` and `wasm`.

## [0.1.0]

### Added

- Initial release: TextMate grammar, diagnostics, hover, completion,
  go-to-definition, find references and document outline for `.dsl`/`.c4sx`
  files, backed by the `structurizrx` binary.
