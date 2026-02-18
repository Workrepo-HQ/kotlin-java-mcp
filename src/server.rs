use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::RwLock;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;
use tracing::info;

use crate::gradle::GradleRunner;
use crate::indexer::parser::index_files;
use crate::indexer::symbols::{cross_reference, register_companion_aliases};
use crate::indexer::SymbolIndex;

#[derive(Clone)]
pub struct KotlinMcpServer {
    project_root: PathBuf,
    index: Arc<RwLock<SymbolIndex>>,
    gradle_runner: Arc<GradleRunner>,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindUsagesParams {
    #[schemars(description = "The symbol name to search for (simple name or fully qualified name)")]
    pub symbol: String,
    #[schemars(description = "Optional file path where the symbol is used, for context")]
    pub file: Option<String>,
    #[schemars(description = "Optional line number where the symbol appears, for precise resolution")]
    pub line: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindDefinitionParams {
    #[schemars(description = "The symbol name to find the definition of (simple name or fully qualified name)")]
    pub symbol: String,
    #[schemars(description = "Optional file path where the symbol is referenced, for context")]
    pub file: Option<String>,
    #[schemars(description = "Optional line number where the symbol is referenced, for precise resolution")]
    pub line: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DependencyTreeParams {
    #[schemars(description = "Optional Gradle module path (e.g., ':app', ':core'). If omitted, lists all modules.")]
    pub module: Option<String>,
}

#[tool_router]
impl KotlinMcpServer {
    pub fn new(project_root: PathBuf) -> Self {
        let gradle_runner = Arc::new(GradleRunner::new(project_root.clone()));

        info!("Indexing Kotlin and Java files in {}", project_root.display());
        let mut index = index_files(&project_root);
        cross_reference(&mut index);
        register_companion_aliases(&mut index);
        info!("{}", index.stats());

        Self {
            project_root,
            index: Arc::new(RwLock::new(index)),
            gradle_runner,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Find all usages/references of a Kotlin or Java symbol across the project. Returns file locations, symbol kinds (call site, type reference, property reference, import), and fully qualified names. Use 'file' and 'line' parameters for precise resolution when the symbol name is ambiguous.")]
    async fn find_usages(
        &self,
        Parameters(params): Parameters<FindUsagesParams>,
    ) -> Result<CallToolResult, McpError> {
        let index = self.index.read();
        let file_path = params.file.as_ref().map(|f| {
            let p = PathBuf::from(f);
            if p.is_relative() {
                self.project_root.join(p)
            } else {
                p
            }
        });

        let results = crate::tools::find_usages::find_usages(
            &index,
            &params.symbol,
            file_path.as_deref(),
            params.line,
        );

        let output = crate::tools::format_occurrences(&results, &self.project_root);
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "Find the definition/declaration of a Kotlin or Java symbol. Returns the file location and declaration kind (class, interface, function, property, etc.). Use 'file' and 'line' parameters when calling from a specific reference location for precise resolution.")]
    async fn find_definition(
        &self,
        Parameters(params): Parameters<FindDefinitionParams>,
    ) -> Result<CallToolResult, McpError> {
        let index = self.index.read();
        let file_path = params.file.as_ref().map(|f| {
            let p = PathBuf::from(f);
            if p.is_relative() {
                self.project_root.join(p)
            } else {
                p
            }
        });

        let results = crate::tools::find_definition::find_definition(
            &index,
            &params.symbol,
            file_path.as_deref(),
            params.line,
        );

        let output = crate::tools::format_occurrences(&results, &self.project_root);
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "Show the Gradle module dependency tree. Without a module parameter, lists all project modules. With a module path (e.g., ':app'), shows the compile classpath dependencies including transitive dependencies, version conflicts, and project references.")]
    async fn dependency_tree(
        &self,
        Parameters(params): Parameters<DependencyTreeParams>,
    ) -> Result<CallToolResult, McpError> {
        match crate::tools::dependency_tree::dependency_tree(
            &self.gradle_runner,
            params.module.as_deref(),
        ) {
            Ok(output) => Ok(CallToolResult::success(vec![Content::text(output)])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "Gradle error: {}",
                e
            ))])),
        }
    }

    #[tool(description = "Re-index all Kotlin and Java files in the project. Use this after making changes to the codebase to update the symbol index. Also invalidates the Gradle cache.")]
    async fn reindex(&self) -> Result<CallToolResult, McpError> {
        info!("Re-indexing project at {}", self.project_root.display());

        let mut new_index = index_files(&self.project_root);
        cross_reference(&mut new_index);
        register_companion_aliases(&mut new_index);

        let stats = format!("{}", new_index.stats());
        info!("{}", stats);

        *self.index.write() = new_index;
        self.gradle_runner.invalidate_cache();

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Reindex complete. {}",
            stats
        ))]))
    }
}

#[tool_handler]
impl ServerHandler for KotlinMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "kotlin-java-mcp".to_string(),
                title: None,
                version: env!("CARGO_PKG_VERSION").to_string(),
                description: None,
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "Kotlin MCP server for code navigation. Indexes .kt and .java files using tree-sitter \
                 and provides find_usages, find_definition, dependency_tree, and reindex tools."
                    .to_string(),
            ),
        }
    }
}
