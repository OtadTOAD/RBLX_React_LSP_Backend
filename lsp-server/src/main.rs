use tower_lsp::{
    lsp_types::{
        InitializeParams, InitializeResult, InitializedParams, ServerCapabilities,
        TextDocumentSyncCapability, TextDocumentSyncKind,
    },
    LspService, Server,
};
type LspResult<T> = std::result::Result<T, tower_lsp::jsonrpc::Error>;

struct Backend;
#[tower_lsp::async_trait]
impl tower_lsp::LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> LspResult<InitializeResult> {
        print!("Init LSP Server...");
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            server_info: None,
        })
    }
    async fn initialized(&self, _params: InitializedParams) {
        println!("Server inited!");
    }
    async fn shutdown(&self) -> LspResult<()> {
        println!("Shutting down LSP Server...");
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|_client| Backend).finish();
    println!("Starting LSP Server...");
    Server::new(stdin, stdout, socket).serve(service).await;
}
