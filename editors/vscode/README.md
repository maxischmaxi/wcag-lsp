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
  "severity": { "A": "error", "AA": "warning" },
  "rules": { "heading-order": "off", "img-alt": "warning" },
  "ignore": { "patterns": ["node_modules/**", "dist/**"] }
}
```

See the [full documentation](https://github.com/maxischmaxi/wcag-lsp) for details.
