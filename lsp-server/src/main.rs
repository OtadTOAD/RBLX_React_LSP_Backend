mod api_manager;
mod api_parser;

use tower_lsp::{
    jsonrpc::Result,
    lsp_types::{
        CompletionOptions, ExecuteCommandOptions, InitializeParams, InitializeResult,
        InitializedParams, MessageType, ServerCapabilities, TextDocumentSyncCapability,
        TextDocumentSyncKind,
    },
    Client, LanguageServer, LspService, Server,
};

#[derive(Debug)]
struct Backend {
    client: Client,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["rblx-react-lsp.genMetadata".to_string()],
                    work_done_progress_options: Default::default(),
                }),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(true),
                    trigger_characters: None,
                    ..Default::default()
                }),
                ..Default::default()
            },
            server_info: None,
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend { client });
    Server::new(stdin, stdout, socket).serve(service).await;
}
