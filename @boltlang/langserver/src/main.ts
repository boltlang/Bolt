
import net from "net"
import minimist from "minimist"
import {
  createConnection,
  Diagnostic,
  DiagnosticSeverity,
  DidChangeConfigurationNotification,
  IConnection,
  InitializeParams,
  InitializeResult,
  ProposedFeatures,
  TextDocuments,
  TextDocumentSyncKind
} from "vscode-languageserver";
import { TextDocument } from "vscode-languageserver-textdocument";

import { createTokenStream, ParseError, ScanError } from "@boltlang/compiler/common";
import { Parser } from "@boltlang/compiler/parser";
import { Scanner } from "@boltlang/compiler/scanner";
import { TextFile } from "@boltlang/compiler/text";
import { FastStringMap } from "@boltlang/compiler/util";

const args = minimist(process.argv.slice(2))

let connection: IConnection;

if (args.socket) {
  if (typeof(args.socket) !== 'number') {
    console.error(`The flag --socket=PORT must contain a valid port number when provided.`)
    process.exit(1);
  }
  const server = net.createServer(socket => {
    const connection = createConnection(ProposedFeatures.all, socket, socket);
    configureConnection(connection)
  });
  server.listen(args.socket);
} else {
  const connection = createConnection(ProposedFeatures.all);
  configureConnection(connection);
}

interface BoltWorkspaceFolder {
  name: string;
  uri: string;
}

function configureConnection(connection: IConnection) {

  const workspaceFolders = new FastStringMap<string, BoltWorkspaceFolder>()

  const documents = new TextDocuments(TextDocument);

  let hasConfigurationCapability = false;
  let hasWorkspaceFolderCapability = false;
  let hasDiagnosticRelatedInformationCapability = false;

  connection.onInitialize((params: InitializeParams) => {

    const capabilities = params.capabilities

    hasConfigurationCapability = !!(capabilities.workspace && capabilities.workspace.configuration);
    hasWorkspaceFolderCapability = !!(capabilities.workspace && capabilities.workspace.workspaceFolders);
    hasDiagnosticRelatedInformationCapability = !!(capabilities.textDocument
      && capabilities.textDocument.publishDiagnostics
      && capabilities.textDocument.publishDiagnostics.relatedInformation);

    const result: InitializeResult = {
      capabilities: {
        textDocumentSync: TextDocumentSyncKind.Incremental,
      }
    }

    if (hasWorkspaceFolderCapability) {
      result.capabilities.workspace = {
        workspaceFolders: { supported: true }
      }
    }

    return result
  });

  connection.onInitialized(() => {
    if (hasConfigurationCapability) {
      connection.client.register(DidChangeConfigurationNotification.type, undefined);
    }
    if (hasWorkspaceFolderCapability) {
      connection.workspace.onDidChangeWorkspaceFolders(event => {
        for (const removed of event.removed) {
          workspaceFolders.delete(removed.uri)
        }
        for (const added of event.added) {
          workspaceFolders.set(added.uri, { name: added.name, uri: added.uri })
        }
      })
    }
  });

  documents.onDidChangeContent(event => {
    validateSourceFile(event.document)
  })

  function validateSourceFile(document: TextDocument) {
    const file = new TextFile(document.uri)
    const scanner = new Scanner(file, document.getText());
    const tokens = createTokenStream(scanner);
    const parser = new Parser()
    const diagnostics: Diagnostic[] = [];
    let sourceFile;
    try {
      sourceFile = parser.parseSourceFile(tokens)
    } catch (e) {
      if (e instanceof ParseError) {
        const span = e.actual.span!;
        diagnostics.push({
          severity: DiagnosticSeverity.Error,
          range: {
            start: { line: span.start.line-1, character: span.start.column-1 },
            end: { line: span.end.line-1, character: span.end.column-1 }
          },
          message: e.errorText
        });
        connection.sendDiagnostics({ uri: document.uri, diagnostics })
        return;
      } else if (e instanceof ScanError) {
        diagnostics.push({
          severity: DiagnosticSeverity.Error,
          range: {
            start: { line: e.position.line-1, character: e.position.column-1 },
            end: { line: e.position.line-1, character: e.position.column-1 },
          },
          message: e.errorText
        });
        connection.sendDiagnostics({ uri: document.uri, diagnostics })
        return;
      } else {
        throw e;
      }
    }
    connection.sendDiagnostics({ uri: document.uri, diagnostics })
  }

  documents.listen(connection)

  connection.listen()

}