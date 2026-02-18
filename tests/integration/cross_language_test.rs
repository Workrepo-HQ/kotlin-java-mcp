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
fn test_find_usages_of_user_includes_java_files() {
    let index = build_index();
    let results = find_usages(&index, "User", None, None, true);

    // User should be referenced in both .kt and .java files
    let java_refs: Vec<_> = results
        .iter()
        .filter(|o| {
            o.file
                .extension()
                .is_some_and(|ext| ext == "java")
        })
        .collect();

    assert!(
        !java_refs.is_empty(),
        "Expected User usages in Java files, but found none. Files with User: {:?}",
        results
            .iter()
            .map(|o| o.file.file_name().unwrap().to_str().unwrap())
            .collect::<Vec<_>>()
    );

    let kt_refs: Vec<_> = results
        .iter()
        .filter(|o| {
            o.file
                .extension()
                .is_some_and(|ext| ext == "kt")
        })
        .collect();

    assert!(
        !kt_refs.is_empty(),
        "Expected User usages in Kotlin files too"
    );
}

#[test]
fn test_find_definition_of_java_class_from_kotlin() {
    let index = build_index();

    // JavaHelper is defined in Java, used in Kotlin (JavaUsage.kt)
    let results = find_definition(&index, "JavaHelper", None, None);

    assert!(
        !results.is_empty(),
        "Expected to find definition of JavaHelper"
    );

    let def = &results[0];
    assert!(
        def.file
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            == "JavaHelper.java",
        "Expected definition in JavaHelper.java, got {:?}",
        def.file
    );
    assert!(def.kind.is_declaration());
}

#[test]
fn test_find_usages_of_java_helper_includes_kotlin_files() {
    let index = build_index();
    let results = find_usages(&index, "JavaHelper", None, None, true);

    // JavaHelper should be referenced from JavaUsage.kt
    let kt_refs: Vec<_> = results
        .iter()
        .filter(|o| {
            o.file
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                == "JavaUsage.kt"
                && !matches!(o.kind, SymbolKind::Import)
                && !o.kind.is_declaration()
        })
        .collect();

    assert!(
        !kt_refs.is_empty(),
        "Expected JavaHelper usage in JavaUsage.kt. All results: {:?}",
        results
            .iter()
            .map(|o| format!(
                "{}:{} {:?}",
                o.file.file_name().unwrap().to_str().unwrap(),
                o.line,
                o.kind
            ))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_find_usages_by_fqn_java_class() {
    let index = build_index();
    let results = find_usages(&index, "com.example.core.JavaHelper", None, None, true);

    assert!(
        !results.is_empty(),
        "Expected usages when searching Java class by FQN"
    );
}

#[test]
fn test_cross_language_fqn_resolution() {
    // User is declared in Kotlin with FQN com.example.core.User
    // JavaHelper.java imports and uses User — its references should resolve to the same FQN
    let index = build_index();

    let user_in_java: Vec<_> = index
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
                && o.kind.is_reference()
        })
        .collect();

    assert!(
        !user_in_java.is_empty(),
        "Expected User references in JavaHelper.java"
    );

    // At least some should have the correct FQN
    let with_fqn: Vec<_> = user_in_java
        .iter()
        .filter(|o| o.fqn.as_deref() == Some("com.example.core.User"))
        .collect();

    assert!(
        !with_fqn.is_empty(),
        "Expected User references in JavaHelper.java to have FQN com.example.core.User, got: {:?}",
        user_in_java
            .iter()
            .map(|o| format!("{:?} fqn={:?}", o.kind, o.fqn))
            .collect::<Vec<_>>()
    );
}
