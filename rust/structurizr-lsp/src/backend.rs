//! tower-lsp `LanguageServer` implementation: a thin async shell around the
//! synchronous [`Core`], which holds all the actual logic.

use tower_lsp_server::jsonrpc::Result as RpcResult;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer};

use crate::core::Core;

pub struct Backend {
    client: Client,
    core: Core,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            core: Core::new(),
        }
    }

    async fn republish(&self, uri: Uri, diagnostics: Vec<Diagnostic>) {
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> RpcResult<InitializeResult> {
        Ok(Core::initialize_result())
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "structurizr-lsp ready")
            .await;
    }

    async fn shutdown(&self) -> RpcResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let diags = self
            .core
            .set_document(uri.clone(), params.text_document.text);
        self.republish(uri, diags).await;
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        let Some(change) = params.content_changes.pop() else {
            return;
        };
        let uri = params.text_document.uri;
        let diags = self.core.set_document(uri.clone(), change.text);
        self.republish(uri, diags).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.core.close_document(&params.text_document.uri);
    }

    async fn hover(&self, params: HoverParams) -> RpcResult<Option<Hover>> {
        let p = params.text_document_position_params;
        Ok(self.core.hover(&p.text_document.uri, p.position))
    }

    async fn completion(&self, params: CompletionParams) -> RpcResult<Option<CompletionResponse>> {
        let p = params.text_document_position;
        Ok(Some(CompletionResponse::Array(
            self.core.completion(&p.text_document.uri, p.position),
        )))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> RpcResult<Option<GotoDefinitionResponse>> {
        let p = params.text_document_position_params;
        Ok(self
            .core
            .goto_definition(&p.text_document.uri, p.position)
            .map(GotoDefinitionResponse::Scalar))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> RpcResult<Option<DocumentSymbolResponse>> {
        Ok(self
            .core
            .document_symbol(&params.text_document.uri)
            .map(DocumentSymbolResponse::Nested))
    }

    async fn references(&self, params: ReferenceParams) -> RpcResult<Option<Vec<Location>>> {
        let p = params.text_document_position;
        Ok(self.core.references(&p.text_document.uri, p.position))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> RpcResult<Option<Vec<DocumentHighlight>>> {
        let p = params.text_document_position_params;
        Ok(self
            .core
            .document_highlight(&p.text_document.uri, p.position))
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> RpcResult<Option<PrepareRenameResponse>> {
        Ok(self
            .core
            .prepare_rename(&params.text_document.uri, params.position)
            .map(PrepareRenameResponse::Range))
    }

    async fn rename(&self, params: RenameParams) -> RpcResult<Option<WorkspaceEdit>> {
        let p = params.text_document_position;
        Ok(self
            .core
            .rename(&p.text_document.uri, p.position, &params.new_name))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> RpcResult<Option<SemanticTokensResult>> {
        Ok(self
            .core
            .semantic_tokens(&params.text_document.uri)
            .map(SemanticTokensResult::Tokens))
    }
}
