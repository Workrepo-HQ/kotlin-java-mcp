use std::path::PathBuf;

use kotlin_java_mcp::indexer::parser::index_files;
use kotlin_java_mcp::indexer::symbols::{cross_reference, register_companion_aliases};
use kotlin_java_mcp::indexer::SymbolKind;
use kotlin_java_mcp::tools::find_definition::find_definition;

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
fn test_find_definition_of_user_class() {
    let index = build_index();
    let results = find_definition(&index, "User", None, None);

    assert!(!results.is_empty(), "Expected definition of User");

    // Should be a class declaration in User.kt
    let user_decl = results
        .iter()
        .find(|o| matches!(o.kind, SymbolKind::ClassDeclaration))
        .expect("Expected a ClassDeclaration for User");

    assert_eq!(
        user_decl.file.file_name().unwrap().to_str().unwrap(),
        "User.kt"
    );
    assert_eq!(user_decl.fqn.as_deref(), Some("com.example.core.User"));
}

#[test]
fn test_find_definition_of_repository_interface() {
    let index = build_index();
    let results = find_definition(&index, "Repository", None, None);

    assert!(!results.is_empty(), "Expected definition of Repository");

    let iface_decl = results
        .iter()
        .find(|o| matches!(o.kind, SymbolKind::InterfaceDeclaration))
        .expect("Expected an InterfaceDeclaration for Repository");

    assert_eq!(
        iface_decl.file.file_name().unwrap().to_str().unwrap(),
        "Repository.kt"
    );
}

#[test]
fn test_find_definition_of_user_service() {
    let index = build_index();
    let results = find_definition(&index, "UserService", None, None);

    assert!(!results.is_empty(), "Expected definition of UserService");

    let class_decl = results
        .iter()
        .find(|o| matches!(o.kind, SymbolKind::ClassDeclaration))
        .expect("Expected a ClassDeclaration for UserService");

    assert_eq!(
        class_decl.file.file_name().unwrap().to_str().unwrap(),
        "UserService.kt"
    );
    assert_eq!(
        class_decl.fqn.as_deref(),
        Some("com.example.core.UserService")
    );
}

#[test]
fn test_find_definition_of_user_role_enum() {
    let index = build_index();
    let results = find_definition(&index, "UserRole", None, None);

    assert!(!results.is_empty(), "Expected definition of UserRole");

    let decl = &results[0];
    assert_eq!(
        decl.file.file_name().unwrap().to_str().unwrap(),
        "User.kt"
    );
}

#[test]
fn test_find_definition_of_config_object() {
    let index = build_index();
    let results = find_definition(&index, "Config", None, None);

    assert!(!results.is_empty(), "Expected definition of Config");

    let obj_decl = results
        .iter()
        .find(|o| matches!(o.kind, SymbolKind::ObjectDeclaration))
        .expect("Expected an ObjectDeclaration for Config");

    assert_eq!(
        obj_decl.file.file_name().unwrap().to_str().unwrap(),
        "Config.kt"
    );
}

#[test]
fn test_find_definition_of_extension_function() {
    let index = build_index();
    let results = find_definition(&index, "displayName", None, None);

    assert!(!results.is_empty(), "Expected definition of displayName");

    let ext_decl = results
        .iter()
        .find(|o| matches!(o.kind, SymbolKind::ExtensionFunctionDeclaration | SymbolKind::FunctionDeclaration))
        .expect("Expected a function declaration for displayName");

    assert_eq!(
        ext_decl.file.file_name().unwrap().to_str().unwrap(),
        "Extensions.kt"
    );
}

#[test]
fn test_find_definition_by_fqn() {
    let index = build_index();
    let results = find_definition(&index, "com.example.core.User", None, None);

    assert!(
        !results.is_empty(),
        "Expected definition when searching by FQN"
    );
    assert!(results[0].kind.is_declaration());
}

#[test]
fn test_find_definition_of_function() {
    let index = build_index();
    let results = find_definition(&index, "getUser", None, None);

    assert!(!results.is_empty(), "Expected definition of getUser");

    let func_decl = results
        .iter()
        .find(|o| matches!(o.kind, SymbolKind::FunctionDeclaration))
        .expect("Expected a FunctionDeclaration for getUser");

    assert_eq!(
        func_decl.file.file_name().unwrap().to_str().unwrap(),
        "UserService.kt"
    );
}

#[test]
fn test_find_definition_nonexistent() {
    let index = build_index();
    let results = find_definition(&index, "DoesNotExist", None, None);
    assert!(results.is_empty());
}
