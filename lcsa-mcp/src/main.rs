use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging to stderr (MCP uses stdout for protocol)
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tracing::info!("Starting LCSA MCP server");

    let mut server = lcsa_mcp::McpServer::new()?;
    server.run_stdio()?;

    Ok(())
}
