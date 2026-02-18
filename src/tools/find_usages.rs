use std::path::Path;

use crate::indexer::{SymbolIndex, SymbolOccurrence};

/// Find all usages (references) of a symbol in the index.
/// If `file` and `line` are provided, first find the symbol at that location
/// to get its FQN for precise matching.
pub fn find_usages<'a>(
    index: &'a SymbolIndex,
    symbol: &str,
    file: Option<&Path>,
    line: Option<usize>,
) -> Vec<&'a SymbolOccurrence> {
    // If file and line are provided, try to find the exact symbol first
    let fqn = if let (Some(f), Some(l)) = (file, line) {
        find_symbol_fqn_at(index, f, l, symbol)
    } else {
        // Try to find by FQN if the symbol looks fully qualified
        if symbol.contains('.') {
            Some(symbol.to_string())
        } else {
            find_unique_fqn(index, symbol)
        }
    };

    if let Some(ref fqn) = fqn {
        // Precise FQN-based lookup
        let mut results: Vec<&SymbolOccurrence> = Vec::new();
        if let Some(occs) = index.by_fqn.get(fqn) {
            for occ in occs {
                if occ.kind.is_reference() || matches!(occ.kind, crate::indexer::SymbolKind::Import) {
                    results.push(occ);
                }
            }
        }
        // Also check type aliases that point to this FQN
        for (alias_fqn, target_fqn) in &index.type_aliases {
            if target_fqn == fqn {
                if let Some(occs) = index.by_fqn.get(alias_fqn) {
                    for occ in occs {
                        if occ.kind.is_reference() {
                            results.push(occ);
                        }
                    }
                }
            }
        }
        if !results.is_empty() {
            results.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
            return results;
        }
    }

    // Fall back to name-based lookup
    let mut results: Vec<&SymbolOccurrence> = Vec::new();
    if let Some(occs) = index.by_name.get(symbol) {
        for occ in occs {
            if occ.kind.is_reference() || matches!(occ.kind, crate::indexer::SymbolKind::Import) {
                results.push(occ);
            }
        }
    }
    results.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    results
}

/// Find the FQN of a symbol at a specific file and line.
fn find_symbol_fqn_at(
    index: &SymbolIndex,
    file: &Path,
    line: usize,
    name: &str,
) -> Option<String> {
    if let Some(occs) = index.by_name.get(name) {
        for occ in occs {
            if occ.file == file && occ.line == line {
                return occ.fqn.clone();
            }
        }
    }
    None
}

/// If a symbol name maps to exactly one FQN, return it.
fn find_unique_fqn(index: &SymbolIndex, name: &str) -> Option<String> {
    if let Some(occs) = index.by_name.get(name) {
        let fqns: std::collections::HashSet<&str> = occs
            .iter()
            .filter(|o| o.kind.is_declaration())
            .filter_map(|o| o.fqn.as_deref())
            .collect();
        if fqns.len() == 1 {
            return fqns.into_iter().next().map(|s| s.to_string());
        }
    }
    None
}
