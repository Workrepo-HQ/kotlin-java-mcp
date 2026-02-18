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

1. On startup, walks the project and parses every `.kt` and `.java` file in parallel using tree-sitter
2. Builds a full cross-reference index: symbol names → declarations, usages, imports, type references
3. Resolves fully qualified names using package declarations, imports (explicit, wildcard, aliased), and scope nesting
4. Serves tools over MCP stdio transport for use with Claude Code or other MCP clients

### Kotlin-specific handling

- **Extension functions**: Tracks receiver types, resolves `"hello".capitalize()` to the correct declaration
- **Companion objects**: Members accessible via both `MyClass.Companion.create()` and `MyClass.create()`
- **Type aliases**: Follows alias chains during symbol resolution
- **Sealed classes**: Correct FQN construction for nested variants
- **Scoping**: Handles nested classes, objects, and functions with byte-range-based scope lookup

### Java-specific handling

- **Lombok support**: `@Data`, `@Getter`, `@Setter` (class-level and field-level) — synthesizes getter/setter declarations, so `find-definition getName` resolves to the field and `find-usages fieldName` includes getter/setter call sites
- **Records**: Indexed as declarations with correct FQNs
- **Annotations**: Annotation type declarations are tracked

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

## Known limitations

This tool uses tree-sitter for syntactic analysis only — it does not have a type inference engine. This means symbol resolution relies on import analysis and name matching rather than resolved types.

**What works well:**
- Resolving symbols through explicit imports, wildcard imports, and same-package declarations
- Cross-language references (Kotlin ↔ Java) when imports are present
- Lombok field usages in files that import the containing class

**Where it falls short — member access without import context:**

When code accesses a member through a chain like `ctx.getService().getConfig().fieldName`, resolving `fieldName` to the correct class requires knowing the return type of each method in the chain. This is type inference, which has escalating levels of complexity:

1. **Local variable type tracking** — parse `val x: Foo = ...` or `Foo x = ...` to know `x` is `Foo`, then resolve `x.field` → `Foo.field`. Moderate effort (~300-400 lines), but only handles the simple single-hop case.

2. **Method return type tracking** — index return types on method declarations so `x.getConfig()` can be resolved if `getConfig()` has a declared return type. Significant effort (~500+ lines on top of level 1). Generics (`<T> T getParam(Class<T>)`) make this dramatically harder since it requires generic type substitution.

3. **Full type inference** — lambda receivers (Kotlin's `apply { this is X }`), smart casts, generic resolution, overload resolution. This is building a compiler frontend — thousands of lines and months of work. At that point, embedding the Kotlin/Java compiler APIs directly would be more practical.

The current approach uses import-based filtering as a proxy for type information: if a file doesn't import class `Foo`, references to `fieldName` in that file are unlikely to be `Foo.fieldName`. This eliminates most false positives without requiring type inference.

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
