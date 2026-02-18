use super::{SymbolIndex, SymbolOccurrence};

/// Kotlin implicit imports that are available in every file.
pub const KOTLIN_IMPLICIT_IMPORTS: &[&str] = &[
    "kotlin",
    "kotlin.annotation",
    "kotlin.collections",
    "kotlin.comparisons",
    "kotlin.io",
    "kotlin.ranges",
    "kotlin.sequences",
    "kotlin.text",
];

/// After initial indexing, cross-reference symbols:
/// For each reference that only has a by-name entry, try to resolve its FQN
/// using the full index.
pub fn cross_reference(index: &mut SymbolIndex) {
    // Collect all declarations by their simple name for resolution
    let declarations_by_name: std::collections::HashMap<String, Vec<(String, std::path::PathBuf)>> = {
        let mut map: std::collections::HashMap<String, Vec<(String, std::path::PathBuf)>> =
            std::collections::HashMap::new();
        for (name, occs) in &index.by_name {
            for occ in occs {
                if occ.kind.is_declaration() {
                    if let Some(ref fqn) = occ.fqn {
                        map.entry(name.clone())
                            .or_default()
                            .push((fqn.clone(), occ.file.clone()));
                    }
                }
            }
        }
        map
    };

    // Collect file info for import resolution
    let files = index.files.clone();
    let type_aliases = index.type_aliases.clone();

    // Resolve references that need better FQN resolution
    let mut updates: Vec<(String, usize, String)> = Vec::new(); // (name, index_in_vec, new_fqn)

    for (name, occs) in &index.by_name {
        for (idx, occ) in occs.iter().enumerate() {
            if !occ.kind.is_reference() {
                continue;
            }

            // Try to resolve to a better FQN
            if let Some(file_info) = files.get(&occ.file) {
                if let Some(resolved_fqn) =
                    resolve_symbol_fqn(name, file_info, &declarations_by_name, &type_aliases)
                {
                    if occ.fqn.as_deref() != Some(&resolved_fqn) {
                        // Don't override a FQN that already resolves to a known declaration.
                        // This prevents same-file class methods from shadowing a correct
                        // top-level function FQN that was assigned during initial parsing.
                        let current_is_valid = occ.fqn.as_ref().is_some_and(|current_fqn| {
                            declarations_by_name.get(name).is_some_and(|decls| {
                                decls.iter().any(|(fqn, _)| fqn == current_fqn)
                            })
                        });
                        if !current_is_valid {
                            updates.push((name.clone(), idx, resolved_fqn));
                        }
                    }
                }
            }
        }
    }

    // Apply updates
    for (name, idx, new_fqn) in updates {
        if let Some(occs) = index.by_name.get_mut(&name) {
            if let Some(occ) = occs.get_mut(idx) {
                // Remove from old FQN index
                if let Some(ref old_fqn) = occ.fqn {
                    if let Some(fqn_occs) = index.by_fqn.get_mut(old_fqn) {
                        fqn_occs.retain(|o| {
                            !(o.file == occ.file && o.byte_range == occ.byte_range)
                        });
                    }
                }
                // Update FQN
                occ.fqn = Some(new_fqn.clone());
                // Add to new FQN index
                index
                    .by_fqn
                    .entry(new_fqn)
                    .or_default()
                    .push(occ.clone());
            }
        }
    }
}

/// Resolve a symbol name to its FQN using the import resolution order:
/// 1. Same-file declarations
/// 2. Explicit imports
/// 3. Alias imports
/// 4. Wildcard imports (check if FQN exists in declarations)
/// 5. Same-package declarations
/// 6. Kotlin implicit imports
fn resolve_symbol_fqn(
    name: &str,
    file_info: &super::FileInfo,
    declarations_by_name: &std::collections::HashMap<String, Vec<(String, std::path::PathBuf)>>,
    type_aliases: &std::collections::HashMap<String, String>,
) -> Option<String> {
    // 1. Explicit imports
    for imp in &file_info.imports {
        if imp.is_wildcard {
            continue;
        }
        let imported_name = if let Some(ref alias) = imp.alias {
            alias.as_str()
        } else {
            imp.path.rsplit('.').next().unwrap_or(&imp.path)
        };
        if imported_name == name {
            let fqn = imp.path.clone();
            // Follow type alias chain
            return Some(follow_type_alias(&fqn, type_aliases));
        }
    }

    // 2. Same-file declarations
    if let Some(decls) = declarations_by_name.get(name) {
        for (fqn, decl_file) in decls {
            if decl_file == &file_info.path {
                return Some(fqn.clone());
            }
        }
    }

    // 3. Wildcard imports
    for imp in &file_info.imports {
        if !imp.is_wildcard {
            continue;
        }
        let candidate_fqn = format!("{}.{}", imp.path, name);
        // Check if this FQN exists in declarations
        if let Some(decls) = declarations_by_name.get(name) {
            for (fqn, _) in decls {
                if *fqn == candidate_fqn {
                    return Some(follow_type_alias(&candidate_fqn, type_aliases));
                }
            }
        }
    }

    // 4. Same-package declarations
    if let Some(ref pkg) = file_info.package {
        let candidate_fqn = format!("{}.{}", pkg, name);
        if let Some(decls) = declarations_by_name.get(name) {
            for (fqn, _) in decls {
                if *fqn == candidate_fqn {
                    return Some(candidate_fqn);
                }
            }
        }
    }

    // 5. Kotlin implicit imports
    if let Some(decls) = declarations_by_name.get(name) {
        for (fqn, _) in decls {
            for prefix in KOTLIN_IMPLICIT_IMPORTS {
                if fqn.starts_with(prefix) && fqn == &format!("{}.{}", prefix, name) {
                    return Some(fqn.clone());
                }
            }
        }
    }

    None
}

fn follow_type_alias(fqn: &str, type_aliases: &std::collections::HashMap<String, String>) -> String {
    let mut current = fqn.to_string();
    let mut seen = std::collections::HashSet::new();
    while let Some(target) = type_aliases.get(&current) {
        if !seen.insert(current.clone()) {
            break; // Prevent cycles
        }
        current = target.clone();
    }
    current
}

/// Register companion object members under both `MyClass.Companion.member` and `MyClass.member`.
pub fn register_companion_aliases(index: &mut SymbolIndex) {
    let mut new_entries: Vec<SymbolOccurrence> = Vec::new();

    for occs in index.by_fqn.values() {
        for occ in occs {
            if let Some(ref fqn) = occ.fqn {
                // Check if this is inside a Companion object
                if fqn.contains(".Companion.") {
                    // Create an alias without .Companion.
                    let alias_fqn = fqn.replace(".Companion.", ".");
                    let mut alias_occ = occ.clone();
                    alias_occ.fqn = Some(alias_fqn);
                    new_entries.push(alias_occ);
                }
            }
        }
    }

    for occ in new_entries {
        if let Some(ref fqn) = occ.fqn {
            index.by_fqn.entry(fqn.clone()).or_default().push(occ);
        }
    }
}
