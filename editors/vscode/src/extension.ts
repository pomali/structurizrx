import * as fs from 'fs';
import * as path from 'path';
import { ExtensionContext, window, workspace } from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from 'vscode-languageclient/node';

import { createWasmServer } from './wasmServer';

let client: LanguageClient | undefined;

type Runtime = 'auto' | 'binary' | 'wasm';

export async function activate(context: ExtensionContext): Promise<void> {
  const resolved = resolveServer();
  if (!resolved) {
    return;
  }

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: 'file', language: 'structurizr-dsl' }],
  };

  client = new LanguageClient(
    'structurizrDsl',
    'Structurizr DSL Language Server',
    resolved.serverOptions,
    clientOptions
  );
  context.subscriptions.push(client);

  try {
    await client.start();
  } catch (error) {
    client = undefined;
    if (resolved.kind === 'wasm') {
      window.showErrorMessage(
        `Structurizr DSL: the bundled WebAssembly language server failed to start (${error}). ` +
          'Install the `structurizrx` binary, or rebuild the extension with `npm run build:wasm`.'
      );
    } else {
      window.showErrorMessage(`Structurizr DSL: the language server failed to start (${error}).`);
    }
  }
}

export async function deactivate(): Promise<void> {
  await client?.stop();
}

/**
 * Picks how to run the server. `auto` prefers the native binary — it's faster
 * and always matches the installed CLI — and falls back to the bundled WASM
 * build, so the extension works with nothing installed.
 */
function resolveServer(): { kind: 'binary' | 'wasm'; serverOptions: ServerOptions } | undefined {
  const runtime = workspace.getConfiguration('structurizrDsl').get<Runtime>('runtime') ?? 'auto';
  const binary = runtime === 'wasm' ? undefined : resolveServerPath();

  if (binary) {
    return {
      kind: 'binary',
      serverOptions: { command: binary, args: ['lsp'], transport: TransportKind.stdio },
    };
  }

  if (runtime === 'binary') {
    window.showErrorMessage(
      "Structurizr DSL: couldn't find the `structurizrx` binary on PATH. " +
        'Set structurizrDsl.serverPath, or set structurizrDsl.runtime to "auto" ' +
        'or "wasm" to use the bundled WebAssembly language server instead.'
    );
    return undefined;
  }

  return { kind: 'wasm', serverOptions: async () => createWasmServer() };
}

// Setting → PATH lookup, in that order. Returns undefined if neither finds
// the binary, rather than guessing.
function resolveServerPath(): string | undefined {
  const configured = workspace.getConfiguration('structurizrDsl').get<string>('serverPath');
  if (configured && configured.trim().length > 0) {
    return configured;
  }

  const exeName = process.platform === 'win32' ? 'structurizrx.exe' : 'structurizrx';
  const pathDirs = (process.env.PATH ?? '').split(path.delimiter);
  for (const dir of pathDirs) {
    const candidate = path.join(dir, exeName);
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }
  return undefined;
}
