use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("LCSA error: {0}")]
    Lcsa(#[from] lcsa_core::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    #[error("Unknown tool: {0}")]
    UnknownTool(String),
}
