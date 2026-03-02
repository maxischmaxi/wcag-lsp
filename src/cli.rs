use std::collections::BTreeMap;

use tower_lsp_server::ls_types::DiagnosticSeverity;

use crate::config::Config;
use crate::document::Document;
use crate::parser::{self, FileType};
use crate::rules::{self, Rule};

struct FileDiagnostic {
    line: u32,
    col: u32,
    severity: &'static str,
    message: String,
    rule_id: String,
}

pub fn run_check(patterns: &[String]) -> i32 {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let config = Config::from_dir(&cwd);
    let rules = rules::all_rules();

    let mut all_files: Vec<std::path::PathBuf> = Vec::new();
    for pattern in patterns {
        match glob::glob(pattern) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    if !all_files.contains(&entry) {
                        all_files.push(entry);
                    }
                }
            }
            Err(e) => {
                eprintln!("Invalid pattern '{}': {}", pattern, e);
            }
        }
    }

    // Apply ignore patterns
    all_files.retain(|path| {
        let path_str = path.to_string_lossy();
        !config
            .ignore_patterns
            .iter()
            .any(|pat| glob_match::glob_match(pat, &path_str))
    });

    let mut results: BTreeMap<String, Vec<FileDiagnostic>> = BTreeMap::new();
    let mut total_errors: usize = 0;
    let mut total_warnings: usize = 0;

    for path in &all_files {
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e,
            None => continue,
        };

        let file_type = FileType::from_extension(ext);
        if file_type == FileType::Unknown {
            continue;
        }

        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Could not read {}: {}", path.display(), e);
                continue;
            }
        };

        let diagnostics = lint_source(&source, file_type, &rules, &config);
        if diagnostics.is_empty() {
            continue;
        }

        let path_str = path.to_string_lossy().to_string();
        for diag in diagnostics {
            if diag.severity == "error" {
                total_errors += 1;
            } else {
                total_warnings += 1;
            }
            results.entry(path_str.clone()).or_default().push(diag);
        }
    }

    print_results(&results, total_errors, total_warnings);

    if total_errors > 0 { 1 } else { 0 }
}

fn lint_source(
    source: &str,
    file_type: FileType,
    rules: &[Box<dyn Rule>],
    config: &Config,
) -> Vec<FileDiagnostic> {
    let mut parser = match parser::create_parser(file_type) {
        Some(p) => p,
        None => return vec![],
    };

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };

    let doc = Document {
        uri: String::new(),
        file_type,
        source: source.to_string(),
        tree,
        version: 0,
    };

    let diagnostics = crate::engine::run_diagnostics(&doc, rules, config);

    diagnostics
        .into_iter()
        .map(|d| {
            let severity = match d.severity {
                Some(DiagnosticSeverity::ERROR) => "error",
                _ => "warning",
            };
            let rule_id = match &d.code {
                Some(tower_lsp_server::ls_types::NumberOrString::String(s)) => s.clone(),
                _ => String::new(),
            };
            FileDiagnostic {
                line: d.range.start.line + 1,
                col: d.range.start.character + 1,
                severity,
                message: d.message,
                rule_id,
            }
        })
        .collect()
}

fn print_results(
    results: &BTreeMap<String, Vec<FileDiagnostic>>,
    total_errors: usize,
    total_warnings: usize,
) {
    if results.is_empty() {
        return;
    }

    for (path, diags) in results {
        eprintln!("\n{}", path);
        for d in diags {
            eprintln!(
                "  {}:{}  {}  {}  {}",
                d.line, d.col, d.severity, d.message, d.rule_id
            );
        }
    }

    let total = total_errors + total_warnings;
    eprintln!(
        "\n\u{2716} {} {} ({} {}, {} {})",
        total,
        if total == 1 { "problem" } else { "problems" },
        total_errors,
        if total_errors == 1 { "error" } else { "errors" },
        total_warnings,
        if total_warnings == 1 {
            "warning"
        } else {
            "warnings"
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_with_violations_returns_exit_1() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("bad.html");
        std::fs::write(&file_path, r#"<img src="photo.jpg">"#).unwrap();

        let pattern = dir.path().join("*.html").to_string_lossy().to_string();
        let saved_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let code = run_check(&[pattern]);
        std::env::set_current_dir(saved_dir).unwrap();

        assert_eq!(code, 1);
    }

    #[test]
    fn test_clean_file_returns_exit_0() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("good.html");
        std::fs::write(
            &file_path,
            r#"<html lang="en"><head><title>Test</title></head><body><img src="x.jpg" alt="A cat"></body></html>"#,
        )
        .unwrap();

        let pattern = dir.path().join("*.html").to_string_lossy().to_string();
        let saved_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let code = run_check(&[pattern]);
        std::env::set_current_dir(saved_dir).unwrap();

        assert_eq!(code, 0);
    }

    #[test]
    fn test_no_matching_files_returns_exit_0() {
        let dir = tempfile::tempdir().unwrap();

        let pattern = dir.path().join("*.html").to_string_lossy().to_string();
        let saved_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let code = run_check(&[pattern]);
        std::env::set_current_dir(saved_dir).unwrap();

        assert_eq!(code, 0);
    }

    #[test]
    fn test_lint_source_detects_img_without_alt() {
        let config = Config::default();
        let rules = rules::all_rules();
        let diags = lint_source(r#"<img src="photo.jpg">"#, FileType::Html, &rules, &config);
        assert!(!diags.is_empty());
        assert!(diags.iter().any(|d| d.rule_id == "img-alt"));
        assert!(diags.iter().any(|d| d.severity == "error"));
    }

    #[test]
    fn test_lint_source_clean_file() {
        let config = Config::default();
        let rules = rules::all_rules();
        let diags = lint_source(
            r#"<img src="photo.jpg" alt="A photo">"#,
            FileType::Html,
            &rules,
            &config,
        );
        let img_alt_diags: Vec<_> = diags.iter().filter(|d| d.rule_id == "img-alt").collect();
        assert!(img_alt_diags.is_empty());
    }
}
