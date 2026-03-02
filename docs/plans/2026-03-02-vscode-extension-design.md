# VS Code Extension Design

## Goal

Build a VS Code extension for wcag-lsp that auto-downloads the server binary and provides WCAG diagnostics out of the box. Also update the README with correct VS Code setup instructions.

## Architecture

Minimal TypeScript extension using `vscode-languageclient`. The extension:

1. Activates on HTML/JSX/TSX/Vue/Svelte/Astro/PHP/ERB files
2. Downloads the correct wcag-lsp binary from GitHub Releases on first activation
3. Starts the LSP server as a child process via `LanguageClient`
4. Diagnostics flow automatically through the LSP protocol

## Project Structure

```text
editors/vscode/
├── package.json          # Extension manifest, contribution points, dependencies
├── tsconfig.json         # TypeScript config
├── .vscodeignore         # Files to exclude from VSIX package
├── src/
│   ├── extension.ts      # Entry point: activation, LanguageClient setup
│   └── download.ts       # Binary download from GitHub Releases
└── README.md             # Marketplace description
```

## Activation Flow

1. VS Code opens a supported file type → extension activates
2. Check for user-configured `wcag-lsp.serverPath` setting
3. If no custom path: check `context.globalStorageUri` for existing binary
4. If no binary found: download from GitHub Releases with progress notification
5. Start `LanguageClient` with the resolved binary path
6. LSP server handles diagnostics, config loading (.wcag-lsp.toml), and file filtering

## Settings

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `wcag-lsp.serverPath` | `string` | `""` | Optional manual path to wcag-lsp binary |

All other configuration is handled by `.wcag-lsp.toml` in the project root (parsed by the LSP server itself).

## Binary Download

Platform/arch mapping to GitHub Release assets:

| VS Code platform | Asset name |
|---|---|
| `linux-x64` | `wcag-lsp-x86_64-unknown-linux-musl.tar.gz` |
| `linux-arm64` | `wcag-lsp-aarch64-unknown-linux-musl.tar.gz` |
| `darwin-x64` | `wcag-lsp-x86_64-apple-darwin.tar.gz` |
| `darwin-arm64` | `wcag-lsp-aarch64-apple-darwin.tar.gz` |
| `win32-x64` | `wcag-lsp-x86_64-pc-windows-msvc.zip` |

Download goes into `context.globalStorageUri`. Progress shown via `vscode.window.withProgress`.

## Supported Languages

`html`, `javascriptreact`, `typescriptreact`, `vue`, `svelte`, `astro`, `php`, `erb`

## Distribution

- Published on VS Code Marketplace
- Extension lives in `editors/vscode/` within the wcag-lsp monorepo

## README Updates

- Replace current (broken) VS Code section with link to Marketplace extension
- Add alternative instructions for generic LSP client extensions
