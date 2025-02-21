import path from 'path';
import * as vscode from 'vscode';
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind } from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: vscode.ExtensionContext) {
	const serverPath = context.asAbsolutePath(
		path.join('..', 'lsp-server', 'target', 'debug', 'React_lSP')
	)
	const serverOptions: ServerOptions = {
		run: { command: serverPath, transport: TransportKind.stdio },
		debug: { command: serverPath, transport: TransportKind.stdio }
	}
	const clientOptions: LanguageClientOptions = {
		documentSelector: [{ scheme: 'file', language: '.luau' }],
		synchronize: {
			fileEvents: vscode.workspace.createFileSystemWatcher('**/.clientrc')
		}
	}

	client = new LanguageClient(
		'rblxReactLSP',
		'RBLX React LSP',
		serverOptions,
		clientOptions
	)
	client.start()
}

export function deactivate(): Thenable<void> | undefined {
	if (!client) {
		return undefined
	}
	return client.stop()
}
