use std::path::PathBuf;
use std::process::Command;

fn binary_path() -> PathBuf {
    // cargo test builds the binary into the deps directory's parent
    let path = PathBuf::from(env!("CARGO_BIN_EXE_kotlin-java-mcp"));
    assert!(path.exists(), "Binary not found at {:?}", path);
    path
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-project")
}

fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(binary_path())
        .args(args)
        .output()
        .expect("Failed to execute binary")
}

// ── help ──────────────────────────────────────────────────────────────

#[test]
fn test_cli_help() {
    let output = run_cli(&["--help"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Expected success, got: {}", stdout);
    assert!(stdout.contains("find-usages"), "Help should list find-usages subcommand");
    assert!(stdout.contains("find-definition"), "Help should list find-definition subcommand");
    assert!(stdout.contains("serve"), "Help should list serve subcommand");
    assert!(stdout.contains("--project"), "Help should list --project flag");
}

#[test]
fn test_cli_find_usages_help() {
    let output = run_cli(&["find-usages", "--help"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("--file"), "find-usages help should list --file");
    assert!(stdout.contains("--line"), "find-usages help should list --line");
}

#[test]
fn test_cli_find_definition_help() {
    let output = run_cli(&["find-definition", "--help"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("--file"), "find-definition help should list --file");
    assert!(stdout.contains("--line"), "find-definition help should list --line");
}

// ── find-usages ───────────────────────────────────────────────────────

#[test]
fn test_cli_find_usages_simple_symbol() {
    let fixture = fixture_path();
    let output = run_cli(&["-p", fixture.to_str().unwrap(), "find-usages", "User"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout.contains("Found"), "Expected 'Found' header in output: {}", stdout);
    assert!(stdout.contains("result(s)"), "Expected result count in output");
    // User is referenced in multiple files
    assert!(stdout.contains("InMemoryUserRepository.kt"), "Expected usage in InMemoryUserRepository.kt");
    assert!(stdout.contains("UserService.kt"), "Expected usage in UserService.kt");
}

#[test]
fn test_cli_find_usages_by_fqn() {
    let fixture = fixture_path();
    let output = run_cli(&["-p", fixture.to_str().unwrap(), "find-usages", "com.example.core.User"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout.contains("Found"), "Expected results for FQN lookup: {}", stdout);
    assert!(stdout.contains("[com.example.core.User]"), "Expected FQN in output");
}

#[test]
fn test_cli_find_usages_nonexistent() {
    let fixture = fixture_path();
    let output = run_cli(&["-p", fixture.to_str().unwrap(), "find-usages", "DoesNotExist"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("No results found"), "Expected 'No results found' for nonexistent symbol: {}", stdout);
}

#[test]
fn test_cli_find_usages_with_file_and_line() {
    let fixture = fixture_path();
    let output = run_cli(&[
        "-p", fixture.to_str().unwrap(),
        "find-usages", "User",
        "--file", "core/src/main/kotlin/com/example/core/UserService.kt",
        "--line", "5",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    // With file+line context it should resolve to the FQN and return results
    assert!(stdout.contains("Found"), "Expected results with file/line context: {}", stdout);
}

// ── find-definition ───────────────────────────────────────────────────

#[test]
fn test_cli_find_definition_simple_symbol() {
    let fixture = fixture_path();
    let output = run_cli(&["-p", fixture.to_str().unwrap(), "find-definition", "User"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout.contains("Found"), "Expected results: {}", stdout);
    assert!(stdout.contains("User.kt"), "Expected definition in User.kt");
    assert!(stdout.contains("ClassDeclaration"), "Expected ClassDeclaration kind");
    assert!(stdout.contains("[com.example.core.User]"), "Expected FQN in output");
}

#[test]
fn test_cli_find_definition_by_fqn() {
    let fixture = fixture_path();
    let output = run_cli(&["-p", fixture.to_str().unwrap(), "find-definition", "com.example.core.UserService"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout.contains("Found"), "Expected results for FQN definition lookup: {}", stdout);
    assert!(stdout.contains("UserService.kt"), "Expected definition in UserService.kt");
    assert!(stdout.contains("ClassDeclaration"), "Expected ClassDeclaration kind");
}

#[test]
fn test_cli_find_definition_interface() {
    let fixture = fixture_path();
    let output = run_cli(&["-p", fixture.to_str().unwrap(), "find-definition", "Repository"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout.contains("Repository.kt"), "Expected definition in Repository.kt");
    assert!(stdout.contains("InterfaceDeclaration"), "Expected InterfaceDeclaration kind");
}

#[test]
fn test_cli_find_definition_nonexistent() {
    let fixture = fixture_path();
    let output = run_cli(&["-p", fixture.to_str().unwrap(), "find-definition", "DoesNotExist"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("No results found"), "Expected 'No results found': {}", stdout);
}

#[test]
fn test_cli_find_definition_with_file_and_line() {
    let fixture = fixture_path();
    let output = run_cli(&[
        "-p", fixture.to_str().unwrap(),
        "find-definition", "User",
        "--file", "app/src/main/kotlin/com/example/app/InMemoryUserRepository.kt",
        "--line", "7",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout.contains("Found"), "Expected results with file/line context: {}", stdout);
    assert!(stdout.contains("User.kt"), "Expected definition in User.kt");
}

// ── indexing output on stderr ─────────────────────────────────────────

#[test]
fn test_cli_indexing_progress_on_stderr() {
    let fixture = fixture_path();
    let output = run_cli(&["-p", fixture.to_str().unwrap(), "find-definition", "User"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success());
    assert!(stderr.contains("Indexing Kotlin and Java files"), "Expected indexing progress on stderr: {}", stderr);
    assert!(stderr.contains("Indexed"), "Expected index stats on stderr: {}", stderr);
}

// ── error cases ───────────────────────────────────────────────────────

#[test]
fn test_cli_invalid_project_path() {
    let output = run_cli(&["-p", "/nonexistent/path/that/does/not/exist", "find-usages", "User"]);

    assert!(!output.status.success(), "Expected failure for invalid project path");
}

#[test]
fn test_cli_missing_symbol_argument() {
    let fixture = fixture_path();
    let output = run_cli(&["-p", fixture.to_str().unwrap(), "find-usages"]);

    assert!(!output.status.success(), "Expected failure when symbol argument is missing");
}
