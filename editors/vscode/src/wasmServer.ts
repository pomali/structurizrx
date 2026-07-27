/**
 * Runs the language server in-process from its WebAssembly build, so the
 * extension works without a `structurizrx` binary installed.
 *
 * The WASM module has no transport of its own — it's a pure
 * message-in/messages-out function. This file wraps it in the
 * MessageReader/MessageWriter pair that `LanguageClient` expects, with no
 * subprocess, pipes or framing in between.
 */
import {
  AbstractMessageReader,
  AbstractMessageWriter,
  DataCallback,
  Disposable,
  Message,
  MessageReader,
  MessageWriter,
} from 'vscode-jsonrpc/node';

/** The subset of the wasm-bindgen output we rely on. */
interface WasmModule {
  LspServer: new () => { handle(message: string): string; free(): void };
}

/** Queue of messages travelling wasm → client. */
class WasmMessageReader extends AbstractMessageReader implements MessageReader {
  private callback: DataCallback | undefined;
  /** Messages produced before `listen` was called, replayed on listen. */
  private pending: Message[] = [];

  listen(callback: DataCallback): Disposable {
    this.callback = callback;
    const buffered = this.pending;
    this.pending = [];
    for (const message of buffered) {
      callback(message);
    }
    return Disposable.create(() => {
      this.callback = undefined;
    });
  }

  emit(message: Message): void {
    if (this.callback) {
      this.callback(message);
    } else {
      this.pending.push(message);
    }
  }
}

class WasmMessageWriter extends AbstractMessageWriter implements MessageWriter {
  constructor(
    private readonly server: { handle(message: string): string },
    private readonly reader: WasmMessageReader
  ) {
    super();
  }

  async write(msg: Message): Promise<void> {
    let outgoing: Message[];
    try {
      outgoing = JSON.parse(this.server.handle(JSON.stringify(msg))) as Message[];
    } catch (error) {
      this.fireError(error, msg);
      return;
    }
    // Deliver asynchronously: the client is still inside its own `write` call
    // here, and re-entering it with a response confuses request tracking.
    for (const message of outgoing) {
      queueMicrotask(() => this.reader.emit(message));
    }
  }

  end(): void {
    // Nothing to flush — `write` completes synchronously.
  }
}

export interface WasmTransports {
  reader: MessageReader;
  writer: MessageWriter;
  detached: boolean;
}

/**
 * Loads the WASM language server and returns transports for `LanguageClient`.
 * Throws if the module is missing from the packaged extension (i.e. it was
 * built without running `npm run build:wasm`).
 */
export function createWasmServer(): WasmTransports {
  // Required lazily so that a missing/failed WASM build only breaks the WASM
  // runtime, not activation with a native binary.
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const wasm = require('../wasm/structurizr_lsp_wasm.js') as WasmModule;
  const server = new wasm.LspServer();
  const reader = new WasmMessageReader();
  return {
    reader,
    writer: new WasmMessageWriter(server, reader),
    // The "server" is this process; nothing to wait on at shutdown.
    detached: true,
  };
}
