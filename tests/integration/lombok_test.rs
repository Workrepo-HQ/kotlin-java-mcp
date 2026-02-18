use std::path::PathBuf;

use kotlin_java_mcp::indexer::parser::index_files;
use kotlin_java_mcp::indexer::symbols::{cross_reference, register_companion_aliases};
use kotlin_java_mcp::indexer::SymbolKind;
use kotlin_java_mcp::tools::find_definition::find_definition;
use kotlin_java_mcp::tools::find_usages::find_usages;

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
fn test_lombok_find_definition_of_getter() {
    let index = build_index();

    // find-definition of getUsername should resolve to a declaration in LombokUser.java
    let results = find_definition(&index, "getUsername", None, None);

    assert!(
        !results.is_empty(),
        "Expected definition for getUsername (synthesized by @Data)"
    );

    let decl = &results[0];
    assert_eq!(
        decl.file.file_name().unwrap().to_str().unwrap(),
        "LombokUser.java"
    );
    assert!(matches!(decl.kind, SymbolKind::FunctionDeclaration));
    assert_eq!(
        decl.fqn.as_deref(),
        Some("com.example.core.LombokUser.getUsername")
    );
}

#[test]
fn test_lombok_find_definition_of_boolean_getter() {
    let index = build_index();

    // boolean field → isActive getter
    let results = find_definition(&index, "isActive", None, None);

    assert!(
        !results.is_empty(),
        "Expected definition for isActive (synthesized by @Data for boolean field)"
    );

    let decl = &results[0];
    assert_eq!(
        decl.file.file_name().unwrap().to_str().unwrap(),
        "LombokUser.java"
    );
}

#[test]
fn test_lombok_find_definition_of_setter() {
    let index = build_index();

    let results = find_definition(&index, "setUsername", None, None);

    assert!(
        !results.is_empty(),
        "Expected definition for setUsername (synthesized by @Data)"
    );

    let decl = &results[0];
    assert_eq!(
        decl.file.file_name().unwrap().to_str().unwrap(),
        "LombokUser.java"
    );
}

#[test]
fn test_lombok_no_setter_for_final_field() {
    let index = build_index();

    // The `id` field is `final`, so @Data should NOT generate setId
    let results = find_definition(&index, "setId", None, None);

    // Filter to LombokUser.java only (setId might exist elsewhere)
    let in_lombok_user: Vec<_> = results
        .iter()
        .filter(|o| o.file.file_name().unwrap().to_str().unwrap() == "LombokUser.java")
        .collect();

    assert!(
        in_lombok_user.is_empty(),
        "Should NOT have setId for final field in LombokUser.java, but found: {:?}",
        in_lombok_user
            .iter()
            .map(|o| format!("{:?} {}", o.kind, o.fqn.as_deref().unwrap_or("")))
            .collect::<Vec<_>>()
    );

    // But getId SHOULD exist
    let getter_results = find_definition(&index, "getId", None, None);
    let getter_in_lombok = getter_results
        .iter()
        .find(|o| o.file.file_name().unwrap().to_str().unwrap() == "LombokUser.java");
    assert!(
        getter_in_lombok.is_some(),
        "Expected getId getter for final field"
    );
}

#[test]
fn test_lombok_find_usages_of_field_includes_getter_calls() {
    let index = build_index();

    // find-usages of the `username` field should include calls to getUsername() and setUsername()
    let results = find_usages(
        &index,
        "com.example.core.LombokUser.username",
        None,
        None,
        false,
    );

    // Should find getter/setter call sites in LombokConsumer.java
    let in_consumer: Vec<_> = results
        .iter()
        .filter(|o| o.file.file_name().unwrap().to_str().unwrap() == "LombokConsumer.java")
        .collect();

    assert!(
        !in_consumer.is_empty(),
        "Expected usages of username field (via getUsername/setUsername) in LombokConsumer.java, found none. All results: {:?}",
        results.iter().map(|o| format!("{}:{} {} {:?}", o.file.file_name().unwrap().to_str().unwrap(), o.line, o.name, o.kind)).collect::<Vec<_>>()
    );
}

#[test]
fn test_lombok_accessor_mappings_in_index() {
    let index = build_index();

    // Check that lombok_accessors contains the username field mapping
    let username_fqn = "com.example.core.LombokUser.username";
    assert!(
        index.lombok_accessors.contains_key(username_fqn),
        "Expected lombok_accessors to contain {}, keys: {:?}",
        username_fqn,
        index
            .lombok_accessors
            .keys()
            .filter(|k| k.contains("LombokUser"))
            .collect::<Vec<_>>()
    );

    let accessors = &index.lombok_accessors[username_fqn];
    assert!(
        accessors.contains(&"com.example.core.LombokUser.getUsername".to_string()),
        "Expected getUsername in accessors, got: {:?}",
        accessors
    );
    assert!(
        accessors.contains(&"com.example.core.LombokUser.setUsername".to_string()),
        "Expected setUsername in accessors, got: {:?}",
        accessors
    );
}
