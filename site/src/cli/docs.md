# structurizrx docs

Print the DSL extension cheat sheet (`llms.txt`) to stdout.

```sh
structurizrx docs
```

This is the same one-page reference also served as plain text by
`structurizrx serve` at `/llms.txt`, and rendered as this documentation site's
[Language reference](../language/README.md). It's designed to fit in an LLM
agent's context in one shot: the entire extension surface (ports, `kind`,
`status`, sketch mode, milestones, generators, selectors) on one page — if an
extension doesn't fit, the extension is considered too big.
