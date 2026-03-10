import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";
import {
  ensureBinary,
  downloadBinary,
  getBinaryPath,
  updateBinaryIfNeeded,
} from "./download";

let client: LanguageClient | undefined;
let statusBarItem: vscode.StatusBarItem | undefined;

function updateStatusBar(): void {
  if (!statusBarItem) {
    return;
  }

  const config = vscode.workspace.getConfiguration("wcag-lsp");
  const showStatusBar = config.get<boolean>("showStatusBar", true);

  if (!showStatusBar) {
    statusBarItem.hide();
    return;
  }

  if (client && client.isRunning()) {
    const version = client.initializeResult?.serverInfo?.version;
    if (version) {
      statusBarItem.text = `$(check) WCAG LSP v${version}`;
      statusBarItem.tooltip = `WCAG LSP Server v${version} — Running`;
    } else {
      statusBarItem.text = `$(check) WCAG LSP`;
      statusBarItem.tooltip = `WCAG LSP Server — Running`;
    }
  } else {
    statusBarItem.text = `$(sync~spin) WCAG LSP Starting...`;
    statusBarItem.tooltip = `WCAG LSP Server — Starting`;
  }

  statusBarItem.show();
}

async function startClient(serverPath: string): Promise<void> {
  const serverOptions: ServerOptions = {
    run: { command: serverPath },
    debug: { command: serverPath },
  };

  const config = vscode.workspace.getConfiguration("wcag-lsp");
  const configPath = config.get<string>("configPath", "");

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
    initializationOptions: {
      configPath: configPath || undefined,
    },
  };

  client = new LanguageClient(
    "wcag-lsp",
    "WCAG LSP",
    serverOptions,
    clientOptions,
  );

  updateStatusBar();
  await client.start();
  updateStatusBar();
}

export async function activate(
  context: vscode.ExtensionContext,
): Promise<void> {
  const storageDir = context.globalStorageUri.fsPath;

  statusBarItem = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Right,
    0,
  );
  context.subscriptions.push(statusBarItem);

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("wcag-lsp.showStatusBar")) {
        updateStatusBar();
      }
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("wcag-lsp.installServer", async () => {
      const existing = getBinaryPath(storageDir);

      if (existing) {
        const choice = await vscode.window.showInformationMessage(
          "WCAG LSP: Server is already installed. Reinstall?",
          "Reinstall",
          "Cancel",
        );
        if (choice !== "Reinstall") {
          return;
        }
      }

      try {
        const serverPath = await downloadBinary(storageDir);
        const restart = await vscode.window.showInformationMessage(
          `WCAG LSP: Server installed at ${serverPath}`,
          "Restart LSP",
        );
        if (restart === "Restart LSP") {
          if (client) {
            await client.stop();
            client = undefined;
          }
          await startClient(serverPath);
        }
      } catch (err) {
        vscode.window.showErrorMessage(
          `WCAG LSP: Failed to install server: ${err}`,
        );
      }
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("wcag-lsp.updateServer", async () => {
      try {
        const updatedPath = await updateBinaryIfNeeded(storageDir);
        if (!updatedPath) {
          vscode.window.showInformationMessage(
            "WCAG LSP: Server is already up to date.",
          );
          return;
        }
        if (client) {
          await client.stop();
          client = undefined;
        }
        await startClient(updatedPath);
        vscode.window.showInformationMessage(
          "WCAG LSP: Server updated and restarted.",
        );
      } catch (err) {
        vscode.window.showErrorMessage(
          `WCAG LSP: Failed to update server: ${err}`,
        );
      }
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("wcag-lsp.restartServer", async () => {
      try {
        if (client) {
          await client.stop();
          client = undefined;
        }

        const cfg = vscode.workspace.getConfiguration("wcag-lsp");
        let path = cfg.get<string>("serverPath", "");
        if (!path) {
          path = getBinaryPath(storageDir) ?? (await ensureBinary(storageDir));
        }

        await startClient(path);
        vscode.window.showInformationMessage("WCAG LSP: Server restarted.");
      } catch (err) {
        vscode.window.showErrorMessage(
          `WCAG LSP: Failed to restart server: ${err}`,
        );
      }
    }),
  );

  const config = vscode.workspace.getConfiguration("wcag-lsp");
  let serverPath = config.get<string>("serverPath", "");

  if (!serverPath) {
    try {
      serverPath = await ensureBinary(storageDir);
    } catch (err) {
      vscode.window.showErrorMessage(
        `WCAG LSP: Failed to download server: ${err}`,
      );
      return;
    }
  }

  try {
    await startClient(serverPath);
  } catch (err) {
    vscode.window.showErrorMessage(
      `WCAG LSP: Failed to start server at "${serverPath}": ${err}`,
    );
  }

  // Check for updates in the background
  updateBinaryIfNeeded(storageDir).then(async (updatedPath) => {
    if (!updatedPath) {
      return;
    }
    const action = await vscode.window.showInformationMessage(
      "WCAG LSP: A new server version has been installed.",
      "Restart LSP",
    );
    if (action === "Restart LSP") {
      if (client) {
        await client.stop();
        client = undefined;
      }
      await startClient(updatedPath);
    }
  });
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
  }
}
