//! LCSA MCP Server
//!
//! Exposes LCSA local context signals via the Model Context Protocol (MCP).
//! This allows AI tools like Claude, Cursor, etc. to access clipboard,
//! selection, and focus signals from the local system.

mod error;
mod protocol;
mod resources;
pub mod server;
mod tools;

pub use error::Error;
pub use server::{ContextSnapshot, McpServer};
