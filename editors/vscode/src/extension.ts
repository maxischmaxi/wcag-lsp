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
let configWatchers: vscode.FileSystemWatcher[] = [];
let restartDebounceTimer: ReturnType<typeof setTimeout> | undefined;

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

function isValidConfig(content: string, filePath: string): boolean {
  if (filePath.endsWith(".json")) {
    try {
      JSON.parse(content);
      return true;
    } catch {
      return false;
    }
  }
  // For TOML, accept any non-empty content — the server handles invalid TOML gracefully
  return content.trim().length > 0;
}

function getServerPath(storageDir: string): string | undefined {
  const cfg = vscode.workspace.getConfiguration("wcag-lsp");
  const path = cfg.get<string>("serverPath", "");
  return path || getBinaryPath(storageDir) || undefined;
}

async function restartServer(storageDir: string): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }

  let serverPath = getServerPath(storageDir);
  if (!serverPath) {
    serverPath = await ensureBinary(storageDir);
  }

  await startClient(serverPath);
  updateStatusBar();
}

function handleConfigChange(uri: vscode.Uri, storageDir: string): void {
  if (restartDebounceTimer) {
    clearTimeout(restartDebounceTimer);
  }

  restartDebounceTimer = setTimeout(async () => {
    try {
      const content = await vscode.workspace.fs.readFile(uri);
      const text = Buffer.from(content).toString("utf-8");

      if (!isValidConfig(text, uri.fsPath)) {
        return;
      }

      await restartServer(storageDir);
    } catch {
      // File might have been deleted, ignore
    }
  }, 500);
}

function setupConfigWatchers(
  context: vscode.ExtensionContext,
  storageDir: string,
): void {
  // Dispose old watchers
  for (const w of configWatchers) {
    w.dispose();
  }
  configWatchers = [];

  const cfg = vscode.workspace.getConfiguration("wcag-lsp");
  const configPath = cfg.get<string>("configPath", "");

  if (configPath) {
    // Watch the specific config file
    const pattern = new vscode.RelativePattern(
      vscode.workspace.workspaceFolders?.[0] ?? "",
      configPath,
    );
    const watcher = vscode.workspace.createFileSystemWatcher(pattern);
    watcher.onDidChange((uri) => handleConfigChange(uri, storageDir));
    watcher.onDidCreate((uri) => handleConfigChange(uri, storageDir));
    configWatchers.push(watcher);
    context.subscriptions.push(watcher);
  } else {
    // Watch .wcag.toml and .wcag.json in workspace root
    for (const filename of [".wcag.toml", ".wcag.json"]) {
      const pattern = new vscode.RelativePattern(
        vscode.workspace.workspaceFolders?.[0] ?? "",
        filename,
      );
      const watcher = vscode.workspace.createFileSystemWatcher(pattern);
      watcher.onDidChange((uri) => handleConfigChange(uri, storageDir));
      watcher.onDidCreate((uri) => handleConfigChange(uri, storageDir));
      configWatchers.push(watcher);
      context.subscriptions.push(watcher);
    }
  }
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
      if (e.affectsConfiguration("wcag-lsp.configPath")) {
        setupConfigWatchers(context, storageDir);
        restartServer(storageDir).catch(() => {});
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

  // Watch config files for changes
  setupConfigWatchers(context, storageDir);

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
