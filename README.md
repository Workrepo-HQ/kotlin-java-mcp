# kotlin-java-mcp

A fast MCP (Model Context Protocol) server that helps AI assistants understand Kotlin codebases. Built in Rust with tree-sitter parsing.

## What it does

kotlin-java-mcp indexes your Kotlin project and exposes tools over MCP that answer questions like "what uses this piece of code?" — designed to support large-scale refactorings where understanding the blast radius of a change matters.

### Tools

| Tool | Description |
|------|-------------|
| `find_usages` | Find all references to a symbol across the project. Handles qualified names, imports, extension functions, companion objects, and type aliases. |
| `find_definition` | Find where a symbol is declared. Resolves through imports to the actual source location. |
| `dependency_tree` | Show the Gradle module dependency graph and external library dependencies. |
| `reindex` | Re-scan all Kotlin files after changes. |

## How it works

1. On startup, walks the project and parses every `.kt` file in parallel using tree-sitter
2. Builds a full cross-reference index: symbol names → declarations, usages, imports, type references
3. Resolves fully qualified names using package declarations, imports (explicit, wildcard, aliased), and scope nesting
4. Serves tools over MCP stdio transport for use with Claude Code or other MCP clients

### Kotlin-specific handling

- **Extension functions**: Tracks receiver types, resolves `"hello".capitalize()` to the correct declaration
- **Companion objects**: Members accessible via both `MyClass.Companion.create()` and `MyClass.create()`
- **Type aliases**: Follows alias chains during symbol resolution
- **Sealed classes**: Correct FQN construction for nested variants
- **Scoping**: Handles nested classes, objects, and functions with byte-range-based scope lookup

## Usage

```bash
# Build
cargo build --release

# Run against a Kotlin project
./target/release/kotlin-java-mcp --project-root /path/to/your/kotlin-project
```

### Claude Code configuration

Add to your Claude Code MCP settings:

```json
{
  "mcpServers": {
    "kotlin": {
      "command": "/path/to/kotlin-java-mcp",
      "args": ["--project-root", "/path/to/your/kotlin-project"]
    }
  }
}
```

## Tech stack

- **Rust** with tokio async runtime
- **[rmcp](https://crates.io/crates/rmcp)** — official Rust MCP SDK
- **[tree-sitter](https://tree-sitter.github.io/)** with [tree-sitter-kotlin-ng](https://github.com/tree-sitter-grammars/tree-sitter-kotlin) grammar
- **rayon** for parallel file parsing
- **clap** for CLI argument parsing
- **walkdir** for file discovery

## Development

```bash
cargo test          # Run all tests
cargo clippy        # Lint
cargo build         # Debug build
```
