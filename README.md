# wcag-lsp

A fast Language Server Protocol (LSP) implementation that checks HTML and JSX/TSX code for [WCAG 2.1](https://www.w3.org/WAI/WCAG21/Understanding/) accessibility violations in real-time.

Built with Rust and [tree-sitter](https://tree-sitter.github.io/) for incremental parsing.

## Features

- Real-time WCAG diagnostics as you type (150ms debounce)
- 14 rules covering WCAG 2.1 Level A criteria
- Supports HTML, JSX, TSX, Vue, Svelte, Astro, PHP, ERB, Handlebars, and Twig
- Configurable severity levels and per-rule overrides
- Glob-based file ignore patterns

## Installation

### Quick install (Linux / macOS)

```sh
curl -fsSL https://raw.githubusercontent.com/maxischmaxi/wcag-lsp/main/install.sh | sh
```

Installs to `~/.local/bin`. Override with `WCAG_LSP_INSTALL_DIR`:

```sh
curl -fsSL https://raw.githubusercontent.com/maxischmaxi/wcag-lsp/main/install.sh | WCAG_LSP_INSTALL_DIR=/usr/local/bin sh
```

Install a specific version:

```sh
curl -fsSL https://raw.githubusercontent.com/maxischmaxi/wcag-lsp/main/install.sh | sh -s v0.1.0
```

### Windows

Download `wcag-lsp-x86_64-pc-windows-msvc.zip` from the [latest release](https://github.com/maxischmaxi/wcag-lsp/releases/latest), extract it, and add the directory to your `PATH`.

### From source

```sh
cargo build --release
```

The binary is at `target/release/wcag-lsp`.

## Editor Setup

### Neovim (0.11+)

```lua
vim.lsp.config("wcag_lsp", {
    cmd = { "/path/to/wcag-lsp" },
    filetypes = { "html", "javascriptreact", "typescriptreact", "vue", "svelte" },
    root_markers = { ".wcag-lsp.toml", ".git" },
})
vim.lsp.enable("wcag_lsp")
```

### VS Code

Add to `.vscode/settings.json`:

```json
{
    "lsp.server.wcag-lsp": {
        "command": "/path/to/wcag-lsp",
        "languages": ["html", "javascriptreact", "typescriptreact", "vue", "svelte"]
    }
}
```

Or use a generic LSP client extension (e.g. [vscode-lsp-client](https://marketplace.visualstudio.com/items?itemName=AGerusworker.vscode-lsp-client)).

## Configuration

Create a `.wcag-lsp.toml` file in your project root. All sections are optional -- without a config file, the default settings apply.

### Full example

```toml
[severity]
A = "error"
AA = "warning"
AAA = "warning"

[rules]
heading-order = "off"
img-alt = "warning"
no-redundant-alt = "error"

[ignore]
patterns = ["node_modules/**", "dist/**", "build/**"]
```

### `[severity]` -- WCAG level defaults

Controls the default diagnostic severity for all rules of a given WCAG conformance level.

| Key   | Values                | Default     |
|-------|-----------------------|-------------|
| `A`   | `"error"`, `"warning"` | `"error"`   |
| `AA`  | `"error"`, `"warning"` | `"warning"` |
| `AAA` | `"error"`, `"warning"` | `"warning"` |

```toml
[severity]
A = "error"      # Level A violations are errors (default)
AA = "error"     # Treat Level AA as errors too
AAA = "warning"  # Level AAA stays as warnings (default)
```

### `[rules]` -- Per-rule overrides

Override severity or disable individual rules. This takes precedence over the `[severity]` section.

| Value                        | Effect                                           |
|------------------------------|--------------------------------------------------|
| `"off"`, `"false"`, `"disable"` | Disable the rule entirely                        |
| `"error"`                    | Report as error regardless of WCAG level default |
| `"warning"`, `"warn"`       | Report as warning regardless of WCAG level default |

```toml
[rules]
heading-order = "off"        # Don't check heading order
img-alt = "warning"          # Downgrade from error to warning
no-redundant-alt = "error"   # Upgrade from warning to error
```

### `[ignore]` -- File patterns

Glob patterns for files that should not be checked. Patterns are matched against the full file path.

```toml
[ignore]
patterns = [
    "node_modules/**",
    "dist/**",
    "build/**",
    "**/*.test.tsx",
    "**/fixtures/**",
]
```

## Rules

All rules are WCAG 2.1 Level A. Rule IDs are used in the `[rules]` config section.

| Rule ID | WCAG | Default | Description |
|---------|------|---------|-------------|
| `img-alt` | [1.1.1](https://www.w3.org/WAI/WCAG21/Understanding/non-text-content.html) | Error | `<img>` elements must have an `alt` attribute |
| `no-redundant-alt` | [1.1.1](https://www.w3.org/WAI/WCAG21/Understanding/non-text-content.html) | Warning | Alt text should not contain words like "image", "picture", "photo" |
| `media-captions` | [1.2.2](https://www.w3.org/WAI/WCAG21/Understanding/captions-prerecorded.html) | Warning | `<video>` and `<audio>` elements must have `<track>` captions |
| `form-label` | [1.3.1](https://www.w3.org/WAI/WCAG21/Understanding/info-and-relationships.html) | Error | Form elements (`<input>`, `<select>`, `<textarea>`) must have labels |
| `heading-order` | [1.3.1](https://www.w3.org/WAI/WCAG21/Understanding/info-and-relationships.html) | Warning | Heading levels should not be skipped (e.g. `<h1>` to `<h3>`) |
| `table-header` | [1.3.1](https://www.w3.org/WAI/WCAG21/Understanding/info-and-relationships.html) | Warning | `<table>` elements must contain `<th>` header cells |
| `click-events-have-key-events` | [2.1.1](https://www.w3.org/WAI/WCAG21/Understanding/keyboard.html) | Error | Elements with `onClick` must also have `onKeyDown` or `onKeyUp` |
| `meta-refresh` | [2.2.1](https://www.w3.org/WAI/WCAG21/Understanding/timing-adjustable.html) | Error | `<meta http-equiv="refresh">` must not have a time limit |
| `iframe-title` | [2.4.1](https://www.w3.org/WAI/WCAG21/Understanding/bypass-blocks.html) | Error | `<iframe>` elements must have a `title` attribute |
| `no-positive-tabindex` | [2.4.3](https://www.w3.org/WAI/WCAG21/Understanding/focus-order.html) | Warning | Avoid `tabindex` values greater than 0 |
| `anchor-content` | [2.4.4](https://www.w3.org/WAI/WCAG21/Understanding/link-purpose-in-context.html) | Error | `<a>` elements must have text content |
| `html-lang` | [3.1.1](https://www.w3.org/WAI/WCAG21/Understanding/language-of-page.html) | Error | `<html>` element must have a `lang` attribute |
| `aria-role` | [4.1.2](https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html) | Error | `role` attribute must be a valid ARIA role |
| `aria-props` | [4.1.2](https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html) | Error | `aria-*` attributes must be valid ARIA properties |

## Development

### Prerequisites

- Rust 1.85+ (edition 2024)

### Build & Test

```sh
cargo build           # debug build
cargo build --release # release build
cargo test            # run all tests (127 tests)
```

### Architecture

```text
src/
  main.rs        # Binary entrypoint (stdio transport)
  lib.rs         # Library re-exports
  server.rs      # LSP server (initialize, didOpen, didChange, didClose)
  parser.rs      # tree-sitter parser creation, FileType detection
  document.rs    # Document storage with incremental tree-sitter trees
  engine.rs      # Diagnostic runner (applies config, runs rules)
  config.rs      # .wcag-lsp.toml parsing
  rules/         # One file per rule, each implementing the Rule trait
```

### Adding a new rule

1. Create `src/rules/my_rule.rs` implementing the `Rule` trait
2. Add `pub mod my_rule;` to `src/rules/mod.rs`
3. Add `Box::new(my_rule::MyRule)` to the `all_rules()` vec in `mod.rs`
4. Write tests in the same file using `#[cfg(test)]`

Each rule receives the tree-sitter root node, the source text, and the file type. Return a `Vec<Diagnostic>` with violations found.

## License

MIT
