mod error;
mod gradle;
mod indexer;
mod server;
mod tools;

use std::path::PathBuf;

use clap::Parser;
use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "kotlin-java-mcp", about = "MCP server for Kotlin code navigation")]
struct Args {
    /// Root directory of the Kotlin project to index
    #[arg(short, long, default_value = ".")]
    project: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Set up tracing to stderr (stdout is used for MCP stdio transport)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_ansi(false)
        .init();

    let args = Args::parse();
    let project_root = args.project.canonicalize()?;

    tracing::info!("Starting kotlin-java-mcp server for {}", project_root.display());

    let server = server::KotlinMcpServer::new(project_root);
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;

    Ok(())
}
