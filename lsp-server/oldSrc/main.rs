mod metadata;
mod parser;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use metadata::generate_api_metadata;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
struct Backend {
    client: Client,
    docs: Arc<RwLock<HashMap<Url, String>>>,
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

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;

        {
            let mut docs = self.docs.write().await;
            docs.insert(uri.clone(), text.clone());
        }

        let diags = parser::parse_doc(uri.clone(), &text);
        self.client.publish_diagnostics(uri, diags, None).await;
        self.client
            .log_message(MessageType::INFO, "File opened!")
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.content_changes[0].text.clone(); // TODO: CHANGE THIS

        {
            let mut docs = self.docs.write().await;
            docs.insert(uri.clone(), text.clone());
        }

        let diags = parser::parse_doc(uri.clone(), &text);
        self.client.publish_diagnostics(uri, diags, None).await;
        self.client
            .log_message(MessageType::INFO, "File changed!")
            .await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        let docs = self.docs.read().await;
        let doc_text = if let Some(text) = docs.get(&uri) {
            text.clone()
        } else {
            return Ok(Some(CompletionResponse::Array(vec![])));
        };

        let suggestions = parser::get_property_completions(&doc_text, pos);
        if suggestions.is_empty() {
            self.client
                .log_message(MessageType::INFO, "No Completions returned!")
                .await;
        } else {
            self.client
                .log_message(MessageType::INFO, "Completions returned!")
                .await;
        }

        Ok(Some(CompletionResponse::Array(suggestions)))
    }

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> Result<Option<serde_json::Value>> {
        if params.command == "rblx-react-lsp.genMetadata" {
            let output_path = "parsed_api_dump.json";

            let result = std::panic::catch_unwind(|| {
                tokio::spawn(async move {
                    generate_api_metadata(output_path).await;
                })
            });

            match result {
                Ok(_) => {
                    self.client
                        .log_message(
                            MessageType::INFO,
                            format!("Metadata saved to {}", output_path),
                        )
                        .await;
                }
                Err(err) => {
                    let err_msg = format!("Failed to generate metadata: {:?}", err);
                    self.client.log_message(MessageType::ERROR, err_msg).await;
                }
            }
        }
        Ok(None)
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        docs: Arc::new(RwLock::new(HashMap::new())),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
