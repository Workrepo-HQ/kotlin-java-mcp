use crate::gradle::{DependencyNode, GradleRunner};

/// Get the dependency tree for a module, formatted as text.
pub fn dependency_tree(
    runner: &GradleRunner,
    module: Option<&str>,
) -> Result<String, crate::error::GradleError> {
    let mut output = String::new();

    if let Some(module) = module {
        // Get dependencies for a specific module
        let deps = runner.get_dependencies(module)?;
        output.push_str(&format!("Dependencies for module '{}':\n\n", module));
        for dep in &deps {
            format_dep_node(&mut output, dep, 0);
        }
    } else {
        // List all modules
        let modules = runner.get_modules()?;
        output.push_str(&format!(
            "Project modules ({} total):\n\n",
            modules.len()
        ));
        for m in &modules {
            output.push_str(&format!("  {} ({})\n", m.path, m.name));
        }
    }

    Ok(output)
}

fn format_dep_node(output: &mut String, node: &DependencyNode, depth: usize) {
    let indent = "  ".repeat(depth);
    let prefix = if depth == 0 { "" } else { "├── " };

    if node.is_project {
        output.push_str(&format!("{}{}project :{}\n", indent, prefix, node.artifact));
    } else {
        let version_display = if let Some(ref resolved) = node.resolved_version {
            format!("{} -> {}", node.version, resolved)
        } else {
            node.version.clone()
        };

        let dup_marker = if node.is_transitive_duplicate {
            " (*)"
        } else {
            ""
        };

        output.push_str(&format!(
            "{}{}{}:{}:{}{}",
            indent, prefix, node.group, node.artifact, version_display, dup_marker
        ));
        output.push('\n');
    }

    for child in &node.children {
        format_dep_node(output, child, depth + 1);
    }
}
