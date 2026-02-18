use std::path::Path;

use crate::indexer::{SymbolIndex, SymbolOccurrence};

/// Find the definition(s) of a symbol.
/// Returns only declaration-kind occurrences.
pub fn find_definition<'a>(
    index: &'a SymbolIndex,
    symbol: &str,
    file: Option<&Path>,
    line: Option<usize>,
) -> Vec<&'a SymbolOccurrence> {
    // If file and line are provided, try to resolve the exact FQN at that location
    let fqn = if let (Some(f), Some(l)) = (file, line) {
        find_reference_fqn_at(index, f, l, symbol)
    } else if symbol.contains('.') {
        Some(symbol.to_string())
    } else {
        None
    };

    if let Some(ref fqn) = fqn {
        // Precise FQN-based lookup
        let mut results: Vec<&SymbolOccurrence> = Vec::new();
        if let Some(occs) = index.by_fqn.get(fqn) {
            for occ in occs {
                if occ.kind.is_declaration() {
                    results.push(occ);
                }
            }
        }
        // Check type aliases
        if results.is_empty() {
            if let Some(target_fqn) = index.type_aliases.get(fqn) {
                if let Some(occs) = index.by_fqn.get(target_fqn) {
                    for occ in occs {
                        if occ.kind.is_declaration() {
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
            if occ.kind.is_declaration() {
                results.push(occ);
            }
        }
    }
    results.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    results
}

/// Find the FQN of a reference at a specific file and line.
fn find_reference_fqn_at(
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
