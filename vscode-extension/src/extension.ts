import * as net from "net"
import * as path from 'path';
import { workspace, ExtensionContext } from 'vscode';

import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    StreamInfo,
    TransportKind
} from 'vscode-languageclient';

let client: LanguageClient;

function spawnLanguageClient(context: ExtensionContext) {

    if (client) {
        return;
    }

    const config = workspace.getConfiguration('bolt');

    const watchLanguageServer = config.get('watchLanguageServer') as boolean;

    let pathToLanguageServer = config.get('pathToLanguageServer') as string | null;

    if (pathToLanguageServer === null) {
        pathToLanguageServer = context.asAbsolutePath(path.join('bin', 'langserver.js'));
    }

    let serverOptions: ServerOptions;

    const languageServerPort = process.env['VSCODE_BOLT_LANGUAGE_SERVER_PORT']

    if (languageServerPort !== undefined) {
        const port = Number(languageServerPort)
        if (isNaN(port)) {
            return;
        }
        serverOptions = () => {
            const socket = net.connect(port)
            return Promise.resolve({ reader: socket, writer: socket });
        }
    } else {
        serverOptions = { module: pathToLanguageServer, transport: TransportKind.ipc }
    };

    // Options to control the language client
    let clientOptions: LanguageClientOptions = {
        // Register the server for plain text documents
        documentSelector: [{ scheme: 'file', language: 'bolt' }],
        synchronize: {
            // Notify the server about file changes to '.clientrc files contained in the workspace
            fileEvents: workspace.createFileSystemWatcher('**/*.bolt')
        }
    };

    // Create the language client and start the client.
    client = new LanguageClient(
        'bolt-client',
        'Bolt Language Server',
        serverOptions,
        clientOptions
    );

    // Start the client. This will also launch the server
    client.start();
}

export function activate(context: ExtensionContext) {
    spawnLanguageClient(context);
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}