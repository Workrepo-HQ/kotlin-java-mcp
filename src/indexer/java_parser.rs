use std::path::Path;

use tracing::warn;

use super::parser::{build_fqn, find_child_name, node_text, resolve_reference};
use super::scope::ScopeTree;
use super::{FileInfo, ImportInfo, SymbolKind, SymbolOccurrence};

/// Parse a single Java file and extract symbols.
pub fn parse_java_file(
    path: &Path,
    source: &str,
) -> (FileInfo, Vec<SymbolOccurrence>, Vec<(String, String)>) {
    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter_java::LANGUAGE;
    parser
        .set_language(&language.into())
        .expect("Failed to set Java language");

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

    let package = extract_package_java(&root, src);
    let imports = extract_imports_java(&root, src);
    let scope_tree = build_scope_tree_java(&root, src);

    let mut occurrences = Vec::new();
    let type_aliases = Vec::new();

    extract_declarations_java(
        &root,
        src,
        path,
        package.as_deref(),
        &scope_tree,
        &mut occurrences,
    );

    extract_references_java(
        &root,
        src,
        path,
        package.as_deref(),
        &scope_tree,
        &imports,
        &mut occurrences,
    );

    // Add import occurrences
    for imp in &imports {
        let name = if imp.is_wildcard {
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
        package,
        imports,
    };

    (file_info, occurrences, type_aliases)
}

fn extract_package_java(root: &tree_sitter::Node, src: &[u8]) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "package_declaration" {
            // The package name is in a scoped_identifier or identifier child
            let mut inner = child.walk();
            for c in child.children(&mut inner) {
                if c.kind() == "scoped_identifier" || c.kind() == "identifier" {
                    return Some(node_text(&c, src).to_string());
                }
            }
        }
    }
    None
}

fn extract_imports_java(root: &tree_sitter::Node, src: &[u8]) -> Vec<ImportInfo> {
    let mut imports = Vec::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() == "import_declaration" {
            if let Some(info) = parse_java_import(&child, src) {
                imports.push(info);
            }
        }
    }

    imports
}

fn parse_java_import(node: &tree_sitter::Node, src: &[u8]) -> Option<ImportInfo> {
    // Java import AST: import_declaration -> [static] scoped_identifier [. asterisk]
    // or: import_declaration -> [static] identifier
    let mut path = None;
    let mut is_wildcard = false;
    let mut is_static = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "static" => {
                is_static = true;
            }
            "scoped_identifier" => {
                path = Some(node_text(&child, src).to_string());
            }
            "identifier" if path.is_none() => {
                path = Some(node_text(&child, src).to_string());
            }
            "asterisk" => {
                is_wildcard = true;
            }
            _ => {}
        }
    }

    // For static imports like `import static com.example.Foo.bar`,
    // the path includes the member name. We store the full path.
    // For wildcard static imports like `import static com.example.Foo.*`,
    // path is the class FQN and is_wildcard is true.
    let _ = is_static; // tracked for potential future use

    path.map(|path| ImportInfo {
        path,
        alias: None,
        is_wildcard,
        line: node.start_position().row + 1,
        column: node.start_position().column + 1,
        byte_range: node.byte_range(),
    })
}

fn build_scope_tree_java(root: &tree_sitter::Node, src: &[u8]) -> ScopeTree {
    let mut scope_tree = ScopeTree::new();
    collect_scopes_java(root, src, &mut scope_tree);
    scope_tree.finalize();
    scope_tree
}

fn collect_scopes_java(node: &tree_sitter::Node, src: &[u8], tree: &mut ScopeTree) {
    match node.kind() {
        "class_declaration" | "interface_declaration" | "enum_declaration"
        | "record_declaration" | "annotation_type_declaration" => {
            if let Some(name) = find_child_name(node, src) {
                if let Some(range) = find_java_body_range(node) {
                    tree.add_scope(name, range);
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_scopes_java(&child, src, tree);
    }
}

fn extract_declarations_java(
    node: &tree_sitter::Node,
    src: &[u8],
    path: &Path,
    package: Option<&str>,
    scope_tree: &ScopeTree,
    occurrences: &mut Vec<SymbolOccurrence>,
) {
    match node.kind() {
        "class_declaration" => {
            if let Some(name) = find_child_name(node, src) {
                let fqn = build_fqn(package, scope_tree, node.start_byte(), &name);
                occurrences.push(SymbolOccurrence {
                    name,
                    fqn: Some(fqn),
                    kind: SymbolKind::ClassDeclaration,
                    file: path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: node.start_position().column + 1,
                    byte_range: node.byte_range(),
                    receiver_type: None,
                });
            }
        }
        "interface_declaration" => {
            if let Some(name) = find_child_name(node, src) {
                let fqn = build_fqn(package, scope_tree, node.start_byte(), &name);
                occurrences.push(SymbolOccurrence {
                    name,
                    fqn: Some(fqn),
                    kind: SymbolKind::InterfaceDeclaration,
                    file: path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: node.start_position().column + 1,
                    byte_range: node.byte_range(),
                    receiver_type: None,
                });
            }
        }
        "enum_declaration" => {
            if let Some(name) = find_child_name(node, src) {
                let fqn = build_fqn(package, scope_tree, node.start_byte(), &name);
                occurrences.push(SymbolOccurrence {
                    name,
                    fqn: Some(fqn),
                    kind: SymbolKind::ClassDeclaration,
                    file: path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: node.start_position().column + 1,
                    byte_range: node.byte_range(),
                    receiver_type: None,
                });
            }
        }
        "enum_constant" => {
            if let Some(name) = find_child_name(node, src) {
                let fqn = build_fqn(package, scope_tree, node.start_byte(), &name);
                occurrences.push(SymbolOccurrence {
                    name,
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
        "record_declaration" => {
            if let Some(name) = find_child_name(node, src) {
                let fqn = build_fqn(package, scope_tree, node.start_byte(), &name);
                occurrences.push(SymbolOccurrence {
                    name,
                    fqn: Some(fqn),
                    kind: SymbolKind::RecordDeclaration,
                    file: path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: node.start_position().column + 1,
                    byte_range: node.byte_range(),
                    receiver_type: None,
                });
            }
        }
        "annotation_type_declaration" => {
            if let Some(name) = find_child_name(node, src) {
                let fqn = build_fqn(package, scope_tree, node.start_byte(), &name);
                occurrences.push(SymbolOccurrence {
                    name,
                    fqn: Some(fqn),
                    kind: SymbolKind::AnnotationTypeDeclaration,
                    file: path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: node.start_position().column + 1,
                    byte_range: node.byte_range(),
                    receiver_type: None,
                });
            }
        }
        "method_declaration" => {
            // Use the "name" field to avoid picking up the return type identifier
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(&name_node, src).to_string();
                let fqn = build_fqn(package, scope_tree, node.start_byte(), &name);
                occurrences.push(SymbolOccurrence {
                    name,
                    fqn: Some(fqn),
                    kind: SymbolKind::FunctionDeclaration,
                    file: path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: node.start_position().column + 1,
                    byte_range: node.byte_range(),
                    receiver_type: None,
                });
            }
        }
        "constructor_declaration" => {
            if let Some(name) = find_child_name(node, src) {
                let fqn = build_fqn(package, scope_tree, node.start_byte(), &name);
                occurrences.push(SymbolOccurrence {
                    name,
                    fqn: Some(fqn),
                    kind: SymbolKind::ConstructorDeclaration,
                    file: path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: node.start_position().column + 1,
                    byte_range: node.byte_range(),
                    receiver_type: None,
                });
            }
        }
        "field_declaration" => {
            // A field_declaration may contain multiple variable_declarators
            extract_field_declarations(node, src, path, package, scope_tree, occurrences);
            // Don't recurse into children; we handle them above
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_declarations_java(&child, src, path, package, scope_tree, occurrences);
    }
}

fn extract_field_declarations(
    node: &tree_sitter::Node,
    src: &[u8],
    path: &Path,
    package: Option<&str>,
    scope_tree: &ScopeTree,
    occurrences: &mut Vec<SymbolOccurrence>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            if let Some(name) = find_child_name(&child, src) {
                let fqn = build_fqn(package, scope_tree, node.start_byte(), &name);
                occurrences.push(SymbolOccurrence {
                    name,
                    fqn: Some(fqn),
                    kind: SymbolKind::PropertyDeclaration,
                    file: path.to_path_buf(),
                    line: child.start_position().row + 1,
                    column: child.start_position().column + 1,
                    byte_range: child.byte_range(),
                    receiver_type: None,
                });
            }
        }
    }
}

fn extract_references_java(
    node: &tree_sitter::Node,
    src: &[u8],
    path: &Path,
    package: Option<&str>,
    scope_tree: &ScopeTree,
    imports: &[ImportInfo],
    occurrences: &mut Vec<SymbolOccurrence>,
) {
    match node.kind() {
        "method_invocation" => {
            // method_invocation has "name" field for the method name and "object" field for receiver
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(&name_node, src).to_string();
                let receiver = node
                    .child_by_field_name("object")
                    .map(|r| node_text(&r, src).to_string());
                let fqn = resolve_reference(&name, package, imports);
                occurrences.push(SymbolOccurrence {
                    name,
                    fqn,
                    kind: SymbolKind::CallSite,
                    file: path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: node.start_position().column + 1,
                    byte_range: node.byte_range(),
                    receiver_type: receiver,
                });
            }
            // Recurse into children (arguments, receiver) but skip the name node
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if node
                    .child_by_field_name("name")
                    .is_some_and(|n| n.id() == child.id())
                {
                    continue;
                }
                extract_references_java(
                    &child, src, path, package, scope_tree, imports, occurrences,
                );
            }
            return;
        }
        "object_creation_expression" => {
            // `new Foo(...)` — the type is the first type_identifier child
            if let Some(type_node) = find_type_child(node) {
                let name = node_text(&type_node, src).to_string();
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
            }
            // Recurse into arguments
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "argument_list" {
                    extract_references_java(
                        &child, src, path, package, scope_tree, imports, occurrences,
                    );
                }
            }
            return;
        }
        "field_access" => {
            // `obj.field` — the field name is in the "field" named child
            if let Some(field_node) = node.child_by_field_name("field") {
                let name = node_text(&field_node, src).to_string();
                let receiver = node
                    .child_by_field_name("object")
                    .map(|r| node_text(&r, src).to_string());
                let fqn = resolve_reference(&name, package, imports);
                occurrences.push(SymbolOccurrence {
                    name,
                    fqn,
                    kind: SymbolKind::PropertyReference,
                    file: path.to_path_buf(),
                    line: node.start_position().row + 1,
                    column: node.start_position().column + 1,
                    byte_range: node.byte_range(),
                    receiver_type: receiver,
                });
            }
            // Process the receiver
            if let Some(obj_node) = node.child_by_field_name("object") {
                extract_references_java(
                    &obj_node, src, path, package, scope_tree, imports, occurrences,
                );
            }
            return;
        }
        "type_identifier" => {
            // Type references like `Foo`, `Bar` in extends/implements, variable types, etc.
            // Skip if parent is already a declaration node (the name of the declaration)
            if let Some(parent) = node.parent() {
                let pk = parent.kind();
                let is_decl_name = matches!(
                    pk,
                    "class_declaration"
                        | "interface_declaration"
                        | "enum_declaration"
                        | "record_declaration"
                        | "annotation_type_declaration"
                        | "enum_constant"
                        | "import_declaration"
                        | "package_declaration"
                        | "scoped_identifier"
                        | "scoped_type_identifier"
                );
                if !is_decl_name {
                    let name = node_text(node, src).to_string();
                    if !name.is_empty() {
                        let fqn = resolve_reference(&name, package, imports);
                        occurrences.push(SymbolOccurrence {
                            name,
                            fqn,
                            kind: SymbolKind::TypeReference,
                            file: path.to_path_buf(),
                            line: node.start_position().row + 1,
                            column: node.start_position().column + 1,
                            byte_range: node.byte_range(),
                            receiver_type: None,
                        });
                    }
                }
            }
            return;
        }
        "identifier" => {
            // Bare identifier as a value reference.
            // Skip if in a context already handled by other cases.
            if let Some(parent) = node.parent() {
                let pk = parent.kind();
                let dominated = matches!(
                    pk,
                    "class_declaration"
                        | "interface_declaration"
                        | "enum_declaration"
                        | "record_declaration"
                        | "annotation_type_declaration"
                        | "enum_constant"
                        | "method_declaration"
                        | "constructor_declaration"
                        | "import_declaration"
                        | "package_declaration"
                        | "scoped_identifier"
                        | "scoped_type_identifier"
                        | "field_access"
                        | "variable_declarator"
                        | "formal_parameter"
                        | "type_parameter"
                        | "annotation"
                        | "marker_annotation"
                        | "catch_formal_parameter"
                        | "enhanced_for_statement"
                        | "local_variable_declaration"
                        | "label"
                        | "break_statement"
                        | "continue_statement"
                );
                if !dominated {
                    // Skip if this is the "name" field of a method_invocation
                    let is_method_name = pk == "method_invocation"
                        && parent
                            .child_by_field_name("name")
                            .is_some_and(|n| n.id() == node.id());
                    if !is_method_name {
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
        extract_references_java(&child, src, path, package, scope_tree, imports, occurrences);
    }
}

fn find_java_body_range(node: &tree_sitter::Node) -> Option<std::ops::Range<usize>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "class_body"
            || child.kind() == "interface_body"
            || child.kind() == "enum_body"
            || child.kind() == "annotation_type_body"
            || child.kind() == "record_declaration_body"
        {
            return Some(child.byte_range());
        }
    }
    None
}

fn find_type_child<'a>(node: &'a tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_identifier" || child.kind() == "identifier" {
            return Some(child);
        }
        // For generic types like `new ArrayList<String>()`, the type is in a generic_type node
        if child.kind() == "generic_type" {
            let mut inner = child.walk();
            for c in child.children(&mut inner) {
                if c.kind() == "type_identifier" || c.kind() == "identifier" {
                    return Some(c);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_java_class() {
        let source = r#"
package com.example;

public class MyClass {
    private String name;
    private int count;

    public MyClass(String name) {
        this.name = name;
    }

    public String getName() {
        return name;
    }

    public void setCount(int count) {
        this.count = count;
    }
}
"#;
        let path = PathBuf::from("MyClass.java");
        let (file_info, occurrences, _) = parse_java_file(&path, source);

        assert_eq!(file_info.package, Some("com.example".to_string()));

        let decl_names: Vec<&str> = occurrences
            .iter()
            .filter(|o| o.kind.is_declaration())
            .map(|o| o.name.as_str())
            .collect();
        assert!(
            decl_names.contains(&"MyClass"),
            "Expected MyClass declaration, got: {:?}",
            decl_names
        );
        assert!(
            decl_names.contains(&"name"),
            "Expected name field, got: {:?}",
            decl_names
        );
        assert!(
            decl_names.contains(&"count"),
            "Expected count field, got: {:?}",
            decl_names
        );
        assert!(
            decl_names.contains(&"getName"),
            "Expected getName method, got: {:?}",
            decl_names
        );
        assert!(
            decl_names.contains(&"setCount"),
            "Expected setCount method, got: {:?}",
            decl_names
        );

        // Check FQNs
        let class_occ = occurrences
            .iter()
            .find(|o| o.name == "MyClass" && o.kind.is_declaration())
            .unwrap();
        assert_eq!(class_occ.fqn.as_deref(), Some("com.example.MyClass"));

        let get_name = occurrences
            .iter()
            .find(|o| o.name == "getName" && o.kind.is_declaration())
            .unwrap();
        assert_eq!(
            get_name.fqn.as_deref(),
            Some("com.example.MyClass.getName")
        );
    }

    #[test]
    fn test_parse_java_constructor() {
        let source = r#"
package com.example;

public class Foo {
    public Foo(int x) {}
}
"#;
        let path = PathBuf::from("Foo.java");
        let (_, occurrences, _) = parse_java_file(&path, source);

        let ctor = occurrences
            .iter()
            .find(|o| o.name == "Foo" && matches!(o.kind, SymbolKind::ConstructorDeclaration))
            .expect("Expected constructor declaration");
        assert_eq!(ctor.fqn.as_deref(), Some("com.example.Foo.Foo"));
    }

    #[test]
    fn test_parse_java_interface() {
        let source = r#"
package com.example;

public interface MyInterface {
    void doSomething();
    String getValue();
}
"#;
        let path = PathBuf::from("MyInterface.java");
        let (_, occurrences, _) = parse_java_file(&path, source);

        let iface = occurrences
            .iter()
            .find(|o| o.name == "MyInterface" && o.kind.is_declaration())
            .expect("Expected MyInterface declaration");
        assert!(matches!(iface.kind, SymbolKind::InterfaceDeclaration));
        assert_eq!(
            iface.fqn.as_deref(),
            Some("com.example.MyInterface")
        );

        let methods: Vec<&str> = occurrences
            .iter()
            .filter(|o| matches!(o.kind, SymbolKind::FunctionDeclaration))
            .map(|o| o.name.as_str())
            .collect();
        assert!(methods.contains(&"doSomething"));
        assert!(methods.contains(&"getValue"));
    }

    #[test]
    fn test_parse_java_enum() {
        let source = r#"
package com.example;

public enum Color {
    RED,
    GREEN,
    BLUE;

    public String display() {
        return name().toLowerCase();
    }
}
"#;
        let path = PathBuf::from("Color.java");
        let (_, occurrences, _) = parse_java_file(&path, source);

        let enum_decl = occurrences
            .iter()
            .find(|o| o.name == "Color" && o.kind.is_declaration())
            .expect("Expected Color declaration");
        assert!(matches!(enum_decl.kind, SymbolKind::ClassDeclaration));

        let entries: Vec<&str> = occurrences
            .iter()
            .filter(|o| matches!(o.kind, SymbolKind::EnumEntryDeclaration))
            .map(|o| o.name.as_str())
            .collect();
        assert!(entries.contains(&"RED"));
        assert!(entries.contains(&"GREEN"));
        assert!(entries.contains(&"BLUE"));
    }

    #[test]
    fn test_parse_java_imports() {
        let source = r#"
package com.example;

import java.util.List;
import java.util.Map;
import static java.util.Collections.emptyList;
import java.io.*;
"#;
        let path = PathBuf::from("Test.java");
        let (file_info, _, _) = parse_java_file(&path, source);

        assert_eq!(file_info.imports.len(), 4);

        let list_imp = &file_info.imports[0];
        assert_eq!(list_imp.path, "java.util.List");
        assert!(!list_imp.is_wildcard);

        let map_imp = &file_info.imports[1];
        assert_eq!(map_imp.path, "java.util.Map");

        let static_imp = &file_info.imports[2];
        assert_eq!(static_imp.path, "java.util.Collections.emptyList");
        assert!(!static_imp.is_wildcard);

        let wildcard_imp = &file_info.imports[3];
        assert_eq!(wildcard_imp.path, "java.io");
        assert!(wildcard_imp.is_wildcard);
    }

    #[test]
    fn test_parse_java_references() {
        let source = r#"
package com.example;

import com.other.Helper;

public class Caller {
    public void run() {
        Helper h = new Helper();
        h.doWork();
        String s = h.getName();
    }
}
"#;
        let path = PathBuf::from("Caller.java");
        let (_, occurrences, _) = parse_java_file(&path, source);

        // Should have a CallSite for `new Helper()`
        let new_helper = occurrences
            .iter()
            .find(|o| o.name == "Helper" && matches!(o.kind, SymbolKind::CallSite));
        assert!(
            new_helper.is_some(),
            "Expected CallSite for new Helper(). All: {:?}",
            occurrences
                .iter()
                .map(|o| format!("{} {:?}", o.name, o.kind))
                .collect::<Vec<_>>()
        );

        // Should have CallSites for doWork and getName
        let call_sites: Vec<&str> = occurrences
            .iter()
            .filter(|o| matches!(o.kind, SymbolKind::CallSite))
            .map(|o| o.name.as_str())
            .collect();
        assert!(call_sites.contains(&"doWork"));
        assert!(call_sites.contains(&"getName"));
    }
}
