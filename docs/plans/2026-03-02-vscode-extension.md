# wcag-lsp VS Code Extension Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a VS Code extension that auto-downloads the wcag-lsp binary and provides WCAG accessibility diagnostics, plus fix the README.

**Architecture:** Minimal TypeScript extension using `vscode-languageclient`. On first activation, downloads the platform-specific binary from GitHub Releases into `globalStorageUri`. Starts the LSP server as a child process. All WCAG config is handled by the server via `.wcag-lsp.toml`.

**Tech Stack:** TypeScript, `vscode-languageclient` 9.x, `@vscode/vsce` for packaging, Node.js `https`/`fs`/`zlib`/`tar` for binary download.

---

### Task 1: Scaffold the Extension Project

**Files:**
- Create: `editors/vscode/package.json`
- Create: `editors/vscode/tsconfig.json`
- Create: `editors/vscode/.vscodeignore`
- Create: `editors/vscode/.gitignore`

**Step 1: Create `editors/vscode/package.json`**

```json
{
  "name": "wcag-lsp",
  "displayName": "WCAG Accessibility Linter",
  "description": "Real-time WCAG 2.1 accessibility diagnostics for HTML, JSX, TSX, Vue, Svelte, and more",
  "version": "0.5.0",
  "publisher": "maxischmaxi",
  "license": "MIT",
  "repository": {
    "type": "git",
    "url": "https://github.com/maxischmaxi/wcag-lsp"
  },
  "engines": {
    "vscode": "^1.75.0"
  },
  "categories": ["Linters"],
  "keywords": ["wcag", "accessibility", "a11y", "linter", "html", "jsx", "tsx"],
  "activationEvents": [
    "onLanguage:html",
    "onLanguage:javascriptreact",
    "onLanguage:typescriptreact",
    "onLanguage:vue",
    "onLanguage:svelte",
    "onLanguage:astro",
    "onLanguage:php",
    "onLanguage:erb"
  ],
  "main": "./out/extension.js",
  "contributes": {
    "configuration": {
      "title": "WCAG LSP",
      "properties": {
        "wcag-lsp.serverPath": {
          "type": "string",
          "default": "",
          "description": "Path to the wcag-lsp binary. Leave empty to auto-download."
        }
      }
    }
  },
  "scripts": {
    "compile": "tsc -p ./",
    "watch": "tsc -watch -p ./",
    "package": "vsce package",
    "lint": "tsc --noEmit"
  },
  "dependencies": {
    "vscode-languageclient": "^9.0.1"
  },
  "devDependencies": {
    "@types/vscode": "^1.75.0",
    "@types/node": "^20.0.0",
    "typescript": "^5.5.0",
    "@vscode/vsce": "^3.0.0"
  }
}
```

**Step 2: Create `editors/vscode/tsconfig.json`**

```json
{
  "compilerOptions": {
    "module": "commonjs",
    "target": "ES2022",
    "outDir": "out",
    "rootDir": "src",
    "lib": ["ES2022"],
    "sourceMap": true,
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true
  },
  "exclude": ["node_modules", "out"]
}
```

**Step 3: Create `editors/vscode/.vscodeignore`**

```text
src/**
node_modules/**
tsconfig.json
.gitignore
```

**Step 4: Create `editors/vscode/.gitignore`**

```text
out/
node_modules/
*.vsix
```

**Step 5: Install dependencies**

```bash
cd editors/vscode && npm install
```

**Step 6: Verify TypeScript compiles (expect error — no source yet)**

```bash
cd editors/vscode && npx tsc --noEmit
```

Expected: Error about missing `src/extension.ts` (this is correct at this stage).

**Step 7: Commit**

```bash
git add editors/vscode/package.json editors/vscode/tsconfig.json editors/vscode/.vscodeignore editors/vscode/.gitignore editors/vscode/package-lock.json
git commit -m "feat(vscode): scaffold extension project"
```

---

### Task 2: Implement Binary Download Module

**Files:**
- Create: `editors/vscode/src/download.ts`

**Step 1: Create `editors/vscode/src/download.ts`**

This module handles:
- Platform/arch detection → GitHub Release asset URL
- Downloading the tar.gz/zip archive
- Extracting the binary to the target directory
- Making it executable (Unix)

```typescript
import * as https from "https";
import * as fs from "fs";
import * as path from "path";
import * as zlib from "zlib";
import * as vscode from "vscode";

const REPO = "maxischmaxi/wcag-lsp";
const BINARY_NAME = "wcag-lsp";

interface PlatformInfo {
  target: string;
  ext: string;
  binaryName: string;
}

function getPlatformInfo(): PlatformInfo {
  const platform = process.platform;
  const arch = process.arch;

  let target: string;
  switch (`${platform}-${arch}`) {
    case "linux-x64":
      target = "x86_64-unknown-linux-musl";
      break;
    case "linux-arm64":
      target = "aarch64-unknown-linux-musl";
      break;
    case "darwin-x64":
      target = "x86_64-apple-darwin";
      break;
    case "darwin-arm64":
      target = "aarch64-apple-darwin";
      break;
    case "win32-x64":
      target = "x86_64-pc-windows-msvc";
      break;
    default:
      throw new Error(`Unsupported platform: ${platform}-${arch}`);
  }

  const isWindows = platform === "win32";
  return {
    target,
    ext: isWindows ? "zip" : "tar.gz",
    binaryName: isWindows ? `${BINARY_NAME}.exe` : BINARY_NAME,
  };
}

async function getLatestVersion(): Promise<string> {
  return new Promise((resolve, reject) => {
    const options = {
      hostname: "api.github.com",
      path: `/repos/${REPO}/releases/latest`,
      headers: { "User-Agent": "wcag-lsp-vscode" },
    };
    https
      .get(options, (res) => {
        if (res.statusCode === 302 || res.statusCode === 301) {
          const location = res.headers.location;
          if (location) {
            const match = location.match(/\/tag\/([^/]+)$/);
            if (match) {
              resolve(match[1]);
              return;
            }
          }
        }
        let data = "";
        res.on("data", (chunk: Buffer) => (data += chunk));
        res.on("end", () => {
          try {
            const json = JSON.parse(data);
            resolve(json.tag_name);
          } catch {
            reject(new Error("Failed to parse GitHub release info"));
          }
        });
      })
      .on("error", reject);
  });
}

function downloadFile(url: string): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    https
      .get(url, { headers: { "User-Agent": "wcag-lsp-vscode" } }, (res) => {
        if (res.statusCode === 302 || res.statusCode === 301) {
          if (res.headers.location) {
            downloadFile(res.headers.location).then(resolve, reject);
            return;
          }
        }
        if (res.statusCode !== 200) {
          reject(new Error(`Download failed with status ${res.statusCode}`));
          return;
        }
        const chunks: Buffer[] = [];
        res.on("data", (chunk: Buffer) => chunks.push(chunk));
        res.on("end", () => resolve(Buffer.concat(chunks)));
      })
      .on("error", reject);
  });
}

async function extractTarGz(
  archive: Buffer,
  destDir: string,
  binaryName: string
): Promise<string> {
  // Simple tar.gz extraction: decompress, then parse tar format
  const decompressed = zlib.gunzipSync(archive);
  // Parse tar: each entry is 512-byte header + data rounded to 512
  let offset = 0;
  while (offset < decompressed.length) {
    const header = decompressed.subarray(offset, offset + 512);
    if (header.every((b) => b === 0)) break;

    const name = header.subarray(0, 100).toString("utf8").replace(/\0/g, "");
    const sizeOctal = header
      .subarray(124, 136)
      .toString("utf8")
      .replace(/\0/g, "")
      .trim();
    const size = parseInt(sizeOctal, 8);
    offset += 512;

    const fileName = path.basename(name);
    if (fileName === binaryName && size > 0) {
      const destPath = path.join(destDir, binaryName);
      fs.writeFileSync(destPath, decompressed.subarray(offset, offset + size));
      fs.chmodSync(destPath, 0o755);
      return destPath;
    }

    offset += Math.ceil(size / 512) * 512;
  }
  throw new Error(`Binary '${binaryName}' not found in archive`);
}

async function extractZip(
  archive: Buffer,
  destDir: string,
  binaryName: string
): Promise<string> {
  // Minimal zip extraction for a single file
  // Find End of Central Directory
  let eocdOffset = archive.length - 22;
  while (eocdOffset >= 0 && archive.readUInt32LE(eocdOffset) !== 0x06054b50) {
    eocdOffset--;
  }
  if (eocdOffset < 0) throw new Error("Invalid zip file");

  const cdOffset = archive.readUInt32LE(eocdOffset + 16);
  let pos = cdOffset;

  while (pos < eocdOffset) {
    if (archive.readUInt32LE(pos) !== 0x02014b50) break;
    const nameLen = archive.readUInt16LE(pos + 28);
    const extraLen = archive.readUInt16LE(pos + 30);
    const commentLen = archive.readUInt16LE(pos + 32);
    const localHeaderOffset = archive.readUInt32LE(pos + 42);
    const name = archive.subarray(pos + 46, pos + 46 + nameLen).toString("utf8");

    if (path.basename(name) === binaryName) {
      // Read from local header
      const localNameLen = archive.readUInt16LE(localHeaderOffset + 26);
      const localExtraLen = archive.readUInt16LE(localHeaderOffset + 28);
      const compSize = archive.readUInt32LE(localHeaderOffset + 18);
      const dataStart = localHeaderOffset + 30 + localNameLen + localExtraLen;
      const data = archive.subarray(dataStart, dataStart + compSize);
      const destPath = path.join(destDir, binaryName);
      fs.writeFileSync(destPath, data);
      return destPath;
    }

    pos += 46 + nameLen + extraLen + commentLen;
  }
  throw new Error(`Binary '${binaryName}' not found in zip`);
}

export async function ensureBinary(storageDir: string): Promise<string> {
  const info = getPlatformInfo();
  const binaryPath = path.join(storageDir, info.binaryName);

  if (fs.existsSync(binaryPath)) {
    return binaryPath;
  }

  const serverPath = await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: "WCAG LSP: Downloading server...",
      cancellable: false,
    },
    async (progress) => {
      progress.report({ message: "Fetching latest version..." });
      const version = await getLatestVersion();

      progress.report({
        message: `Downloading ${version} for ${info.target}...`,
      });
      const url = `https://github.com/${REPO}/releases/download/${version}/${BINARY_NAME}-${info.target}.${info.ext}`;
      const archive = await downloadFile(url);

      progress.report({ message: "Extracting..." });
      fs.mkdirSync(storageDir, { recursive: true });

      if (info.ext === "zip") {
        return extractZip(archive, storageDir, info.binaryName);
      }
      return extractTarGz(archive, storageDir, info.binaryName);
    }
  );

  return serverPath;
}
```

**Step 2: Verify compilation**

```bash
cd editors/vscode && npx tsc --noEmit
```

Expected: May have errors about missing extension.ts — that's fine, will be created in Task 3.

**Step 3: Commit**

```bash
git add editors/vscode/src/download.ts
git commit -m "feat(vscode): add binary download module"
```

---

### Task 3: Implement Extension Entry Point

**Files:**
- Create: `editors/vscode/src/extension.ts`

**Step 1: Create `editors/vscode/src/extension.ts`**

```typescript
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
  context: vscode.ExtensionContext
): Promise<void> {
  const config = vscode.workspace.getConfiguration("wcag-lsp");
  let serverPath = config.get<string>("serverPath", "");

  if (!serverPath) {
    try {
      serverPath = await ensureBinary(context.globalStorageUri.fsPath);
    } catch (err) {
      vscode.window.showErrorMessage(
        `WCAG LSP: Failed to download server: ${err}`
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
    clientOptions
  );

  await client.start();
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
  }
}
```

**Step 2: Verify full compilation**

```bash
cd editors/vscode && npx tsc -p ./
```

Expected: Compiles successfully, `out/` directory created with `.js` files.

**Step 3: Commit**

```bash
git add editors/vscode/src/extension.ts
git commit -m "feat(vscode): add extension entry point with LanguageClient"
```

---

### Task 4: Build and Verify VSIX Packaging

**Step 1: Compile TypeScript**

```bash
cd editors/vscode && npx tsc -p ./
```

Expected: Clean compilation.

**Step 2: Package as VSIX**

```bash
cd editors/vscode && npx vsce package --no-dependencies
```

Expected: Creates `wcag-lsp-0.5.0.vsix`.

**Step 3: Verify the VSIX contents look correct**

```bash
cd editors/vscode && unzip -l *.vsix | head -30
```

Expected: Should contain `extension/out/extension.js`, `extension/out/download.js`, `extension/package.json`, and NOT contain `src/`, `node_modules/`, or `tsconfig.json`.

**Step 4: Commit (only if changes were needed)**

```bash
git add editors/vscode/
git commit -m "feat(vscode): verify VSIX packaging"
```

---

### Task 5: Update README

**Files:**
- Modify: `README.md` (lines 60-71, the VS Code section)

**Step 1: Replace the VS Code section in `README.md`**

Replace lines 60-71 (the current broken VS Code section) with:

```markdown
### VS Code

Install the [WCAG Accessibility Linter](https://marketplace.visualstudio.com/items?itemName=maxischmaxi.wcag-lsp) extension from the Marketplace. It downloads the server binary automatically on first use.

Alternatively, if you prefer a generic LSP client, install [Generic LSP Client](https://marketplace.visualstudio.com/items?itemName=AlanWalk.vscode-lsp-client) and add to `.vscode/settings.json`:

```json
{
  "glspc.serverPath": "/path/to/wcag-lsp",
  "glspc.languageId": "html"
}
```

> Note: The generic client approach requires manual binary installation and only covers one language ID at a time.
```sql

**Step 2: Verify README renders correctly**

Read the file and visually check the markdown.

**Step 3: Commit**

```bash
git add README.md
git commit -m "docs: update VS Code setup instructions"
```

---

### Task 6: Add Extension README for Marketplace

**Files:**
- Create: `editors/vscode/README.md`

**Step 1: Create `editors/vscode/README.md`**

```markdown
# WCAG Accessibility Linter

Real-time WCAG 2.1/2.2 accessibility diagnostics for HTML, JSX, TSX, Vue, Svelte, and more.

## Features

- 40 rules covering WCAG 2.1/2.2 Level A and AA criteria
- Real-time diagnostics as you type
- Configurable severity levels and per-rule overrides
- Supports HTML, JSX, TSX, Vue, Svelte, Astro, PHP, ERB

## Setup

Install the extension — it downloads the wcag-lsp server automatically on first use.

To use a custom server binary, set `wcag-lsp.serverPath` in your VS Code settings.

## Configuration

Create a `.wcag-lsp.toml` in your project root:

\`\`\`toml
[severity]
A = "error"
AA = "warning"

[rules]
heading-order = "off"
img-alt = "warning"

[ignore]
patterns = ["node_modules/**", "dist/**"]
\`\`\`

See the [full documentation](https://github.com/maxischmaxi/wcag-lsp) for details.
```

**Step 2: Commit**

```bash
git add editors/vscode/README.md
git commit -m "docs(vscode): add Marketplace README"
```
