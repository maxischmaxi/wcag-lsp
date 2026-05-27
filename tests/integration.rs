use tower_lsp_server::ls_types::NumberOrString;
use wcag_lsp::config::Config;
use wcag_lsp::document::DocumentManager;
use wcag_lsp::engine;
use wcag_lsp::rules;

#[test]
fn test_full_html_analysis() {
    let mut mgr = DocumentManager::new();
    let html = r#"<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body>
  <img src="photo.jpg">
  <a href="/"></a>
  <iframe src="/embed"></iframe>
</body>
</html>"#;

    let doc = mgr
        .open("file:///test.html".to_string(), html.to_string(), 1)
        .unwrap();
    let rules = rules::all_rules();
    let config = Config::default();
    let diagnostics = engine::run_diagnostics(doc, &rules, &config);

    let codes: Vec<String> = diagnostics
        .iter()
        .filter_map(|d| d.code.as_ref())
        .map(|c| match c {
            NumberOrString::String(s) => s.clone(),
            NumberOrString::Number(n) => n.to_string(),
        })
        .collect();

    // Should find: img-alt, html-lang, anchor-content, iframe-title
    assert!(
        codes.contains(&"img-alt".to_string()),
        "Missing img-alt, found: {:?}",
        codes
    );
    assert!(
        codes.contains(&"html-lang".to_string()),
        "Missing html-lang, found: {:?}",
        codes
    );
    assert!(
        codes.contains(&"anchor-content".to_string()),
        "Missing anchor-content, found: {:?}",
        codes
    );
    assert!(
        codes.contains(&"iframe-title".to_string()),
        "Missing iframe-title, found: {:?}",
        codes
    );
}

#[test]
fn test_tsx_analysis() {
    let mut mgr = DocumentManager::new();
    let tsx = r#"const App = () => (
  <div>
    <img src="photo.jpg" />
    <a href="/"></a>
  </div>
);"#;

    let doc = mgr
        .open("file:///App.tsx".to_string(), tsx.to_string(), 1)
        .unwrap();
    let rules = rules::all_rules();
    let config = Config::default();
    let diagnostics = engine::run_diagnostics(doc, &rules, &config);

    assert!(
        !diagnostics.is_empty(),
        "Expected at least 1 diagnostic for TSX"
    );

    let codes: Vec<String> = diagnostics
        .iter()
        .filter_map(|d| d.code.as_ref())
        .map(|c| match c {
            NumberOrString::String(s) => s.clone(),
            NumberOrString::Number(n) => n.to_string(),
        })
        .collect();

    assert!(
        codes.contains(&"img-alt".to_string()),
        "Missing img-alt for TSX"
    );
}

#[test]
fn test_vue_analysis() {
    let mut mgr = DocumentManager::new();
    // Mirrors the reported issue: bound `:alt` must NOT trigger img-alt, while a
    // genuinely missing alt still does. Also exercises a Vue listbox/option
    // composite widget with `@click` and a bound `:role`.
    let vue = r#"<template>
  <div>
    <img :alt="alt" src="a.jpg" />
    <img v-bind:alt="alt2" src="b.jpg">
    <img src="noalt.jpg" />
    <button :aria-label="label" @click="go" />
    <ul role="listbox">
      <li
        v-for="(o, i) in items"
        :key="o.id"
        role="option"
        :aria-selected="i === active"
        tabindex="-1"
        @click="select(o)"
      >{{ o.label }}</li>
    </ul>
    <div :role="dynamicRole" aria-checked="maybe" />
  </div>
</template>"#;

    let doc = mgr
        .open("file:///Comp.vue".to_string(), vue.to_string(), 1)
        .unwrap();
    let rules = rules::all_rules();
    let config = Config::default();
    let diagnostics = engine::run_diagnostics(doc, &rules, &config);

    let codes: Vec<String> = diagnostics
        .iter()
        .filter_map(|d| d.code.as_ref())
        .map(|c| match c {
            NumberOrString::String(s) => s.clone(),
            NumberOrString::Number(n) => n.to_string(),
        })
        .collect();

    // The genuine violations must be reported...
    assert!(
        codes.contains(&"img-alt".to_string()),
        "Missing img-alt for the alt-less <img>, found: {codes:?}"
    );
    assert!(
        codes.contains(&"aria-valid-attr-value".to_string()),
        "Missing aria-valid-attr-value for static aria-checked=\"maybe\", found: {codes:?}"
    );
    // ...but exactly one img-alt (the two bound :alt images must NOT be flagged).
    assert_eq!(
        codes.iter().filter(|c| *c == "img-alt").count(),
        1,
        "bound :alt / v-bind:alt must not be flagged, found: {codes:?}"
    );
    // page-title is a document-level rule and must not fire on an SFC fragment.
    assert!(
        !codes.contains(&"page-title".to_string()),
        "page-title must not fire on a Vue SFC fragment, found: {codes:?}"
    );
    // Composite-widget option must not trigger these in Vue.
    assert!(
        !codes.contains(&"aria-required-children".to_string()),
        "listbox has option children, found: {codes:?}"
    );
    assert!(
        !codes.contains(&"nested-interactive".to_string()),
        "option in listbox is a valid composite widget, found: {codes:?}"
    );
}

#[test]
fn test_config_disables_rule() {
    let mut mgr = DocumentManager::new();
    let html = r#"<html><body><img src="photo.jpg"></body></html>"#;

    let doc = mgr
        .open("file:///test.html".to_string(), html.to_string(), 1)
        .unwrap();
    let rules = rules::all_rules();
    let config = Config::parse(
        r#"
[rules]
img-alt = "off"
"#,
    );
    let diagnostics = engine::run_diagnostics(doc, &rules, &config);

    let codes: Vec<String> = diagnostics
        .iter()
        .filter_map(|d| d.code.as_ref())
        .map(|c| match c {
            NumberOrString::String(s) => s.clone(),
            _ => String::new(),
        })
        .collect();

    assert!(
        !codes.contains(&"img-alt".to_string()),
        "img-alt should be disabled"
    );
    // html-lang should still fire
    assert!(
        codes.contains(&"html-lang".to_string()),
        "html-lang should still be active"
    );
}

#[test]
fn test_config_severity_override() {
    let mut mgr = DocumentManager::new();
    let html = r#"<html><body><img src="photo.jpg"></body></html>"#;

    let doc = mgr
        .open("file:///test.html".to_string(), html.to_string(), 1)
        .unwrap();
    let rules = rules::all_rules();
    let config = Config::parse(
        r#"
[rules]
img-alt = "warning"
"#,
    );
    let diagnostics = engine::run_diagnostics(doc, &rules, &config);

    let img_alt_diag = diagnostics
        .iter()
        .find(|d| d.code == Some(NumberOrString::String("img-alt".to_string())));

    assert!(img_alt_diag.is_some(), "img-alt diagnostic should exist");
    assert_eq!(
        img_alt_diag.unwrap().severity,
        Some(tower_lsp_server::ls_types::DiagnosticSeverity::WARNING),
        "img-alt should be WARNING severity"
    );
}

#[test]
fn test_clean_html_no_diagnostics() {
    let mut mgr = DocumentManager::new();
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head><title>Clean Page</title></head>
<body>
  <h1>Welcome</h1>
  <img src="photo.jpg" alt="A beautiful sunset">
  <a href="/">Home</a>
</body>
</html>"#;

    let doc = mgr
        .open("file:///clean.html".to_string(), html.to_string(), 1)
        .unwrap();
    let rules = rules::all_rules();
    let config = Config::default();
    let diagnostics = engine::run_diagnostics(doc, &rules, &config);

    assert_eq!(
        diagnostics.len(),
        0,
        "Clean HTML should have no diagnostics, found: {:?}",
        diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

#[test]
fn test_inline_disable_file_wide_suppresses_all_diagnostics() {
    let mut mgr = DocumentManager::new();
    let html = r#"<!-- wcag-disable -->
<html>
<body>
  <img src="photo.jpg">
  <a href="/"></a>
</body>
</html>"#;

    let doc = mgr
        .open("file:///disabled.html".to_string(), html.to_string(), 1)
        .unwrap();
    let rules = rules::all_rules();
    let config = Config::default();
    let diagnostics = engine::run_diagnostics(doc, &rules, &config);

    assert!(
        diagnostics.is_empty(),
        "file-wide disable should suppress all diagnostics"
    );
}

#[test]
fn test_inline_disable_file_wide_can_target_level_and_rule() {
    let mut mgr = DocumentManager::new();
    let html = r#"<!-- wcag-disable AA img-alt -->
<html>
<body>
  <h2></h2>
  <img src="photo.jpg">
  <a href="/"></a>
</body>
</html>"#;

    let doc = mgr
        .open(
            "file:///partially-disabled.html".to_string(),
            html.to_string(),
            1,
        )
        .unwrap();
    let rules = rules::all_rules();
    let config = Config::default();
    let diagnostics = engine::run_diagnostics(doc, &rules, &config);

    let codes: Vec<String> = diagnostics
        .iter()
        .filter_map(|d| d.code.as_ref())
        .map(|c| match c {
            NumberOrString::String(s) => s.clone(),
            NumberOrString::Number(n) => n.to_string(),
        })
        .collect();

    assert!(!codes.contains(&"img-alt".to_string()));
    assert!(!codes.contains(&"heading-content".to_string()));
    assert!(codes.contains(&"anchor-content".to_string()));
    assert!(codes.contains(&"html-lang".to_string()));
}

#[test]
fn test_inline_disable_next_line_suppresses_only_targeted_rule() {
    let mut mgr = DocumentManager::new();
    let html = r#"<html>
<body>
  <!-- wcag-disable-next-line img-alt -->
  <img src="photo.jpg">
  <img src="photo2.jpg">
</body>
</html>"#;

    let doc = mgr
        .open("file:///next-line.html".to_string(), html.to_string(), 1)
        .unwrap();
    let rules = rules::all_rules();
    let config = Config::default();
    let diagnostics = engine::run_diagnostics(doc, &rules, &config);

    let img_alt_diags: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code == Some(NumberOrString::String("img-alt".to_string())))
        .collect();

    assert_eq!(
        img_alt_diags.len(),
        1,
        "only one img-alt diagnostic should remain"
    );
    assert_eq!(img_alt_diags[0].range.start.line, 4);
}

#[test]
fn test_inline_disable_line_suppresses_only_current_line() {
    let mut mgr = DocumentManager::new();
    let html = r#"<html>
<body>
  <!-- wcag-disable-line img-alt --><img src="photo.jpg">
  <img src="photo2.jpg">
</body>
</html>"#;

    let doc = mgr
        .open("file:///disable-line.html".to_string(), html.to_string(), 1)
        .unwrap();
    let rules = rules::all_rules();
    let config = Config::default();
    let diagnostics = engine::run_diagnostics(doc, &rules, &config);

    let img_alt_diags: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code == Some(NumberOrString::String("img-alt".to_string())))
        .collect();

    assert_eq!(
        img_alt_diags.len(),
        1,
        "only one img-alt diagnostic should remain"
    );
    assert_eq!(img_alt_diags[0].range.start.line, 3);
}
