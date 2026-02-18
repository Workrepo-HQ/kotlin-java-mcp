use thiserror::Error;

#[derive(Error, Debug)]
pub enum KotlinMcpError {
    #[error("Indexing error: {0}")]
    IndexError(String),

    #[error("Tree-sitter parse error for file: {0}")]
    ParseError(String),

    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("Gradle error: {0}")]
    GradleError(#[from] GradleError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum GradleError {
    #[error("Gradle wrapper not found at: {0}")]
    WrapperNotFound(String),

    #[error("Gradle command failed: {0}")]
    CommandFailed(String),

    #[error("Failed to parse Gradle output: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
