use std::path::PathBuf;

use kotlin_java_mcp::indexer::parser::index_files;
use kotlin_java_mcp::indexer::symbols::{cross_reference, register_companion_aliases};
use kotlin_java_mcp::indexer::SymbolKind;
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
fn test_find_usages_of_user_class() {
    let index = build_index();
    let results = find_usages(&index, "User", None, None);

    // User is used in many places: imports, type references, call sites
    assert!(!results.is_empty(), "Expected usages of User, found none");

    // Should find usages across multiple files
    let files: std::collections::HashSet<&str> = results
        .iter()
        .map(|o| o.file.file_name().unwrap().to_str().unwrap())
        .collect();
    assert!(
        files.len() > 1,
        "Expected User usages in multiple files, found in: {:?}",
        files
    );
}

#[test]
fn test_find_usages_of_user_service() {
    let index = build_index();
    let results = find_usages(&index, "UserService", None, None);

    assert!(!results.is_empty(), "Expected usages of UserService");

    // UserService is imported/used in Application.kt, Config.kt, UserProfile.kt
    let files: Vec<&str> = results
        .iter()
        .map(|o| o.file.file_name().unwrap().to_str().unwrap())
        .collect();
    assert!(
        files.iter().any(|f| *f == "Application.kt" || *f == "Config.kt" || *f == "UserProfile.kt"),
        "Expected UserService usage in app/feature modules, found in: {:?}",
        files
    );
}

#[test]
fn test_find_usages_of_repository_interface() {
    let index = build_index();
    let results = find_usages(&index, "Repository", None, None);

    assert!(
        !results.is_empty(),
        "Expected usages of Repository interface"
    );

    // Repository is used in UserService.kt and InMemoryUserRepository.kt
    let files: Vec<&str> = results
        .iter()
        .map(|o| o.file.file_name().unwrap().to_str().unwrap())
        .collect();
    assert!(
        files.iter().any(|f| *f == "InMemoryUserRepository.kt"),
        "Expected Repository usage in InMemoryUserRepository, found in: {:?}",
        files
    );
}

#[test]
fn test_find_usages_of_user_role_enum() {
    let index = build_index();
    let results = find_usages(&index, "UserRole", None, None);

    assert!(!results.is_empty(), "Expected usages of UserRole");
}

#[test]
fn test_find_usages_by_fqn() {
    let index = build_index();
    let results = find_usages(&index, "com.example.core.User", None, None);

    assert!(
        !results.is_empty(),
        "Expected usages when searching by FQN"
    );
}

#[test]
fn test_find_usages_nonexistent_symbol() {
    let index = build_index();
    let results = find_usages(&index, "NonExistentSymbol", None, None);

    assert!(results.is_empty(), "Expected no usages for nonexistent symbol");
}

#[test]
fn test_find_usages_import_has_correct_line_number() {
    let index = build_index();
    let results = find_usages(&index, "worksheetWorkflowConfig", None, None);

    let imports: Vec<_> = results
        .iter()
        .filter(|o| matches!(o.kind, SymbolKind::Import))
        .collect();
    assert!(!imports.is_empty(), "Expected at least one import of worksheetWorkflowConfig");

    for imp in &imports {
        assert!(
            imp.line > 0,
            "Import line should be > 0 (actual position), got line {}",
            imp.line
        );
    }
}

#[test]
fn test_find_usages_bare_identifier_reference() {
    // worksheetWorkflowConfig is used as a bare value reference (not a call, not a type)
    // in expressions like `listOf(worksheetWorkflowConfig)` and `val config = worksheetWorkflowConfig`
    let index = build_index();
    let results = find_usages(&index, "worksheetWorkflowConfig", None, None);

    // Should have at least the import + two value references in WorkflowUsage.kt
    let in_workflow_usage: Vec<_> = results
        .iter()
        .filter(|o| {
            o.file.file_name().unwrap().to_str().unwrap() == "WorkflowUsage.kt"
                && !matches!(o.kind, SymbolKind::Import)
        })
        .collect();

    assert!(
        in_workflow_usage.len() >= 2,
        "Expected at least 2 non-import usages of worksheetWorkflowConfig in WorkflowUsage.kt, found {}: {:?}",
        in_workflow_usage.len(),
        in_workflow_usage.iter().map(|o| format!("{}:{} {:?}", o.file.display(), o.line, o.kind)).collect::<Vec<_>>()
    );
}

// Helper: collect all non-import, non-declaration usages in a specific file
fn usages_in_file<'a>(
    results: &'a [&'a kotlin_java_mcp::indexer::SymbolOccurrence],
    filename: &str,
) -> Vec<&'a kotlin_java_mcp::indexer::SymbolOccurrence> {
    results
        .iter()
        .filter(|o| {
            o.file.file_name().unwrap().to_str().unwrap() == filename
                && !matches!(o.kind, SymbolKind::Import)
                && !o.kind.is_declaration()
        })
        .copied()
        .collect()
}

// --- Navigation receivers ---

#[test]
fn test_find_usages_navigation_receiver_property_access() {
    // `Config.maxRetries` — Config is the receiver of a navigation_expression
    let index = build_index();
    let results = find_usages(&index, "Config", None, None);

    let in_patterns = usages_in_file(&results, "ReferencePatterns.kt");

    // Config is used as receiver on two lines: Config.maxRetries and Config.getTimeout()
    assert!(
        in_patterns.len() >= 2,
        "Expected at least 2 usages of Config as receiver in ReferencePatterns.kt, found {}: {:?}",
        in_patterns.len(),
        in_patterns.iter().map(|o| format!("line:{} {:?}", o.line, o.kind)).collect::<Vec<_>>()
    );
}

// --- Callable references ---

#[test]
fn test_find_usages_callable_reference_bare() {
    // `::createUser` — callable reference to a top-level function
    let index = build_index();
    let results = find_usages(&index, "createUser", None, None);

    let in_patterns = usages_in_file(&results, "ReferencePatterns.kt");

    // ::createUser appears twice: `val factory = ::createUser` and `.map(::createUser)`
    assert!(
        in_patterns.len() >= 2,
        "Expected at least 2 callable reference usages of createUser in ReferencePatterns.kt, found {}: {:?}",
        in_patterns.len(),
        in_patterns.iter().map(|o| format!("line:{} {:?}", o.line, o.kind)).collect::<Vec<_>>()
    );
}

#[test]
fn test_find_usages_callable_reference_qualified() {
    // `User::toString` — qualified callable reference, User is the receiver
    let index = build_index();
    let results = find_usages(&index, "User", None, None);

    let in_patterns = usages_in_file(&results, "ReferencePatterns.kt");

    // User appears as: import, type ref in ::toString, and in callable ref `User::toString`
    assert!(
        !in_patterns.is_empty(),
        "Expected User referenced in ReferencePatterns.kt (from User::toString), found none"
    );
}

// --- Infix function calls ---

#[test]
fn test_find_usages_infix_function() {
    // `"key" mapTo "value"` — infix call to mapTo
    let index = build_index();
    let results = find_usages(&index, "mapTo", None, None);

    let in_patterns = usages_in_file(&results, "ReferencePatterns.kt");

    assert!(
        !in_patterns.is_empty(),
        "Expected mapTo infix usage in ReferencePatterns.kt, found none. All results: {:?}",
        results.iter().map(|o| format!("{}:{} {:?} {}", o.file.file_name().unwrap().to_str().unwrap(), o.line, o.kind, o.name)).collect::<Vec<_>>()
    );
}

// --- Cross-reference FQN resolution ---

#[test]
fn test_find_usages_fqn_not_shadowed_by_class_method() {
    // Regression: when a file contains a class with method `generateReport` AND a top-level
    // call to the same-named top-level function `com.example.core.generateReport`,
    // cross_reference should NOT override the correct FQN with the class method's FQN.
    let index = build_index();

    // Search by FQN for the top-level function
    let results = find_usages(&index, "com.example.core.generateReport", None, None);

    // The call `generateReport("test")` in useTopLevel() in ReportServiceImpl.kt
    // should resolve to the top-level function, not to ReportServiceImpl.generateReport
    let in_impl: Vec<_> = results
        .iter()
        .filter(|o| {
            o.file.file_name().unwrap().to_str().unwrap() == "ReportServiceImpl.kt"
                && !matches!(o.kind, SymbolKind::Import)
                && !o.kind.is_declaration()
        })
        .collect();

    assert!(
        !in_impl.is_empty(),
        "Expected a reference to top-level generateReport in ReportServiceImpl.kt, but FQN search missed it. \
         This likely means cross_reference incorrectly reassigned the FQN to the class method. \
         All results: {:?}",
        results.iter().map(|o| format!("{}:{} {:?} fqn={:?}", o.file.file_name().unwrap().to_str().unwrap(), o.line, o.kind, o.fqn)).collect::<Vec<_>>()
    );
}
