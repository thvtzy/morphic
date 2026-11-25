// Morphic for VS Code — the thin client
// (DISEBAR_15_SCOPE.md §6).
//
// The whole client: read the one configuration key,
// start `morphic lsp` over stdio through the generic
// LanguageClient, and let the server's diagnostics be
// the product. Diagnostics-only means diagnostics
// only — this entry registers no hover, completion,
// definition, formatting, or code-action provider of
// its own, and contributes no commands or views. The
// syntax grammar shipped beside it is highlighting
// decoration; it carries no diagnostic claim.

import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
    // `morphic.serverPath` — the binary whose `lsp`
    // subcommand serves the session. Its contributed
    // default is `morphic`, a PATH lookup; the fallback
    // here only covers a hand-edited settings value.
    const configured = vscode.workspace
        .getConfiguration('morphic')
        .get<string>('serverPath');
    const serverPath =
        configured !== undefined && configured !== '' ? configured : 'morphic';

    const serverOptions: ServerOptions = {
        command: serverPath,
        args: ['lsp'],
        transport: TransportKind.stdio,
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'morphic' }],
    };

    client = new LanguageClient(
        'morphic',
        'Morphic Language Server',
        serverOptions,
        clientOptions,
    );
    context.subscriptions.push({
        dispose: () => {
            void client?.stop();
        },
    });
    void client.start();
}

export async function deactivate(): Promise<void> {
    if (client !== undefined) {
        await client.stop();
        client = undefined;
    }
}
