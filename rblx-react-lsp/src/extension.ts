import * as vscode from 'vscode';
import path from 'path';
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind } from 'vscode-languageclient/node';

// Just for debuggin so I know changes got thru
const ver = "V3";
let client: LanguageClient;

// This method is called when your extension is activated
// Your extension is activated the very first time the command is executed
export function activate(context: vscode.ExtensionContext) {
	const serverPath = context.asAbsolutePath(
		path.join('..', 'lsp-server', 'target', 'debug', 'React_LSP.exe')
	);
	const serverOpts: ServerOptions = {
		run: { command: serverPath, transport: TransportKind.stdio },
		debug: { command: serverPath, transport: TransportKind.stdio }
	};
	const clientOpts: LanguageClientOptions = {
		documentSelector: [{ scheme: 'file', language: 'lua' }],
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
	});
}

// This method is called when your extension is deactivated
export function deactivate() {
	if (!client) {
		return undefined;
	}
	return client.stop();
}
