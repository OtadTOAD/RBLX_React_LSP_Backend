import * as vscode from 'vscode';
import path from 'path';
import { LanguageClient, LanguageClientOptions, LogMessageParams, ServerOptions, TransportKind, MessageType } from 'vscode-languageclient/node';

// Just for debuggin so I know changes got thru
const ver = "V7";
let client: LanguageClient;

function messageTypeToString(type: MessageType): string {
	switch (type) {
		case MessageType.Error:
			return "Error";
		case MessageType.Warning:
			return "Warning";
		case MessageType.Info:
			return "Info";
		case MessageType.Log:
			return "Log";
		default:
			return "Unknown";
	}
}

// This method is called when your extension is activated
// Your extension is activated the very first time the command is executed
// I changed this to "*" so it activates instantly, probably should change so it only activates when .lua/.luau file is opened
export function activate(context: vscode.ExtensionContext) {
	// LSP SERVER INITIATION
	const lspLogChannel = vscode.window.createOutputChannel("React LSP Logs");

	const serverPath = context.asAbsolutePath(
		path.join('..', 'lsp-server', 'target', 'debug', 'React_LSP.exe')
	);
	const serverOpts: ServerOptions = {
		run: { command: serverPath, transport: TransportKind.stdio },
		debug: { command: serverPath, transport: TransportKind.stdio }
	};
	const clientOpts: LanguageClientOptions = {
		documentSelector: [{ scheme: 'file', language: 'lua' }, { scheme: 'file', language: 'plaintext' }],

		synchronize: {
			fileEvents: vscode.workspace.createFileSystemWatcher('**/.clientrc')
		}
	};

	console.log(`Server ${ver} starting...`);

	client = new LanguageClient(
		'rblxReactLSP',
		'RBLX React LSP',
		serverOpts,
		clientOpts
	);
	client.start().then(() => {
		console.log(`Server ${ver} started!`);

		client.onNotification("window/logMessage", (params: LogMessageParams) => {
			const logLine = `[${messageTypeToString(params.type)}] ${params.message}`;
			lspLogChannel.appendLine(logLine);
		});
		lspLogChannel.show(true);

		console.log(`Client ${ver} started!`);
	});
	context.subscriptions.push(client);
}

// This method is called when your extension is deactivated
export function deactivate() {
	if (!client) {
		return undefined;
	}
	return client.stop();
}
