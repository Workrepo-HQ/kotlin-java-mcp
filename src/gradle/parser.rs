use super::{DependencyNode, GradleModule};

/// Parse the output of `gradlew projects -q`.
/// Lines look like:
/// ```
/// Root project 'my-project'
/// +--- Project ':app'
/// +--- Project ':core'
/// \--- Project ':feature'
/// ```
pub fn parse_projects_output(output: &str) -> Vec<GradleModule> {
    let mut modules = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        // Look for "Project ':path'" pattern
        if let Some(start) = trimmed.find("Project '") {
            let rest = &trimmed[start + 9..];
            if let Some(end) = rest.find('\'') {
                let path = &rest[..end];
                let name = path.rsplit(':').next().unwrap_or(path).to_string();
                if !name.is_empty() {
                    modules.push(GradleModule {
                        path: path.to_string(),
                        name,
                    });
                }
            }
        }
    }

    modules
}

/// Parse the output of `gradlew :module:dependencies --configuration compileClasspath -q`.
/// Lines look like:
/// ```
/// compileClasspath - Compile classpath for source set 'main'.
/// +--- org.jetbrains.kotlin:kotlin-stdlib:1.9.0
/// +--- com.google.code.gson:gson:2.10.1
/// |    \--- com.google.errorprone:error_prone_annotations:2.21.1
/// +--- project :core
/// \--- org.some:lib:1.0 -> 1.1 (*)
/// ```
pub fn parse_dependencies_output(output: &str) -> Vec<DependencyNode> {
    let lines: Vec<&str> = output.lines().collect();

    // Find the start of the dependency tree (after the configuration header)
    let start = lines
        .iter()
        .position(|l| {
            l.contains("compileClasspath")
                || l.trim().starts_with("+---")
                || l.trim().starts_with("\\---")
        })
        .unwrap_or(0);

    // Skip the header line if it's the configuration description
    let start = if lines.get(start).is_some_and(|l| l.contains("compileClasspath") && l.contains('-')) {
        start + 1
    } else {
        start
    };

    let dep_lines: Vec<&str> = lines[start..]
        .iter()
        .take_while(|l| {
            let t = l.trim();
            !t.is_empty()
                && (t.starts_with("+---")
                    || t.starts_with("\\---")
                    || t.starts_with("|")
                    || t.starts_with("+")
                    || t.starts_with("\\"))
        })
        .copied()
        .collect();

    parse_dep_tree(&dep_lines, 0).0
}

fn parse_dep_tree(lines: &[&str], base_indent: usize) -> (Vec<DependencyNode>, usize) {
    let mut nodes = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let indent = dependency_indent_level(line);

        if indent < base_indent && base_indent > 0 {
            break;
        }

        if indent == base_indent || (base_indent == 0 && nodes.is_empty()) {
            if let Some(mut node) = parse_dependency_line(line) {
                // Parse children at next indent level
                let (children, consumed) = parse_dep_tree(&lines[i + 1..], indent + 1);
                node.children = children;
                nodes.push(node);
                i += 1 + consumed;
                continue;
            }
        } else if indent > base_indent {
            // This is a child of the previous node - handled by recursion
            break;
        }

        i += 1;
    }

    (nodes, i)
}

fn dependency_indent_level(line: &str) -> usize {
    // Each indent level is represented by "| " or "  " (5 chars typically)
    // Count the number of tree drawing characters
    let mut level = 0;
    let chars: Vec<char> = line.chars().collect();
    let mut pos = 0;

    while pos < chars.len() {
        if chars[pos] == '|' || chars[pos] == ' ' {
            if pos + 4 < chars.len() {
                let chunk: String = chars[pos..pos + 5].iter().collect();
                if chunk == "|    " || chunk == "     " {
                    level += 1;
                    pos += 5;
                    continue;
                }
            }
        }
        break;
    }

    level
}

fn parse_dependency_line(line: &str) -> Option<DependencyNode> {
    // Strip tree characters to get the dependency spec
    let spec = line
        .trim_start_matches(|c: char| c == '|' || c == ' ' || c == '+' || c == '\\' || c == '-');
    let spec = spec.trim();

    if spec.is_empty() {
        return None;
    }

    let is_transitive_duplicate = spec.ends_with("(*)");
    let spec = spec.trim_end_matches("(*)").trim();

    // Project dependency
    if spec.starts_with("project ") {
        let project_path = spec
            .trim_start_matches("project ")
            .trim_matches(':')
            .trim();
        return Some(DependencyNode {
            group: "project".to_string(),
            artifact: project_path.to_string(),
            version: String::new(),
            resolved_version: None,
            is_project: true,
            is_transitive_duplicate,
            children: Vec::new(),
        });
    }

    // External dependency: group:artifact:version [-> resolved_version]
    let parts: Vec<&str> = spec.splitn(2, " -> ").collect();
    let base = parts[0];
    let resolved = parts.get(1).map(|s| s.trim().to_string());

    let segments: Vec<&str> = base.split(':').collect();
    if segments.len() >= 3 {
        Some(DependencyNode {
            group: segments[0].to_string(),
            artifact: segments[1].to_string(),
            version: segments[2].to_string(),
            resolved_version: resolved,
            is_project: false,
            is_transitive_duplicate,
            children: Vec::new(),
        })
    } else if segments.len() == 2 {
        Some(DependencyNode {
            group: segments[0].to_string(),
            artifact: segments[1].to_string(),
            version: String::new(),
            resolved_version: resolved,
            is_project: false,
            is_transitive_duplicate,
            children: Vec::new(),
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_projects() {
        let output = r#"
Root project 'my-project'
+--- Project ':app'
+--- Project ':core'
\--- Project ':feature'
"#;
        let modules = parse_projects_output(output);
        assert_eq!(modules.len(), 3);
        assert_eq!(modules[0].path, ":app");
        assert_eq!(modules[0].name, "app");
        assert_eq!(modules[1].path, ":core");
        assert_eq!(modules[2].path, ":feature");
    }

    #[test]
    fn test_parse_dependencies() {
        let output = r#"compileClasspath - Compile classpath for source set 'main'.
+--- org.jetbrains.kotlin:kotlin-stdlib:1.9.0
+--- com.google.code.gson:gson:2.10.1
+--- project :core
\--- org.some:lib:1.0 -> 1.1 (*)
"#;
        let deps = parse_dependencies_output(output);
        assert_eq!(deps.len(), 4);

        assert_eq!(deps[0].group, "org.jetbrains.kotlin");
        assert_eq!(deps[0].artifact, "kotlin-stdlib");
        assert_eq!(deps[0].version, "1.9.0");

        assert_eq!(deps[2].is_project, true);
        assert_eq!(deps[2].artifact, "core");

        assert_eq!(deps[3].resolved_version, Some("1.1".to_string()));
        assert!(deps[3].is_transitive_duplicate);
    }

    #[test]
    fn test_parse_empty_output() {
        let deps = parse_dependencies_output("");
        assert!(deps.is_empty());
    }
}
