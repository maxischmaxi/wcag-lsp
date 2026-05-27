//! Vue-aware attribute helpers for the HTML tree-sitter grammar.
//!
//! HTML, Vue and Svelte files are all parsed with `tree-sitter-html`. The rules
//! that operate on that grammar need a consistent way to read attributes that
//! also understands Vue's directive syntax and self-closing tags. This module
//! provides a single normalized view so individual rules don't each have to
//! re-implement the tree walking (and forget about Vue or `<img/>`).
//!
//! Normalization performed on the attribute name:
//!   - `:alt` / `v-bind:alt`   → name `alt`,    `bound = true`
//!   - `@click` / `v-on:click` → name `click`,  `bound = true`, `event = true`
//!   - `v-html`, `v-if`, …     → name kept as-is (`v-html`), `directive = true`
//!   - modifiers are stripped: `@click.prevent` → `click`, `:foo.sync` → `foo`
//!
//! Plain HTML attributes pass through unchanged, so this is safe to use for
//! every HTML-grammar file type.

use tree_sitter::Node;

/// A normalized attribute on an element parsed with the HTML grammar.
#[derive(Debug, Clone)]
pub struct Attr<'a> {
    /// The underlying `attribute` node (useful for diagnostic ranges).
    pub node: Node<'a>,
    /// Normalized name with any Vue prefix/modifier stripped (original case).
    pub name: String,
    /// The literal value inside the quotes, if present. For a bound attribute
    /// this is the expression text (e.g. `"alt"` in `:alt="alt"`), not a value
    /// that should be validated literally.
    pub value: Option<String>,
    /// `true` when the value is a dynamic expression (`:x`, `v-bind:x`, `@x`,
    /// `v-on:x`). The literal `value` text is then a JS expression.
    pub bound: bool,
    /// `true` for event bindings (`@x` / `v-on:x`).
    pub event: bool,
}

impl Attr<'_> {
    /// Case-insensitive comparison against the normalized name.
    pub fn name_eq(&self, other: &str) -> bool {
        self.name.eq_ignore_ascii_case(other)
    }

    /// The normalized name, lowercased.
    pub fn name_lower(&self) -> String {
        self.name.to_ascii_lowercase()
    }
}

/// Split a raw attribute name into `(normalized_name, bound, event)`.
pub fn normalize_attr_name(raw: &str) -> (String, bool, bool) {
    let (base, bound, event) = if let Some(rest) = raw.strip_prefix('@') {
        (rest, true, true)
    } else if let Some(rest) = raw.strip_prefix("v-on:") {
        (rest, true, true)
    } else if let Some(rest) = raw.strip_prefix(':') {
        (rest, true, false)
    } else if let Some(rest) = raw.strip_prefix("v-bind:") {
        (rest, true, false)
    } else {
        // Plain attribute or a `v-*` directive (kept as-is).
        (raw, false, false)
    };

    // Strip Vue modifiers (`.prevent`, `.enter`, `.camel`, …). HTML/ARIA
    // attribute names never contain a dot, so this is safe.
    let name = base.split('.').next().unwrap_or(base).to_string();
    (name, bound, event)
}

/// Build an [`Attr`] from an `attribute` node, or `None` if it isn't one.
pub fn attr_from_node<'a>(node: &Node<'a>, source: &str) -> Option<Attr<'a>> {
    if node.kind() != "attribute" {
        return None;
    }

    let mut raw_name: Option<&str> = None;
    let mut value: Option<String> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "attribute_name" => raw_name = Some(&source[child.byte_range()]),
            "quoted_attribute_value" => {
                let mut vc = child.walk();
                for v in child.children(&mut vc) {
                    if v.kind() == "attribute_value" {
                        value = Some(source[v.byte_range()].to_string());
                    }
                }
            }
            // Unquoted attribute value, e.g. `tabindex=0`.
            "attribute_value" => value = Some(source[child.byte_range()].to_string()),
            _ => {}
        }
    }

    let raw_name = raw_name?;
    let (name, bound, event) = normalize_attr_name(raw_name);
    Some(Attr {
        node: *node,
        name,
        value,
        bound,
        event,
    })
}

/// The tag node of an `element` — its `start_tag` or `self_closing_tag` child.
/// Returns `None` for nodes that aren't element wrappers.
pub fn element_tag<'a>(element: &Node<'a>) -> Option<Node<'a>> {
    if element.kind() == "start_tag" || element.kind() == "self_closing_tag" {
        return Some(*element);
    }
    let mut cursor = element.walk();
    element
        .children(&mut cursor)
        .find(|c| c.kind() == "start_tag" || c.kind() == "self_closing_tag")
}

/// The tag name from a `start_tag`/`self_closing_tag` node.
pub fn tag_name<'a>(tag: &Node, source: &'a str) -> Option<&'a str> {
    let mut cursor = tag.walk();
    for child in tag.children(&mut cursor) {
        if child.kind() == "tag_name" {
            return Some(&source[child.byte_range()]);
        }
    }
    None
}

/// The tag name of an `element` node (resolves the inner tag first).
pub fn element_tag_name<'a>(element: &Node, source: &'a str) -> Option<&'a str> {
    let tag = element_tag(element)?;
    tag_name(&tag, source)
}

/// All normalized attributes on a `start_tag`/`self_closing_tag` node.
pub fn attrs<'a>(tag: &Node<'a>, source: &str) -> Vec<Attr<'a>> {
    let mut out = Vec::new();
    let mut cursor = tag.walk();
    for child in tag.children(&mut cursor) {
        if let Some(attr) = attr_from_node(&child, source) {
            out.push(attr);
        }
    }
    out
}

/// All normalized attributes on an `element` node (resolves its tag first).
pub fn element_attrs<'a>(element: &Node<'a>, source: &str) -> Vec<Attr<'a>> {
    match element_tag(element) {
        Some(tag) => attrs(&tag, source),
        None => Vec::new(),
    }
}

/// Find a single attribute by name (case-insensitive) on a tag node.
pub fn find_attr<'a>(tag: &Node<'a>, source: &str, name: &str) -> Option<Attr<'a>> {
    attrs(tag, source).into_iter().find(|a| a.name_eq(name))
}

/// Whether an element (or tag) has an attribute with the given name.
pub fn element_has_attr(element: &Node, source: &str, name: &str) -> bool {
    element_attrs(element, source).iter().any(|a| a.name_eq(name))
}

/// The (normalized) value of a named attribute on an element, if present.
/// Bound attributes return their expression text; callers that care should
/// check [`Attr::bound`] via [`element_attrs`] instead.
pub fn element_attr_value(element: &Node, source: &str, name: &str) -> Option<String> {
    element_attrs(element, source)
        .into_iter()
        .find(|a| a.name_eq(name))
        .and_then(|a| a.value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{self, FileType};

    fn parse_vue(src: &str) -> (tree_sitter::Tree, String) {
        let mut p = parser::create_parser(FileType::Vue).unwrap();
        let tree = p.parse(src, None).unwrap();
        (tree, src.to_string())
    }

    /// Find the first `element` node whose tag name matches `tag`.
    fn find_element_by_tag<'a>(
        n: tree_sitter::Node<'a>,
        source: &str,
        tag: &str,
    ) -> Option<tree_sitter::Node<'a>> {
        if n.kind() == "element" && element_tag_name(&n, source) == Some(tag) {
            return Some(n);
        }
        let mut cursor = n.walk();
        n.children(&mut cursor)
            .find_map(|c| find_element_by_tag(c, source, tag))
    }

    #[test]
    fn test_normalize_bind_shorthand() {
        assert_eq!(normalize_attr_name(":alt"), ("alt".to_string(), true, false));
    }

    #[test]
    fn test_normalize_bind_full() {
        assert_eq!(
            normalize_attr_name("v-bind:title"),
            ("title".to_string(), true, false)
        );
    }

    #[test]
    fn test_normalize_event_shorthand() {
        assert_eq!(
            normalize_attr_name("@click"),
            ("click".to_string(), true, true)
        );
    }

    #[test]
    fn test_normalize_event_full() {
        assert_eq!(
            normalize_attr_name("v-on:keydown"),
            ("keydown".to_string(), true, true)
        );
    }

    #[test]
    fn test_normalize_modifiers_stripped() {
        assert_eq!(
            normalize_attr_name("@click.prevent.stop"),
            ("click".to_string(), true, true)
        );
        assert_eq!(
            normalize_attr_name(":foo.sync"),
            ("foo".to_string(), true, false)
        );
    }

    #[test]
    fn test_normalize_plain_and_directive() {
        assert_eq!(normalize_attr_name("alt"), ("alt".to_string(), false, false));
        assert_eq!(
            normalize_attr_name("v-html"),
            ("v-html".to_string(), false, false)
        );
    }

    #[test]
    fn test_attrs_on_vue_img() {
        let (tree, src) = parse_vue(r#"<template><img :alt="alt" src="x"></template>"#);
        let img = find_element_by_tag(tree.root_node(), &src, "img").unwrap();
        let attrs = element_attrs(&img, &src);
        let alt = attrs.iter().find(|a| a.name_eq("alt")).unwrap();
        assert!(alt.bound);
        assert!(!alt.event);
        assert_eq!(alt.value.as_deref(), Some("alt"));
        assert!(element_has_attr(&img, &src, "alt"));
        assert_eq!(element_tag_name(&img, &src), Some("img"));
    }

    #[test]
    fn test_self_closing_tag_resolved() {
        let (tree, src) = parse_vue(r#"<template><input type="text" /></template>"#);
        let input = find_element_by_tag(tree.root_node(), &src, "input").unwrap();
        assert_eq!(element_tag_name(&input, &src), Some("input"));
        assert!(element_has_attr(&input, &src, "type"));
    }
}
