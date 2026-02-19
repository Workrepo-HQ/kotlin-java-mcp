# Changelog

## v0.0.1

Initial release of kotlin-java-mcp — an MCP server and CLI tool that indexes Kotlin and Java codebases using tree-sitter.

### Features

- **MCP server** with stdio transport (rmcp) exposing `find_usages`, `find_definition`, `dependency_tree`, and `reindex` tools
- **CLI mode** with `find-usages` and `find-definition` subcommands (optional `--file`/`--line` context)
- **Kotlin indexing** via tree-sitter-kotlin-ng with scope-based FQN resolution, import resolution, companion object aliasing, and type alias following
- **Java indexing** via tree-sitter-java for cross-language find-usages and find-definition
- **Lombok support** — synthesized getters/setters for `@Data`, `@Getter`, `@Setter` (class-level and field-level), with boolean `isXxx` prefix handling and final-field awareness
- **Gradle integration** for dependency tree parsing
- **Parallel file parsing** via rayon
- **Import-context filtering** for Lombok name-based lookups to reduce false positives
- **`--include-imports` flag** to optionally include import statements in usage results (excluded by default, matching IntelliJ behavior)
- **`--version` flag** via clap

