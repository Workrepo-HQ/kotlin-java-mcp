/// Scope tracking for Kotlin source files.
/// Uses byte ranges from tree-sitter nodes to determine which scope a symbol belongs to.

#[derive(Debug, Clone)]
pub struct ScopeSegment {
    pub name: String,
    pub byte_range: std::ops::Range<usize>,
}

#[derive(Debug, Default)]
pub struct ScopeTree {
    segments: Vec<ScopeSegment>,
}

impl ScopeTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_scope(&mut self, name: String, byte_range: std::ops::Range<usize>) {
        self.segments.push(ScopeSegment { name, byte_range });
    }

    /// Sort segments by start position for binary search.
    pub fn finalize(&mut self) {
        self.segments.sort_by_key(|s| s.byte_range.start);
    }

    /// Find the scope chain (outermost to innermost) for a given byte offset.
    /// Returns a list of scope names that contain the given position.
    pub fn scope_chain_at(&self, byte_offset: usize) -> Vec<&str> {
        let mut chain: Vec<&ScopeSegment> = self
            .segments
            .iter()
            .filter(|s| s.byte_range.start <= byte_offset && byte_offset < s.byte_range.end)
            .collect();
        // Sort by range size (largest first = outermost first)
        chain.sort_by_key(|s| std::cmp::Reverse(s.byte_range.end - s.byte_range.start));
        chain.iter().map(|s| s.name.as_str()).collect()
    }

    /// Build the FQN prefix from package and scope chain at a byte offset.
    pub fn fqn_prefix_at(&self, package: Option<&str>, byte_offset: usize) -> String {
        let mut parts = Vec::new();
        if let Some(pkg) = package {
            parts.push(pkg.to_string());
        }
        for scope in self.scope_chain_at(byte_offset) {
            parts.push(scope.to_string());
        }
        parts.join(".")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_chain() {
        let mut tree = ScopeTree::new();
        // Simulate: class Outer { class Inner { fun method() {} } }
        tree.add_scope("Outer".into(), 0..100);
        tree.add_scope("Inner".into(), 20..80);
        tree.finalize();

        // Inside Inner
        let chain = tree.scope_chain_at(50);
        assert_eq!(chain, vec!["Outer", "Inner"]);

        // Inside Outer but outside Inner
        let chain = tree.scope_chain_at(10);
        assert_eq!(chain, vec!["Outer"]);

        // Outside everything
        let chain = tree.scope_chain_at(150);
        assert!(chain.is_empty());
    }

    #[test]
    fn test_fqn_prefix() {
        let mut tree = ScopeTree::new();
        tree.add_scope("MyClass".into(), 0..100);
        tree.finalize();

        let fqn = tree.fqn_prefix_at(Some("com.example"), 50);
        assert_eq!(fqn, "com.example.MyClass");

        let fqn = tree.fqn_prefix_at(None, 50);
        assert_eq!(fqn, "MyClass");
    }
}
