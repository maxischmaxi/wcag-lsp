# wcag-lsp

A fast Language Server Protocol (LSP) implementation that checks HTML and JSX/TSX code for [WCAG 2.1](https://www.w3.org/WAI/WCAG21/Understanding/) accessibility violations in real-time.

Built with Rust and [tree-sitter](https://tree-sitter.github.io/) for incremental parsing.

## Features

- Real-time WCAG diagnostics as you type (150ms debounce)
- 40 rules covering WCAG 2.1/2.2 Level A and AA criteria
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

| Key   | Values                 | Default     |
| ----- | ---------------------- | ----------- |
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

| Value                           | Effect                                             |
| ------------------------------- | -------------------------------------------------- |
| `"off"`, `"false"`, `"disable"` | Disable the rule entirely                          |
| `"error"`                       | Report as error regardless of WCAG level default   |
| `"warning"`, `"warn"`           | Report as warning regardless of WCAG level default |

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

wcag-lsp includes 40 rules covering WCAG 2.1/2.2 Level A and AA criteria.

See [RULES.md](RULES.md) for the complete rule reference and WCAG criterion coverage matrix.

## Contributing

### Prerequisites

- Rust 1.85+ (edition 2024)

### Setup

```sh
git config core.hooksPath .githooks
```

### Adding a new rule

1. Create `src/rules/my_rule.rs` implementing the `Rule` trait
2. Add `pub mod my_rule;` to `src/rules/mod.rs`
3. Add `Box::new(my_rule::MyRule)` to the `all_rules()` vec in `mod.rs`
4. Write tests in the same file using `#[cfg(test)]`

Each rule receives the tree-sitter root node, the source text, and the file type. Return a `Vec<Diagnostic>` with violations found.

## License

MIT
