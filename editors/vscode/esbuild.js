/**
 * Bundles the extension into a single `out/extension.js`.
 *
 * Two things are deliberately left unbundled:
 *  - `vscode`, which the extension host injects at runtime;
 *  - `../wasm/structurizr_lsp_wasm.js`, the wasm-bindgen nodejs glue. It reads
 *    `structurizr_lsp_wasm_bg.wasm` relative to its own `__dirname`, so it has
 *    to keep living in `wasm/` and be required from there. The specifier is
 *    relative to `src/`, which is the same depth below the extension root as
 *    `out/`, so it resolves identically from the bundle.
 */
const esbuild = require('esbuild');

const production = process.argv.includes('--production');
const watch = process.argv.includes('--watch');

async function main() {
  const ctx = await esbuild.context({
    entryPoints: ['src/extension.ts'],
    bundle: true,
    format: 'cjs',
    platform: 'node',
    target: 'node20',
    outfile: 'out/extension.js',
    external: ['vscode', '../wasm/structurizr_lsp_wasm.js'],
    minify: production,
    sourcemap: !production,
    sourcesContent: false,
    logLevel: 'info',
  });

  if (watch) {
    await ctx.watch();
  } else {
    await ctx.rebuild();
    await ctx.dispose();
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
