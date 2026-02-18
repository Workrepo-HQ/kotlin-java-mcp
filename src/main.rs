use std::path::PathBuf;

use clap::{Parser, Subcommand};
use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "kotlin-java-mcp", about = "Kotlin code navigation — MCP server and CLI")]
struct Args {
    /// Root directory of the Kotlin project to index
    #[arg(short, long, default_value = ".")]
    project: PathBuf,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Start the MCP server (stdio transport) — this is the default when no subcommand is given
    Serve,

    /// Find all usages/references of a symbol
    FindUsages {
        /// Symbol name (simple or fully-qualified)
        symbol: String,

        /// Optional file path for context-based resolution
        #[arg(short, long)]
        file: Option<String>,

        /// Optional line number for precise resolution
        #[arg(short, long)]
        line: Option<usize>,
    },

    /// Find the definition/declaration of a symbol
    FindDefinition {
        /// Symbol name (simple or fully-qualified)
        symbol: String,

        /// Optional file path for context-based resolution
        #[arg(short, long)]
        file: Option<String>,

        /// Optional line number for precise resolution
        #[arg(short, long)]
        line: Option<usize>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let project_root = args.project.canonicalize()?;

    match args.command {
        None | Some(Command::Serve) => run_server(project_root).await,
        Some(Command::FindUsages { symbol, file, line }) => {
            init_cli_tracing();
            run_find_usages(project_root, &symbol, file.as_deref(), line)
        }
        Some(Command::FindDefinition { symbol, file, line }) => {
            init_cli_tracing();
            run_find_definition(project_root, &symbol, file.as_deref(), line)
        }
    }
}

fn init_cli_tracing() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_ansi(true)
        .init();
}

async fn run_server(project_root: PathBuf) -> anyhow::Result<()> {
    // MCP server logs to stderr, protocol uses stdout
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_ansi(false)
        .init();

    tracing::info!("Starting kotlin-java-mcp server for {}", project_root.display());

    let server = kotlin_java_mcp::server::KotlinMcpServer::new(project_root);
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;

    Ok(())
}

fn run_find_usages(
    project_root: PathBuf,
    symbol: &str,
    file: Option<&str>,
    line: Option<usize>,
) -> anyhow::Result<()> {
    let index = build_index(&project_root);

    let file_path = file.map(|f| {
        let p = PathBuf::from(f);
        if p.is_relative() {
            project_root.join(p)
        } else {
            p
        }
    });

    let results =
        kotlin_java_mcp::tools::find_usages::find_usages(&index, symbol, file_path.as_deref(), line);

    let output = kotlin_java_mcp::tools::format_occurrences(&results, &project_root);
    println!("{}", output);
    Ok(())
}

fn run_find_definition(
    project_root: PathBuf,
    symbol: &str,
    file: Option<&str>,
    line: Option<usize>,
) -> anyhow::Result<()> {
    let index = build_index(&project_root);

    let file_path = file.map(|f| {
        let p = PathBuf::from(f);
        if p.is_relative() {
            project_root.join(p)
        } else {
            p
        }
    });

    let results = kotlin_java_mcp::tools::find_definition::find_definition(
        &index,
        symbol,
        file_path.as_deref(),
        line,
    );

    let output = kotlin_java_mcp::tools::format_occurrences(&results, &project_root);
    println!("{}", output);
    Ok(())
}

fn build_index(project_root: &PathBuf) -> kotlin_java_mcp::indexer::SymbolIndex {
    use kotlin_java_mcp::indexer::parser::index_files;
    use kotlin_java_mcp::indexer::symbols::{cross_reference, register_companion_aliases};

    eprintln!("Indexing Kotlin files in {} ...", project_root.display());
    let mut index = index_files(project_root);
    cross_reference(&mut index);
    register_companion_aliases(&mut index);
    eprintln!("{}", index.stats());
    index
}
