import * as path from "path";
import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";
import { ensureBinary } from "./download";

let client: LanguageClient | undefined;

export async function activate(
  context: vscode.ExtensionContext,
): Promise<void> {
  const config = vscode.workspace.getConfiguration("wcag-lsp");
  let serverPath = config.get<string>("serverPath", "");

  if (!serverPath) {
    try {
      serverPath = await ensureBinary(context.globalStorageUri.fsPath);
    } catch (err) {
      vscode.window.showErrorMessage(
        `WCAG LSP: Failed to download server: ${err}`,
      );
      return;
    }
  }

  const serverOptions: ServerOptions = {
    run: { command: serverPath },
    debug: { command: serverPath },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "html" },
      { scheme: "file", language: "javascriptreact" },
      { scheme: "file", language: "typescriptreact" },
      { scheme: "file", language: "vue" },
      { scheme: "file", language: "svelte" },
      { scheme: "file", language: "astro" },
      { scheme: "file", language: "php" },
      { scheme: "file", language: "erb" },
    ],
  };

  client = new LanguageClient(
    "wcag-lsp",
    "WCAG LSP",
    serverOptions,
    clientOptions,
  );

  await client.start();
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
  }
}
