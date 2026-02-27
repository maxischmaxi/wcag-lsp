use crate::document::Document;
use crate::rules::Rule;
use tower_lsp_server::ls_types::*;

pub fn run_diagnostics(doc: &Document, rules: &[Box<dyn Rule>]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for rule in rules {
        let rule_diags = rule.check(&doc.tree.root_node(), &doc.source, doc.file_type);
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
