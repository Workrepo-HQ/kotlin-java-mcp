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
    include_imports: bool,
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
                if occ.kind.is_reference()
                    || (include_imports && matches!(occ.kind, crate::indexer::SymbolKind::Import))
                {
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
        // Also collect usages via Lombok accessor FQNs (getter/setter calls count as field usages)
        if let Some(accessor_fqns) = index.lombok_accessors.get(fqn) {
            // Extract the containing class FQN for import-based filtering.
            // e.g., "com.example.Foo.fieldName" → "com.example.Foo"
            let class_fqn = fqn.rsplit_once('.').map(|(prefix, _)| prefix);

            // Kotlin accesses Lombok fields using property syntax (obj.fieldName) rather than
            // getter/setter methods (obj.getFieldName()). Search by the field's simple name
            // to catch these property-style references, but only in files that import the
            // containing class (to avoid false positives from unrelated fields with the same name).
            let field_simple_name = fqn.rsplit('.').next().unwrap_or(fqn);
            if let Some(occs) = index.by_name.get(field_simple_name) {
                for occ in occs {
                    if occ.kind.is_reference()
                        || (include_imports
                            && matches!(occ.kind, crate::indexer::SymbolKind::Import))
                    {
                        if occ.fqn.as_deref() != Some(fqn)
                            && file_references_class(index, &occ.file, class_fqn)
                        {
                            results.push(occ);
                        }
                    }
                }
            }

            for acc_fqn in accessor_fqns {
                // First try FQN-based lookup
                if let Some(occs) = index.by_fqn.get(acc_fqn) {
                    for occ in occs {
                        if occ.kind.is_reference()
                            || (include_imports
                                && matches!(occ.kind, crate::indexer::SymbolKind::Import))
                        {
                            results.push(occ);
                        }
                    }
                }
                // Also check by simple name, filtering to files that import the containing class.
                let simple_name = acc_fqn.rsplit('.').next().unwrap_or(acc_fqn);
                if let Some(occs) = index.by_name.get(simple_name) {
                    for occ in occs {
                        if occ.kind.is_reference() {
                            let dominated_by_fqn = occ.fqn.as_deref() == Some(acc_fqn);
                            if !dominated_by_fqn
                                && file_references_class(index, &occ.file, class_fqn)
                            {
                                results.push(occ);
                            }
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
    // When the symbol is a FQN (contains '.'), by_name is keyed by simple names,
    // so extract the last component for the lookup.
    let lookup_name = if symbol.contains('.') {
        symbol.rsplit('.').next().unwrap_or(symbol)
    } else {
        symbol
    };
    let mut results: Vec<&SymbolOccurrence> = Vec::new();
    if let Some(occs) = index.by_name.get(lookup_name) {
        for occ in occs {
            if occ.kind.is_reference()
                || (include_imports && matches!(occ.kind, crate::indexer::SymbolKind::Import))
            {
                results.push(occ);
            }
        }
    }
    results.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    results
}

/// Check if a file could reference a given class: the file imports it explicitly,
/// has a wildcard import covering its package, or is in the same package.
fn file_references_class(index: &SymbolIndex, file: &Path, class_fqn: Option<&str>) -> bool {
    let class_fqn = match class_fqn {
        Some(fqn) => fqn,
        None => return true, // Can't determine class, don't filter
    };
    let file_info = match index.files.get(file) {
        Some(fi) => fi,
        None => return false,
    };

    // Check explicit imports
    for imp in &file_info.imports {
        if !imp.is_wildcard && imp.path == class_fqn {
            return true;
        }
        // Wildcard import covering the class's package
        if imp.is_wildcard {
            if let Some((class_pkg, _)) = class_fqn.rsplit_once('.') {
                if imp.path == class_pkg {
                    return true;
                }
            }
        }
    }

    // Same package as the class
    if let Some(ref pkg) = file_info.package {
        if let Some((class_pkg, _)) = class_fqn.rsplit_once('.') {
            if pkg == class_pkg {
                return true;
            }
        }
    }

    false
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
