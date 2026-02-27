# WCAG LSP Server — Design Document

## Overview

A Rust-based Language Server Protocol (LSP) server that statically analyzes HTML, JSX, TSX, Vue, Svelte (and other HTML-containing formats) for WCAG 2.1 accessibility violations and reports them as diagnostics in the editor.

## Goals

- Real-time WCAG violation detection as the user types
- Support for Neovim and VS Code
- Support for HTML, JSX, TSX, Vue, Svelte, Astro, and template languages (PHP, ERB, HBS) as fallback
- Configurable severity mapping and rule toggles via project-level config
- High performance through Rust + tree-sitter incremental parsing

## Non-Goals (v1)

- Quickfixes / Code Actions
- Hover information
- Completion
- Runtime/browser-dependent checks (color contrast, computed styles, focus order)

## Architecture

```bash
┌─────────────────────────────────────────────────────┐
│                    Editor (Neovim / VS Code)         │
│                         │ LSP Protocol (stdio)       │
└─────────────────────────┼───────────────────────────┘
                          │
┌─────────────────────────┼───────────────────────────┐
│                   wcag-lsp Server                    │
│                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────┐ │
│  │  LSP Layer   │→ │ Document     │→ │  Rule      │ │
│  │ (tower-lsp)  │  │ Manager      │  │  Engine    │ │
│  └──────────────┘  └──────┬───────┘  └─────┬──────┘ │
│                           │                │         │
│                    ┌──────┴───────┐  ┌─────┴──────┐ │
│                    │ Tree-sitter  │  │  WCAG      │ │
│                    │ Parser Pool  │  │  Rules     │ │
│                    │ (HTML/JSX/   │  │  (~50-60)  │ │
│                    │  Vue/Svelte) │  │            │ │
│                    └──────────────┘  └────────────┘ │
│                                                      │
│  ┌──────────────────────────────────────────────────┐│
│  │              Config Loader                       ││
│  │  (.wcag-lsp.toml in project root)                ││
│  └──────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────┘
```

### Components

1. **LSP Layer** — `tower-lsp-server` (community fork), handles `initialize`, `didOpen`, `didChange`, `didClose`, publishes diagnostics
2. **Document Manager** — Manages open documents, detects file type, holds tree-sitter trees per document
3. **Tree-sitter Parser Pool** — Loads appropriate grammar per file type, performs incremental parsing on changes
4. **Rule Engine** — Traverses the tree-sitter AST, executes WCAG rules, produces diagnostics with severity and WCAG reference
5. **Config Loader** — Reads `.wcag-lsp.toml` from project root, allows severity mapping and rule toggle configuration

## Rule Engine

### Rule Structure

```rust
pub struct Rule {
    pub id: &'static str,            // e.g. "img-alt"
    pub description: &'static str,   // e.g. "Images must have alt text"
    pub wcag_level: WcagLevel,       // A, AA, AAA
    pub wcag_criterion: &'static str, // e.g. "1.1.1"
    pub default_severity: Severity,   // Error or Warning
}

pub trait RuleCheck {
    fn check(&self, node: &tree_sitter::Node, source: &str, ctx: &RuleContext)
        -> Vec<Diagnostic>;
}
```

Each rule is a separate module (e.g. `rules/img_alt.rs`) implementing the `RuleCheck` trait. The Rule Engine traverses the AST once and dispatches relevant nodes to matching rules based on node type.

### Rule Categories

| Category | Example Rules | WCAG Level | Count |
|---|---|---|---|
| Images | `<img>` missing alt, redundant alt, decorative without empty alt | A | ~5 |
| ARIA Validity | Invalid `aria-*` attrs, invalid role values, abstract roles, missing required ARIA attrs | A | ~8 |
| ARIA Semantics | Interactive elements with non-interactive roles, `aria-hidden` on focusable | A | ~5 |
| Forms | `<label>` without `for`, form control without label | A | ~4 |
| Document Structure | Missing `<html lang>`, invalid lang, missing `<title>`, empty headings, heading hierarchy | A/AA | ~6 |
| Frames | `<iframe>` without title | A | ~1 |
| Deprecated Elements | `<blink>`, `<marquee>` | A | ~2 |
| Keyboard | onClick without onKeyDown/onKeyUp, onMouseOver without onFocus | A | ~4 |
| Links | Empty anchor content, ambiguous link text | A/AA | ~3 |
| Meta | `<meta http-equiv="refresh">` with time limit | A | ~2 |
| Autocomplete | Invalid autocomplete values | AA | ~1 |
| Media | `<video>`/`<audio>` without `<track>` | A/AA | ~3 |
| Tabindex | Non-interactive elements with positive tabindex | A | ~2 |
| Tables | `<table>` without `<th>`, `<th>` without scope | A | ~3 |

### JSX/TSX Normalization

A mapping layer normalizes JSX-specific differences: `className` → `class`, `htmlFor` → `for`, camelCase event handlers.

## Document Manager & Tree-sitter Integration

### File Type Detection

| Extension | Tree-sitter Grammar | Template Extraction |
|---|---|---|
| `.html`, `.htm` | `tree-sitter-html` | Direct, whole file |
| `.jsx` | `tree-sitter-javascript` (JSX included) | JSX expressions in return |
| `.tsx` | `tree-sitter-typescript` (TSX variant) | JSX expressions in return |
| `.vue` | `tree-sitter-vue` | `<template>` block |
| `.svelte` | `tree-sitter-svelte` | Template portion |
| `.astro` | `tree-sitter-astro` | Template after frontmatter |
| `.php`, `.erb`, `.hbs`, `.twig` | `tree-sitter-html` fallback | Best-effort HTML parsing |

### Incremental Parsing Flow

```text
1. Editor sends textDocument/didChange with edits
2. Document Manager applies edits to stored source text
3. Tree-sitter tree is incrementally updated:
   - tree.edit(InputEdit { ... })        // O(1) — marks affected nodes
   - parser.parse(new_source, old_tree)  // Sub-ms — only changed region
4. Rule Engine traverses the new tree
5. Diagnostics sent via publishDiagnostics to editor
```

### Document State

```rust
pub struct Document {
    pub uri: Url,
    pub language: Language,       // Html, Jsx, Tsx, Vue, Svelte, ...
    pub source: String,           // Current source text
    pub tree: tree_sitter::Tree,  // Current syntax tree
    pub version: i32,             // LSP document version
}
```

### Debouncing

Diagnostics are debounced with a ~100-150ms window to avoid unnecessary work during fast typing.

## Configuration

Config file: `.wcag-lsp.toml` in project root.

```toml
# WCAG Level Severity Mapping
[severity]
A = "error"       # Default
AA = "warning"    # Default
AAA = "warning"   # Default

# Override individual rule severity or disable rules
[rules]
img-alt = "error"
heading-order = "off"
click-events-have-key-events = "warning"

# Ignore files/directories
[ignore]
patterns = [
    "node_modules/**",
    "dist/**",
    "**/*.test.tsx",
]
```

Config resolution: Defaults → `.wcag-lsp.toml` overrides → no config file = defaults only.

Config reload: On `workspace/didChangeWatchedFiles` for `.wcag-lsp.toml` → reload config → re-diagnose all open documents.

## LSP Protocol Features (v1)

| Capability | Description |
|---|---|
| `textDocument/didOpen` | Parse file, send diagnostics |
| `textDocument/didChange` | Incremental parse, send diagnostics (debounced) |
| `textDocument/didClose` | Clean up document state, clear diagnostics |
| `textDocument/publishDiagnostics` | Push diagnostics to editor |
| `workspace/didChangeWatchedFiles` | Config reload on `.wcag-lsp.toml` change |
| `initialize` | Exchange capabilities, load config |

### Diagnostic Format

```json
{
  "range": { "start": { "line": 11, "character": 4 }, "end": { "line": 11, "character": 30 } },
  "severity": 1,
  "code": "img-alt",
  "codeDescription": { "href": "https://www.w3.org/WAI/WCAG21/Understanding/non-text-content.html" },
  "source": "wcag-lsp",
  "message": "Image element missing alt attribute [WCAG 1.1.1 Level A]"
}
```

## Project Structure

```text
wcag-lsp/
├── Cargo.toml
├── .wcag-lsp.toml
├── src/
│   ├── main.rs                 # Entrypoint, start stdio server
│   ├── server.rs               # LSP Server (tower-lsp LanguageServer impl)
│   ├── config.rs               # Config loading & parsing
│   ├── document.rs             # Document Manager (state per file)
│   ├── parser.rs               # Tree-sitter parser pool & file type detection
│   ├── engine.rs               # Rule Engine: AST traversal + dispatch
│   ├── diagnostic.rs           # Diagnostic creation & formatting
│   ├── jsx_normalize.rs        # JSX/TSX → HTML attribute mapping
│   └── rules/
│       ├── mod.rs              # Rule registry
│       ├── img_alt.rs
│       ├── aria_role.rs
│       ├── aria_props.rs
│       ├── form_label.rs
│       ├── html_lang.rs
│       ├── heading_order.rs
│       ├── iframe_title.rs
│       ├── click_events.rs
│       ├── no_redundant_alt.rs
│       ├── anchor_content.rs
│       ├── meta_refresh.rs
│       ├── media_captions.rs
│       ├── tabindex.rs
│       ├── table_header.rs
│       └── ...
├── queries/
│   ├── html_elements.scm
│   ├── jsx_elements.scm
│   └── ...
└── tests/
    ├── fixtures/
    │   ├── img_alt_pass.html
    │   ├── img_alt_fail.html
    │   └── ...
    └── rules/
        ├── test_img_alt.rs
        └── ...
```

## Dependencies

| Crate | Purpose |
|---|---|
| `tower-lsp-server` | LSP framework |
| `tree-sitter` | Parser runtime |
| `tree-sitter-html` | HTML grammar |
| `tree-sitter-typescript` | TSX grammar |
| `tree-sitter-javascript` | JSX grammar |
| `tree-sitter-vue` | Vue grammar |
| `tree-sitter-svelte` | Svelte grammar |
| `toml` | Config parsing |
| `serde` / `serde_derive` | Serialization |
| `tokio` | Async runtime |

## Technology Decisions

| Decision | Rationale |
|---|---|
| Rust over Go/TypeScript | Best performance for real-time LSP diagnostics, native tree-sitter integration |
| Tree-sitter over manual parsing | Incremental parsing, error recovery, multi-format support via grammars |
| tower-lsp-server (community fork) | Most popular Rust LSP framework, actively maintained fork |
| Own rule engine over wrapping axe-core | Static checks don't need browser DOM, Rust gives us microsecond-scale audits |
| TOML config over JSON/YAML | Rust ecosystem standard, simple and readable |
