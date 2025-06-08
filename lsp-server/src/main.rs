mod api_manager;
mod api_parser;
mod file_manager;

use std::sync::Arc;

use tokio::sync::Mutex;
use tower_lsp::{
    jsonrpc::Result,
    lsp_types::{
        CompletionOptions, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
        DidOpenTextDocumentParams, ExecuteCommandOptions, InitializeParams, InitializeResult,
        InitializedParams, MessageType, ServerCapabilities, TextDocumentSyncCapability,
        TextDocumentSyncKind,
    },
    Client, LanguageServer, LspService, Server,
};

use crate::file_manager::FileManager;

#[derive(Debug)]
struct Backend {
    client: Client,
    file_manager: Arc<Mutex<FileManager>>,
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

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let mut mutex_fm = self.file_manager.lock().await;
        mutex_fm.on_opened_file(
            params.text_document.uri,
            params.text_document.text,
            params.text_document.version,
        );
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let mut mutex_fm = self.file_manager.lock().await;
        mutex_fm.on_changed_file(
            &params.text_document.uri,
            &params.content_changes,
            params.text_document.version,
        );
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut mutex_fm = self.file_manager.lock().await;
        mutex_fm.on_closed_file(&params.text_document.uri);
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        file_manager: Arc::new(Mutex::new(FileManager::new())),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
