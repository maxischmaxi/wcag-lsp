use crate::config::Config;
use crate::document::Document;
use crate::inline_directives::InlineDirectives;
use crate::rules::{Rule, Severity};
use tower_lsp_server::ls_types::*;

pub fn run_diagnostics(
    doc: &Document,
    rules: &[Box<dyn Rule>],
    config: &Config,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let directives = InlineDirectives::parse(&doc.source);

    for rule in rules {
        let meta = rule.metadata();

        if directives.disables_file_rule(meta.id, meta.wcag_level) {
            continue;
        }

        let severity = match config.effective_severity(meta.id, meta.wcag_level) {
            Some(s) => s,
            None => continue,
        };
        let lsp_severity = match severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
        };

        let mut rule_diags = rule.check(&doc.tree.root_node(), &doc.source, doc.file_type);
        rule_diags.retain(|diag| {
            !directives.disables_line_rule(diag.range.start.line, meta.id, meta.wcag_level)
        });

        for diag in &mut rule_diags {
            diag.severity = Some(lsp_severity);
        }
        diagnostics.extend(rule_diags);
    }
    diagnostics
}

pub fn node_to_range(node: &tree_sitter::Node) -> Range {
    let start = node.start_position();
    let end = node.end_position();
    Range {
        start: Position {
            line: start.row as u32,
            character: start.column as u32,
        },
        end: Position {
            line: end.row as u32,
            character: end.column as u32,
        },
    }
}
