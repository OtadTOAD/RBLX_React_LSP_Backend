mod api_manager;
mod api_parser;
mod file_diagnoser;
mod file_manager;

use std::{path::PathBuf, sync::Arc};

use serde_json::Value;
use tokio::sync::Mutex;
use tower_lsp::{
    jsonrpc::Result,
    lsp_types::{
        CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
        DidCloseTextDocumentParams, DidOpenTextDocumentParams, ExecuteCommandOptions,
        ExecuteCommandParams, InitializeParams, InitializeResult, InitializedParams, MessageType,
        ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
    },
    Client, LanguageServer, LspService, Server,
};

use crate::{
    api_manager::ApiManager, api_parser::create_api_file_readable,
    file_diagnoser::generate_auto_completions, file_manager::FileManager,
};

#[derive(Debug)]
struct Backend {
    client: Client,
    file_manager: Arc<Mutex<FileManager>>,
    api_manager: Arc<Mutex<ApiManager>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                semantic_tokens_provider: None,
                hover_provider: None,
                signature_help_provider: None,
                selection_range_provider: None,
                definition_provider: None,
                type_definition_provider: None,
                implementation_provider: None,
                references_provider: None,
                document_highlight_provider: None,
                document_symbol_provider: None,
                workspace_symbol_provider: None,
                code_action_provider: None,
                code_lens_provider: None,
                document_formatting_provider: None,
                document_range_formatting_provider: None,
                document_on_type_formatting_provider: None,
                rename_provider: None,
                document_link_provider: None,
                color_provider: None,
                folding_range_provider: None,
                declaration_provider: None,
                workspace: None,
                call_hierarchy_provider: None,
                moniker_provider: None,
                linked_editing_range_provider: None,
                experimental: None,

                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["rblx-react-lsp.genMetadata".to_string()],
                    work_done_progress_options: Default::default(),
                }),
                completion_provider: Some(CompletionOptions {
                    //resolve_provider: Some(true),
                    trigger_characters: Some(vec![
                        "\"".to_string(),
                        ".".to_string(),
                        "{".to_string(),
                        "`".to_string(),
                        "'".to_string(),
                        "[".to_string(),
                    ]),
                    ..Default::default()
                }),
            },
            server_info: None,
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        let api_manager = self.api_manager.clone(); // Clones Arc, not the actual manager
        let client = self.client.clone(); // Clones internal client handle

        tokio::spawn(async move {
            let mut api_manager = api_manager.lock().await;
            if let Err(e) = api_manager.load_api().await {
                let _ = client
                    .log_message(MessageType::ERROR, format!("Failed to load API: {}", e))
                    .await;
            } else {
                let _ = client
                    .log_message(MessageType::INFO, "API loaded in background.")
                    .await;
            }
        });

        self.client
            .log_message(MessageType::INFO, "Server initialized!")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let mut file_manager = self.file_manager.lock().await;
        let mut api_manager = self.api_manager.lock().await;
        api_manager.update_freq(&params.text_document.text);
        file_manager.on_opened_file(
            params.text_document.uri,
            params.text_document.text,
            params.text_document.version,
        );
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let mut file_manager = self.file_manager.lock().await;
        let mut api_manager = self.api_manager.lock().await;
        file_manager.on_changed_file(
            &params.text_document.uri,
            &params.content_changes,
            params.text_document.version,
        );
        if let Some(doc) = file_manager.get_text(&params.text_document.uri) {
            api_manager.update_freq(doc);
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut file_manager = self.file_manager.lock().await;
        file_manager.on_closed_file(&params.text_document.uri);
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let file_manager = self.file_manager.lock().await;
        let api_manager = self.api_manager.lock().await;
        let text_document = params.text_document_position;

        let file_text = file_manager.get_text(&text_document.text_document.uri);
        if file_text.is_some() {
            if let Ok(diagnose_results) =
                generate_auto_completions(file_text.unwrap(), &text_document.position, &api_manager)
            {
                return Ok(Some(diagnose_results));
            }
        } else {
            self.client
                .log_message(MessageType::LOG, "Could not find file!")
                .await;
        }

        Ok(Some(CompletionResponse::Array(vec![])))
    }

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> Result<Option<serde_json::Value>> {
        if params.command == "rblx-react-lsp.genMetadata" {
            let mut api_manager = self.api_manager.lock().await;
            let _ = api_manager
                .download_api()
                .await
                .map_err(|e| self.client.log_message(MessageType::ERROR, e.to_string()));
        } else if params.command == "rblx-react-lsp.readCache" {
            let args = params.arguments;
            if let Some(Value::String(path_str)) = args.get(0) {
                let path = PathBuf::from(path_str);
                if path.exists() {
                    let _ = create_api_file_readable(path)
                        .await
                        .map_err(|e| self.client.log_message(MessageType::ERROR, e.to_string()));
                } else {
                    self.client
                        .log_message(MessageType::LOG, "Failed to find path from arguments!")
                        .await;
                }
            }
        }

        Ok(None)
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
        api_manager: Arc::new(Mutex::new(ApiManager::new())),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
