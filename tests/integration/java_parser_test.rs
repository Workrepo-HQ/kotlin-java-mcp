use std::path::PathBuf;

use kotlin_java_mcp::indexer::parser::index_files;
use kotlin_java_mcp::indexer::symbols::{cross_reference, register_companion_aliases};
use kotlin_java_mcp::indexer::SymbolKind;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-project")
}

fn build_index() -> kotlin_java_mcp::indexer::SymbolIndex {
    let root = fixture_path();
    let mut index = index_files(&root);
    cross_reference(&mut index);
    register_companion_aliases(&mut index);
    index
}

#[test]
fn test_java_file_discovered_and_indexed() {
    let index = build_index();

    // JavaHelper.java should be indexed
    let has_java_helper = index
        .files
        .keys()
        .any(|p| p.file_name().unwrap().to_str().unwrap() == "JavaHelper.java");
    assert!(
        has_java_helper,
        "Expected JavaHelper.java to be indexed. Files: {:?}",
        index
            .files
            .keys()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_java_class_declaration_fqn() {
    let index = build_index();

    let java_helper_decls: Vec<_> = index
        .by_name
        .get("JavaHelper")
        .unwrap()
        .iter()
        .filter(|o| o.kind.is_declaration())
        .collect();

    assert!(
        !java_helper_decls.is_empty(),
        "Expected JavaHelper declaration"
    );
    assert_eq!(
        java_helper_decls[0].fqn.as_deref(),
        Some("com.example.core.JavaHelper")
    );
}

#[test]
fn test_java_method_declarations() {
    let index = build_index();

    // Check methods inside JavaHelper
    let create_user: Vec<_> = index
        .by_name
        .get("createUser")
        .unwrap()
        .iter()
        .filter(|o| {
            o.kind.is_declaration()
                && o.file
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .ends_with(".java")
        })
        .collect();

    assert!(
        !create_user.is_empty(),
        "Expected createUser method declaration in Java file"
    );
    assert_eq!(
        create_user[0].fqn.as_deref(),
        Some("com.example.core.JavaHelper.createUser")
    );
}

#[test]
fn test_java_field_declarations() {
    let index = build_index();

    let empty = vec![];
    let prefix_decls: Vec<_> = index
        .by_name
        .get("prefix")
        .unwrap_or(&empty)
        .iter()
        .filter(|o| {
            matches!(o.kind, SymbolKind::PropertyDeclaration)
                && o.file
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    == "JavaHelper.java"
        })
        .collect();

    assert!(
        !prefix_decls.is_empty(),
        "Expected 'prefix' field declaration in JavaHelper.java"
    );
}

#[test]
fn test_java_constructor_declaration() {
    let index = build_index();

    let ctor: Vec<_> = index
        .by_name
        .get("JavaHelper")
        .unwrap()
        .iter()
        .filter(|o| matches!(o.kind, SymbolKind::ConstructorDeclaration))
        .collect();

    assert!(
        !ctor.is_empty(),
        "Expected constructor declaration for JavaHelper"
    );
}

#[test]
fn test_java_references_kotlin_type() {
    // JavaHelper.java references User (a Kotlin class) as a type and constructor
    let index = build_index();

    let user_refs: Vec<_> = index
        .by_name
        .get("User")
        .unwrap()
        .iter()
        .filter(|o| {
            o.file
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                == "JavaHelper.java"
                && !o.kind.is_declaration()
                && !matches!(o.kind, SymbolKind::Import)
        })
        .collect();

    assert!(
        !user_refs.is_empty(),
        "Expected User references in JavaHelper.java (cross-language reference)"
    );
}

#[test]
fn test_java_imports_indexed() {
    let index = build_index();

    let file_info = index
        .files
        .iter()
        .find(|(p, _)| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                == "JavaHelper.java"
        })
        .map(|(_, info)| info)
        .expect("Expected JavaHelper.java in index");

    assert_eq!(file_info.package, Some("com.example.core".to_string()));
    assert!(
        file_info.imports.len() >= 2,
        "Expected at least 2 imports in JavaHelper.java, got {}",
        file_info.imports.len()
    );

    let import_paths: Vec<&str> = file_info.imports.iter().map(|i| i.path.as_str()).collect();
    assert!(import_paths.contains(&"java.util.List"));
    assert!(import_paths.contains(&"java.util.ArrayList"));
}
