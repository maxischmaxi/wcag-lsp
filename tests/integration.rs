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
fn test_config_disables_rule() {
    let mut mgr = DocumentManager::new();
    let html = r#"<html><body><img src="photo.jpg"></body></html>"#;

    let doc = mgr
        .open("file:///test.html".to_string(), html.to_string(), 1)
        .unwrap();
    let rules = rules::all_rules();
    let config = Config::from_str(
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
    let config = Config::from_str(
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
