pub mod dependency_tree;
pub mod find_definition;
pub mod find_usages;

use crate::indexer::SymbolOccurrence;
use std::path::Path;

/// Format a list of symbol occurrences into a human-readable string.
pub fn format_occurrences(occurrences: &[&SymbolOccurrence], project_root: &Path) -> String {
    if occurrences.is_empty() {
        return "No results found.".to_string();
    }

    let mut lines = Vec::new();
    lines.push(format!("Found {} result(s):\n", occurrences.len()));

    for occ in occurrences {
        let rel_path = occ
            .file
            .strip_prefix(project_root)
            .unwrap_or(&occ.file)
            .display();
        let kind = format!("{:?}", occ.kind);
        let fqn_display = occ
            .fqn
            .as_deref()
            .map(|f| format!(" [{}]", f))
            .unwrap_or_default();
        let receiver_display = occ
            .receiver_type
            .as_deref()
            .map(|r| format!(" (receiver: {})", r))
            .unwrap_or_default();

        lines.push(format!(
            "  {}:{}:{} - {} `{}`{}{}",
            rel_path,
            occ.line,
            occ.column,
            kind,
            occ.name,
            fqn_display,
            receiver_display,
        ));
    }

    lines.join("\n")
}
