import * as fs from 'fs';
import * as path from 'path';
import { ExtensionContext, window, workspace } from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export async function activate(context: ExtensionContext): Promise<void> {
  const command = resolveServerPath();
  if (!command) {
    window.showErrorMessage(
      "Structurizr DSL: couldn't find the `structurizrx` binary on PATH. " +
        'Build it with `cargo build -p structurizr-cli` in the repo\'s rust/ directory, ' +
        'or set the structurizrDsl.serverPath setting.'
    );
    return;
  }

  const serverOptions: ServerOptions = {
    command,
    args: ['lsp'],
    transport: TransportKind.stdio,
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: 'file', language: 'structurizr-dsl' }],
  };

  client = new LanguageClient(
    'structurizrDsl',
    'Structurizr DSL Language Server',
    serverOptions,
    clientOptions
  );
  context.subscriptions.push(client);
  await client.start();
}

export async function deactivate(): Promise<void> {
  await client?.stop();
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
