# WCAG Accessibility Linter

Real-time WCAG 2.1/2.2 accessibility diagnostics for HTML, JSX, TSX, Vue, Svelte, and more.

## Features

- 40 rules covering WCAG 2.1/2.2 Level A and AA criteria
- Real-time diagnostics as you type
- Configurable severity levels and per-rule overrides
- Supports HTML, JSX, TSX, Vue, Svelte, Astro, PHP, ERB
- Inline `wcag-disable`, `wcag-disable-line`, and `wcag-disable-next-line` directives

## Setup

Install the extension — it downloads the wcag-lsp server automatically on first use.

### Recommended Extension

Add wcag-lsp as a recommended extension for your workspace in `.vscode/extensions.json`:

```json
{
  "recommendations": ["maxischmaxi.wcag-lsp"]
}
```

## Configuration

Create a `.wcag.toml` (or `.wcag.json`) in your project root:

```toml
[severity]
A = "error"
AA = "warning"

[rules]
heading-order = "off"
img-alt = "warning"

[ignore]
patterns = ["node_modules/**", "dist/**"]
```

Or equivalently in `.wcag.json`:

```json
{
  "$schema": "https://raw.githubusercontent.com/maxischmaxi/wcag-lsp/main/wcag-lsp.schema.json",
  "severity": { "A": "error", "AA": "warning" },
  "rules": { "heading-order": "off", "img-alt": "warning" },
  "ignore": { "patterns": ["node_modules/**", "dist/**"] }
}
```

### Custom Config Path

By default, wcag-lsp looks for `.wcag.toml` or `.wcag.json` in your project root. To use a config file at a custom location, set `wcag-lsp.configPath` in your VS Code settings:

```json
{
  "wcag-lsp.configPath": "./configs/.wcag.toml"
}
```

### Custom Server Binary

To use a custom server binary, set `wcag-lsp.serverPath` in your VS Code settings.

## Inline disable directives

Use comment directives for local suppressions:

```html
<!-- wcag-disable AA img-alt -->
<!-- wcag-disable-line img-alt --><img src="photo.jpg" />
<!-- wcag-disable-next-line img-alt -->
```

```tsx
/* wcag-disable */
/* wcag-disable-line img-alt */ <img src="photo.jpg" />
// wcag-disable-next-line img-alt
```

`wcag-disable` works file-wide only when it appears in the comment header at the top of the file.

See the [full documentation](https://github.com/maxischmaxi/wcag-lsp) for details.
