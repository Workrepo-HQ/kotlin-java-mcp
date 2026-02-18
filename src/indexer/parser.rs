use std::path::{Path, PathBuf};

use rayon::prelude::*;
use tracing::{debug, warn};
use walkdir::WalkDir;

use super::scope::ScopeTree;
use super::{FileInfo, ImportInfo, SymbolIndex, SymbolKind, SymbolOccurrence};

/// Discover all .kt files under the given root, skipping build dirs and hidden dirs.
pub fn discover_kotlin_files(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // Skip hidden dirs, build dirs, gradle cache dirs
            if e.file_type().is_dir() {
                return !name.starts_with('.')
                    && name != "build"
                    && name != ".gradle"
                    && name != "node_modules";
            }
            true
        })
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().is_some_and(|ext| ext == "kt")
        })
        .map(|e| e.into_path())
        .collect()
}

/// Parse all discovered files in parallel and build a SymbolIndex.
pub fn index_files(root: &Path) -> SymbolIndex {
    let files = discover_kotlin_files(root);
    debug!("Discovered {} Kotlin files", files.len());

    let file_results: Vec<(FileInfo, Vec<SymbolOccurrence>, Vec<(String, String)>)> = files
        .par_iter()
        .filter_map(|path| {
            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    warn!("Failed to read {}: {}", path.display(), e);
                    return None;
                }
            };
            Some(parse_file(path, &source))
        })
        .collect();

    let mut index = SymbolIndex::new();
    for (file_info, occurrences, type_aliases) in file_results {
        index.add_file_info(file_info);
        for occ in occurrences {
            index.add_occurrence(occ);
        }
        for (alias_fqn, target_fqn) in type_aliases {
            index.type_aliases.insert(alias_fqn, target_fqn);
        }
    }

    debug!("{}", index.stats());
    index
}

/// Parse a single Kotlin file and extract symbols.
fn parse_file(
    path: &Path,
    source: &str,
) -> (FileInfo, Vec<SymbolOccurrence>, Vec<(String, String)>) {
    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter_kotlin_ng::LANGUAGE;
    parser
        .set_language(&language.into())
        .expect("Failed to set Kotlin language");

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => {
            warn!("Failed to parse {}", path.display());
            return (
                FileInfo {
                    path: path.to_path_buf(),
                    package: None,
                    imports: vec![],
                },
                vec![],
                vec![],
            );
        }
    };

    let root = tree.root_node();
    let src = source.as_bytes();

    // Extract package declaration
    let package = extract_package(&root, src);

    // Extract imports
    let imports = extract_imports(&root, src);

    // Build scope tree
    let scope_tree = build_scope_tree(&root, src);

    // Extract all symbols
    let mut occurrences = Vec::new();
    let mut type_aliases = Vec::new();

    extract_declarations(
        &root,
        src,
        path,
        package.as_deref(),
        &scope_tree,
        &mut occurrences,
        &mut type_aliases,
    );

    extract_references(&root, src, path, package.as_deref(), &scope_tree, &imports, &mut occurrences);

    // Add import occurrences
    for imp in &imports {
        let name = if let Some(ref alias) = imp.alias {
            alias.clone()
        } else if imp.is_wildcard {
            imp.path.clone()
        } else {
            imp.path.rsplit('.').next().unwrap_or(&imp.path).to_string()
        };
        occurrences.push(SymbolOccurrence {
            name,
            fqn: Some(imp.path.clone()),
            kind: SymbolKind::Import,
            file: path.to_path_buf(),
            line: imp.line,
            column: imp.column,
            byte_range: imp.byte_range.clone(),
            receiver_type: None,
        });
    }

    let file_info = FileInfo {
        path: path.to_path_buf(),
        package: package.clone(),
        imports,
    };

    (file_info, occurrences, type_aliases)
}

fn extract_package(root: &tree_sitter::Node, src: &[u8]) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "package_header" {
            let mut inner = child.walk();
            for c in child.children(&mut inner) {
                if c.kind() == "qualified_identifier" || c.kind() == "identifier" {
                    return Some(node_text(&c, src).to_string());
                }
            }
        }
    }
    None
}

fn extract_imports(root: &tree_sitter::Node, src: &[u8]) -> Vec<ImportInfo> {
    let mut imports = Vec::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        // In tree-sitter-kotlin-ng, imports are direct `import` nodes at the root level
        if child.kind() == "import" || child.kind() == "import_header" {
            if let Some(info) = parse_import_node(&child, src) {
                imports.push(info);
            }
        }
        // Also check for import_list wrapper
        if child.kind() == "import_list" {
            let mut inner = child.walk();
            for import_node in child.children(&mut inner) {
                if import_node.kind() == "import" || import_node.kind() == "import_header" {
                    if let Some(info) = parse_import_node(&import_node, src) {
                        imports.push(info);
                    }
                }
            }
        }
    }

    imports
}

fn parse_import_node(node: &tree_sitter::Node, src: &[u8]) -> Option<ImportInfo> {
    // The AST structure for imports:
    //   import com.other.Foo          -> qualified_identifier("com.other.Foo")
    //   import com.other.Bar as Baz   -> qualified_identifier("com.other.Bar"), as, identifier("Baz")
    //   import com.util.*             -> qualified_identifier("com.util"), ., *
    let mut path = None;
    let mut alias = None;
    let mut is_wildcard = false;
    let mut seen_as = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "qualified_identifier" => {
                path = Some(node_text(&child, src).to_string());
            }
            "as" => {
                seen_as = true;
            }
            "identifier" if seen_as => {
                // This is the alias name after "as"
                alias = Some(node_text(&child, src).to_string());
            }
            "identifier" if path.is_none() => {
                // Simple single-segment import (no qualified_identifier)
                path = Some(node_text(&child, src).to_string());
            }
            "*" => {
                is_wildcard = true;
            }
            "import_alias" => {
                let mut inner = child.walk();
                for c in child.children(&mut inner) {
                    if c.kind() == "identifier"
                        || c.kind() == "type_identifier"
                        || c.kind() == "simple_identifier"
                    {
                        alias = Some(node_text(&c, src).to_string());
                    }
                }
            }
            _ => {}
        }
    }

    path.map(|path| ImportInfo {
        path,
        alias,
        is_wildcard,
        line: node.start_position().row + 1,
        column: node.start_position().column + 1,
        byte_range: node.byte_range(),
    })
}

fn build_scope_tree(root: &tree_sitter::Node, src: &[u8]) -> ScopeTree {
    let mut scope_tree = ScopeTree::new();
    collect_scopes(root, src, &mut scope_tree);
    scope_tree.finalize();
    scope_tree
}

fn collect_scopes(node: &tree_sitter::Node, src: &[u8], tree: &mut ScopeTree) {
    match node.kind() {
        "class_declaration"
        | "object_declaration"
        | "enum_class_body" => {
            if let Some(name) = find_child_name(node, src) {
                // Only add scope if there's a body — no body means no nested declarations
                if let Some(range) = find_body_range(node) {
                    tree.add_scope(name, range);
                }
            }
        }
        "companion_object" => {
            if let Some(range) = find_body_range(node) {
                tree.add_scope("Companion".to_string(), range);
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_scopes(&child, src, tree);
    }
}

fn extract_declarations(
    node: &tree_sitter::Node,
    src: &[u8],
    path: &Path,
    package: Option<&str>,
    scope_tree: &ScopeTree,
    occurrences: &mut Vec<SymbolOccurrence>,
    type_aliases: &mut Vec<(String, String)>,
) {
    match node.kind() {
        "class_declaration" => {
            if let Some(name) = find_child_name(node, src) {
                let fqn = build_fqn(package, scope_tree, node.start_byte(), &name);
                // tree-sitter-kotlin-ng uses class_declaration for both classes and interfaces.
                // Check for the "interface" keyword child to distinguish them.
                let kind = if has_keyword_child(node, "interface") {
                    SymbolKind::InterfaceDeclaration
                } else {
                    SymbolKind::ClassDeclaration
                };
                occurrences.push(SymbolOccurrence {
                    name: name.clone(),
                    fqn: Some(fqn),
                    kind,
                    file: path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: node.start_position().column + 1,
                    byte_range: node.byte_range(),
                    receiver_type: None,
                });
            }
        }
        "object_declaration" => {
            if let Some(name) = find_child_name(node, src) {
                let fqn = build_fqn(package, scope_tree, node.start_byte(), &name);
                occurrences.push(SymbolOccurrence {
                    name: name.clone(),
                    fqn: Some(fqn),
                    kind: SymbolKind::ObjectDeclaration,
                    file: path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: node.start_position().column + 1,
                    byte_range: node.byte_range(),
                    receiver_type: None,
                });
            }
        }
        "companion_object" => {
            let name = find_child_name(node, src).unwrap_or_else(|| "Companion".to_string());
            let fqn = build_fqn(package, scope_tree, node.start_byte(), &name);
            occurrences.push(SymbolOccurrence {
                name,
                fqn: Some(fqn),
                kind: SymbolKind::CompanionObjectDeclaration,
                file: path.to_path_buf(),
                line: node.start_position().row + 1,
                column: node.start_position().column + 1,
                byte_range: node.byte_range(),
                receiver_type: None,
            });
        }
        "function_declaration" => {
            if let Some(name) = find_child_name(node, src) {
                // Check for extension function (has receiver type)
                let receiver = find_receiver_type(node, src);
                let kind = if receiver.is_some() {
                    SymbolKind::ExtensionFunctionDeclaration
                } else {
                    SymbolKind::FunctionDeclaration
                };
                let fqn = build_fqn(package, scope_tree, node.start_byte(), &name);
                occurrences.push(SymbolOccurrence {
                    name: name.clone(),
                    fqn: Some(fqn),
                    kind,
                    file: path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: node.start_position().column + 1,
                    byte_range: node.byte_range(),
                    receiver_type: receiver,
                });
            }
        }
        "property_declaration" => {
            if let Some(name) = find_property_name(node, src) {
                let fqn = build_fqn(package, scope_tree, node.start_byte(), &name);
                occurrences.push(SymbolOccurrence {
                    name: name.clone(),
                    fqn: Some(fqn),
                    kind: SymbolKind::PropertyDeclaration,
                    file: path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: node.start_position().column + 1,
                    byte_range: node.byte_range(),
                    receiver_type: None,
                });
            }
        }
        "enum_entry" => {
            if let Some(name) = find_child_name(node, src) {
                let fqn = build_fqn(package, scope_tree, node.start_byte(), &name);
                occurrences.push(SymbolOccurrence {
                    name: name.clone(),
                    fqn: Some(fqn),
                    kind: SymbolKind::EnumEntryDeclaration,
                    file: path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: node.start_position().column + 1,
                    byte_range: node.byte_range(),
                    receiver_type: None,
                });
            }
        }
        "type_alias" => {
            if let Some(name) = find_child_name(node, src) {
                let fqn = build_fqn(package, scope_tree, node.start_byte(), &name);
                // Find the aliased type
                if let Some(target) = find_type_alias_target(node, src) {
                    type_aliases.push((fqn.clone(), target));
                }
                occurrences.push(SymbolOccurrence {
                    name: name.clone(),
                    fqn: Some(fqn),
                    kind: SymbolKind::TypeAliasDeclaration,
                    file: path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: node.start_position().column + 1,
                    byte_range: node.byte_range(),
                    receiver_type: None,
                });
            }
        }
        _ => {}
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_declarations(&child, src, path, package, scope_tree, occurrences, type_aliases);
    }
}

fn extract_references(
    node: &tree_sitter::Node,
    src: &[u8],
    path: &Path,
    package: Option<&str>,
    scope_tree: &ScopeTree,
    imports: &[ImportInfo],
    occurrences: &mut Vec<SymbolOccurrence>,
) {
    match node.kind() {
        "call_expression" => {
            // Extract the function name from the call
            if let Some(name_node) = node.child(0) {
                let text = node_text(&name_node, src);
                // Check if it's a navigation expression like `foo.bar()`
                if name_node.kind() == "navigation_expression" {
                    if let Some(member) = name_node.child_by_field_name("member").or_else(|| {
                        // Last child is typically the member
                        let count = name_node.child_count();
                        if count > 0 {
                            name_node.child(count - 1)
                        } else {
                            None
                        }
                    }) {
                        let member_name = node_text(&member, src).to_string();
                        let fqn = resolve_reference(&member_name, package, imports);
                        occurrences.push(SymbolOccurrence {
                            name: member_name,
                            fqn,
                            kind: SymbolKind::CallSite,
                            file: path.to_path_buf(),
                            line: node.start_position().row + 1,
                            column: node.start_position().column + 1,
                            byte_range: node.byte_range(),
                            receiver_type: extract_receiver_from_nav(&name_node, src),
                        });
                        // Process the receiver of the navigation expression
                        extract_nav_receiver(&name_node, src, path, package, scope_tree, imports, occurrences);
                        // Recurse into arguments (skip the navigation_expression itself)
                        let mut cursor = node.walk();
                        for child in node.children(&mut cursor) {
                            if child.id() != name_node.id() {
                                extract_references(&child, src, path, package, scope_tree, imports, occurrences);
                            }
                        }
                        return;
                    }
                } else if name_node.kind() == "simple_identifier" || name_node.kind() == "identifier" {
                    let name = text.to_string();
                    let fqn = resolve_reference(&name, package, imports);
                    occurrences.push(SymbolOccurrence {
                        name,
                        fqn,
                        kind: SymbolKind::CallSite,
                        file: path.to_path_buf(),
                        line: node.start_position().row + 1,
                        column: node.start_position().column + 1,
                        byte_range: node.byte_range(),
                        receiver_type: None,
                    });
                    // Recurse into arguments only
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.id() != name_node.id() {
                            extract_references(&child, src, path, package, scope_tree, imports, occurrences);
                        }
                    }
                    return;
                }
            }
        }
        "navigation_expression" => {
            // Only handle if not already handled by parent call_expression
            if let Some(parent) = node.parent() {
                if parent.kind() == "call_expression" {
                    // Will be handled by call_expression
                    return;
                }
            }
            // Property access like `foo.bar`, `foo?.bar`, `Foo::bar`
            let count = node.child_count();
            if count > 0 {
                if let Some(member) = node.child(count - 1) {
                    if member.kind() == "simple_identifier" || member.kind() == "identifier" || member.kind() == "navigation_suffix" {
                        let member_name = node_text(&member, src).to_string();
                        let fqn = resolve_reference(&member_name, package, imports);
                        occurrences.push(SymbolOccurrence {
                            name: member_name,
                            fqn,
                            kind: SymbolKind::PropertyReference,
                            file: path.to_path_buf(),
                            line: node.start_position().row + 1,
                            column: node.start_position().column + 1,
                            byte_range: node.byte_range(),
                            receiver_type: extract_receiver_from_nav(node, src),
                        });
                    }
                }
                // Process the receiver to capture it as a reference
                extract_nav_receiver(node, src, path, package, scope_tree, imports, occurrences);
            }
            return;
        }
        "user_type" => {
            // Type references like `: Foo` or `Foo<Bar>`
            let text = node_text(node, src);
            // Get the simple type name (first identifier)
            let type_name = text.split('<').next().unwrap_or(&text).trim().to_string();
            if !type_name.is_empty() && type_name.chars().next().is_some_and(|c| c.is_uppercase()) {
                let fqn = resolve_reference(&type_name, package, imports);
                occurrences.push(SymbolOccurrence {
                    name: type_name,
                    fqn,
                    kind: SymbolKind::TypeReference,
                    file: path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: node.start_position().column + 1,
                    byte_range: node.byte_range(),
                    receiver_type: None,
                });
            }
            // Don't recurse into type parameters - they'll be handled separately
            return;
        }
        "simple_identifier" | "identifier" => {
            // Bare identifier used as a value reference (e.g., passed as argument,
            // assigned to variable). Only capture if not already handled by another case.
            if let Some(parent) = node.parent() {
                let pk = parent.kind();
                let dominated = matches!(
                    pk,
                    // Declaration names
                    "class_declaration"
                        | "object_declaration"
                        | "function_declaration"
                        | "variable_declaration"
                        | "parameter"
                        | "companion_object"
                        | "enum_entry"
                        | "type_alias"
                        // Import / package path components
                        | "import"
                        | "import_header"
                        | "import_alias"
                        | "import_list"
                        | "package_header"
                        | "qualified_identifier"
                        // Already handled by navigation_expression / user_type
                        | "navigation_expression"
                        | "navigation_suffix"
                        | "user_type"
                        // Type parameters and annotations
                        | "type_parameter"
                        | "type_constraint"
                        | "annotation"
                        // Labels
                        | "label"
                );
                if !dominated {
                    // Also skip if this is child(0) of a call_expression (callee, already handled)
                    let is_callee = pk == "call_expression"
                        && parent.child(0).is_some_and(|c| c.id() == node.id());
                    if !is_callee {
                        let name = node_text(node, src).to_string();
                        if !name.is_empty() {
                            let fqn = resolve_reference(&name, package, imports);
                            occurrences.push(SymbolOccurrence {
                                name,
                                fqn,
                                kind: SymbolKind::PropertyReference,
                                file: path.to_path_buf(),
                                line: node.start_position().row + 1,
                                column: node.start_position().column + 1,
                                byte_range: node.byte_range(),
                                receiver_type: None,
                            });
                        }
                    }
                }
            }
            return;
        }
        _ => {}
    }

    // Recurse
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_references(&child, src, path, package, scope_tree, imports, occurrences);
    }
}

fn resolve_reference(name: &str, package: Option<&str>, imports: &[ImportInfo]) -> Option<String> {
    // Check explicit imports first
    for imp in imports {
        if imp.is_wildcard {
            continue;
        }
        let imported_name = if let Some(ref alias) = imp.alias {
            alias.as_str()
        } else {
            imp.path.rsplit('.').next().unwrap_or(&imp.path)
        };
        if imported_name == name {
            if imp.alias.is_some() {
                // Aliased import: the FQN is the original path
                return Some(imp.path.clone());
            }
            return Some(imp.path.clone());
        }
    }

    // Check wildcard imports - we can't fully resolve without the full index,
    // so we return None and let cross-referencing handle it
    // But we note the package for same-package resolution
    if let Some(pkg) = package {
        return Some(format!("{}.{}", pkg, name));
    }

    None
}

/// Process the receiver (child 0) of a navigation_expression, capturing it as a reference.
/// For a leaf identifier receiver (e.g., `Config` in `Config.foo`), emit it directly.
/// For a complex receiver (e.g., another navigation in `a.b.c`), recurse into it.
fn extract_nav_receiver(
    nav_node: &tree_sitter::Node,
    src: &[u8],
    path: &Path,
    package: Option<&str>,
    scope_tree: &ScopeTree,
    imports: &[ImportInfo],
    occurrences: &mut Vec<SymbolOccurrence>,
) {
    if let Some(receiver) = nav_node.child(0) {
        if receiver.kind() == "simple_identifier" || receiver.kind() == "identifier" {
            // Leaf receiver — capture directly as a reference
            let name = node_text(&receiver, src).to_string();
            if !name.is_empty() {
                let fqn = resolve_reference(&name, package, imports);
                occurrences.push(SymbolOccurrence {
                    name,
                    fqn,
                    kind: SymbolKind::PropertyReference,
                    file: path.to_path_buf(),
                    line: receiver.start_position().row + 1,
                    column: receiver.start_position().column + 1,
                    byte_range: receiver.byte_range(),
                    receiver_type: None,
                });
            }
        } else {
            // Complex receiver (e.g., nested navigation_expression, call_expression) — recurse
            extract_references(&receiver, src, path, package, scope_tree, imports, occurrences);
        }
    }
}

fn extract_receiver_from_nav(nav_node: &tree_sitter::Node, src: &[u8]) -> Option<String> {
    if nav_node.child_count() >= 2 {
        if let Some(receiver) = nav_node.child(0) {
            let text = node_text(&receiver, src).to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

fn build_fqn(
    package: Option<&str>,
    scope_tree: &ScopeTree,
    byte_offset: usize,
    name: &str,
) -> String {
    let prefix = scope_tree.fqn_prefix_at(package, byte_offset);
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{}.{}", prefix, name)
    }
}

fn find_body_range(node: &tree_sitter::Node) -> Option<std::ops::Range<usize>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "class_body"
            || child.kind() == "enum_class_body"
            || child.kind() == "object_body"
        {
            return Some(child.byte_range());
        }
    }
    None
}

fn find_child_name(node: &tree_sitter::Node, src: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier"
            || child.kind() == "type_identifier"
            || child.kind() == "simple_identifier"
        {
            return Some(node_text(&child, src).to_string());
        }
    }
    None
}

fn find_property_name(node: &tree_sitter::Node, src: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declaration" {
            let mut inner = child.walk();
            for c in child.children(&mut inner) {
                if c.kind() == "identifier" || c.kind() == "simple_identifier" {
                    return Some(node_text(&c, src).to_string());
                }
            }
        }
        // Also check for direct identifier (e.g., top-level val)
        if child.kind() == "identifier" || child.kind() == "simple_identifier" {
            return Some(node_text(&child, src).to_string());
        }
    }
    None
}

fn find_receiver_type(func_node: &tree_sitter::Node, src: &[u8]) -> Option<String> {
    // Extension functions have a receiver type before the function name
    // In the AST: function_declaration -> user_type (receiver) -> simple_identifier (name)
    let mut cursor = func_node.walk();
    for child in func_node.children(&mut cursor) {
        if child.kind() == "user_type" {
            // This is the receiver type (appears before the function name)
            return Some(node_text(&child, src).to_string());
        }
        if child.kind() == "identifier" || child.kind() == "simple_identifier" {
            // No receiver type before the name
            return None;
        }
    }
    None
}

fn find_type_alias_target(node: &tree_sitter::Node, src: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    let mut found_eq = false;
    for child in node.children(&mut cursor) {
        if child.kind() == "=" {
            found_eq = true;
            continue;
        }
        if found_eq
            && (child.kind() == "user_type"
                || child.kind() == "type_identifier"
                || child.kind() == "identifier")
        {
            return Some(node_text(&child, src).to_string());
        }
    }
    None
}

fn has_keyword_child(node: &tree_sitter::Node, keyword: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == keyword {
            return true;
        }
    }
    false
}

fn node_text<'a>(node: &tree_sitter::Node, src: &'a [u8]) -> &'a str {
    node.utf8_text(src).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interface_parsing() {
        let source = "package com.example\n\ninterface Repository<T> {\n    fun findById(id: String): T?\n}\n";
        let file_path = std::path::PathBuf::from("Test.kt");
        let (_, occurrences, _) = parse_file(&file_path, source);
        let repo = occurrences
            .iter()
            .find(|o| o.name == "Repository")
            .expect("Expected Repository in occurrences");
        assert!(
            matches!(repo.kind, super::SymbolKind::InterfaceDeclaration),
            "Expected InterfaceDeclaration, got {:?}",
            repo.kind
        );
        assert_eq!(repo.fqn.as_deref(), Some("com.example.Repository"));
    }

    #[test]
    fn test_discover_files() {
        // Just test the function doesn't panic with a temp dir
        let dir = tempfile::tempdir().unwrap();
        let files = discover_kotlin_files(dir.path());
        assert!(files.is_empty());
    }

    #[test]
    fn test_parse_simple_file() {
        let source = r#"
package com.example

import java.util.List

class MyClass {
    fun myMethod(): String {
        return "hello"
    }

    val myProperty: Int = 42
}

fun topLevelFunction() {}
"#;
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("Test.kt");
        std::fs::write(&file_path, source).unwrap();

        let (file_info, occurrences, _) = parse_file(&file_path, source);
        assert_eq!(file_info.package, Some("com.example".to_string()));
        assert_eq!(file_info.imports.len(), 1);
        assert_eq!(file_info.imports[0].path, "java.util.List");

        // Check that we have class, method, property, and top-level function declarations
        let decl_names: Vec<&str> = occurrences
            .iter()
            .filter(|o| o.kind.is_declaration())
            .map(|o| o.name.as_str())
            .collect();
        assert!(decl_names.contains(&"MyClass"), "Expected MyClass declaration, got: {:?}", decl_names);
        assert!(decl_names.contains(&"myMethod"), "Expected myMethod declaration, got: {:?}", decl_names);
        assert!(decl_names.contains(&"myProperty"), "Expected myProperty declaration, got: {:?}", decl_names);
        assert!(decl_names.contains(&"topLevelFunction"), "Expected topLevelFunction, got: {:?}", decl_names);
    }

    #[test]
    fn test_parse_imports() {
        let source = r#"
package com.example

import com.other.Foo
import com.other.Bar as Baz
import com.util.*
"#;
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("Test.kt");

        let (file_info, _, _) = parse_file(&file_path, source);
        assert_eq!(file_info.imports.len(), 3);

        let foo = &file_info.imports[0];
        assert_eq!(foo.path, "com.other.Foo");
        assert!(!foo.is_wildcard);
        assert!(foo.alias.is_none());

        let baz = &file_info.imports[1];
        assert_eq!(baz.path, "com.other.Bar");
        assert_eq!(baz.alias, Some("Baz".to_string()));

        let wildcard = &file_info.imports[2];
        assert!(wildcard.is_wildcard);
    }

}
