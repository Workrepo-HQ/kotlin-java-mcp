use kotlin_java_mcp::gradle::parser::{parse_dependencies_output, parse_projects_output};
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/gradle")
        .join(name)
}

#[test]
fn test_parse_projects_fixture() {
    let content = std::fs::read_to_string(fixture_path("projects_output.txt")).unwrap();
    let modules = parse_projects_output(&content);

    assert_eq!(modules.len(), 3);

    let names: Vec<&str> = modules.iter().map(|m| m.name.as_str()).collect();
    assert!(names.contains(&"app"));
    assert!(names.contains(&"core"));
    assert!(names.contains(&"feature"));

    let paths: Vec<&str> = modules.iter().map(|m| m.path.as_str()).collect();
    assert!(paths.contains(&":app"));
    assert!(paths.contains(&":core"));
    assert!(paths.contains(&":feature"));
}

#[test]
fn test_parse_dependencies_fixture() {
    let content = std::fs::read_to_string(fixture_path("dependencies_output.txt")).unwrap();
    let deps = parse_dependencies_output(&content);

    assert!(
        !deps.is_empty(),
        "Expected dependencies to be parsed from fixture"
    );

    // Check that kotlin-stdlib is found
    let kotlin_stdlib = deps
        .iter()
        .find(|d| d.artifact == "kotlin-stdlib")
        .expect("Expected kotlin-stdlib dependency");
    assert_eq!(kotlin_stdlib.group, "org.jetbrains.kotlin");
    assert_eq!(kotlin_stdlib.version, "1.9.22");

    // Check project dependency
    let project_core = deps
        .iter()
        .find(|d| d.is_project && d.artifact == "core")
        .expect("Expected project :core dependency");
    assert!(project_core.is_project);

    // Check gson
    let gson = deps
        .iter()
        .find(|d| d.artifact == "gson")
        .expect("Expected gson dependency");
    assert_eq!(gson.group, "com.google.code.gson");
    assert_eq!(gson.version, "2.10.1");
}

#[test]
fn test_parse_dependencies_version_conflict() {
    let content = std::fs::read_to_string(fixture_path("dependencies_output.txt")).unwrap();
    let deps = parse_dependencies_output(&content);

    // The okhttp dep should be present
    let okhttp = deps
        .iter()
        .find(|d| d.artifact == "okhttp");
    assert!(okhttp.is_some(), "Expected okhttp dependency");
}

#[test]
fn test_parse_dependencies_transitive() {
    let content = std::fs::read_to_string(fixture_path("dependencies_output.txt")).unwrap();
    let deps = parse_dependencies_output(&content);

    // kotlin-stdlib has transitive children (kotlin-stdlib-common, annotations)
    let kotlin_stdlib = deps
        .iter()
        .find(|d| d.artifact == "kotlin-stdlib" && !d.is_transitive_duplicate)
        .expect("Expected non-duplicate kotlin-stdlib");
    assert!(
        !kotlin_stdlib.children.is_empty(),
        "Expected kotlin-stdlib to have transitive dependencies, got: {:?}",
        kotlin_stdlib.children
    );
}
