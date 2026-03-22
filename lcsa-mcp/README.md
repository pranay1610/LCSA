# lcsa-mcp

MCP (Model Context Protocol) server for LCSA. Exposes local context signals to AI tools like Claude, Cursor, Windsurf, and others.

## Installation

```bash
cargo install lcsa-mcp
```

Or build from source:

```bash
git clone https://github.com/pranay1610/LCSA
cd LCSA
cargo build --release -p lcsa-mcp
```

## Usage

Run the server (stdio transport):

```bash
lcsa-mcp
```

Or run via wrapper (from repo root):

```bash
./lcsa-mcp-wrapper
```

Wrapper behavior:

- Uses `lcsa-mcp` from `PATH` by default
- Supports pinning with `LCSA_MCP_BIN=/absolute/path/to/lcsa-mcp`
- Sets `RUST_LOG=warn` by default
- Enables debug logs with `LCSA_MCP_DEBUG=1`

### Configure Claude Desktop

Add to `~/.config/claude/mcp_servers.json`:

```json
{
  "lcsa": {
    "command": "/absolute/path/to/lcsa-mcp-wrapper"
  }
}
```

Or direct binary:

```json
{
  "lcsa": {
    "command": "/path/to/lcsa-mcp"
  }
}
```

### Configure Cursor

Add to your MCP configuration:

```json
{
  "mcpServers": {
    "lcsa": {
      "command": "/absolute/path/to/lcsa-mcp-wrapper"
    }
  }
}
```

## MCP Resources

| URI | Description |
|-----|-------------|
| `lcsa://context/current` | Snapshot of all latest signals |
| `lcsa://signals/latest/clipboard` | Latest clipboard signal (metadata) |
| `lcsa://signals/latest/selection` | Latest selection signal |
| `lcsa://signals/latest/focus` | Latest focus signal |

## MCP Tools

| Tool | Description |
|------|-------------|
| `get_supported_signals` | Query which signal types are supported on this platform |
| `get_current_context` | Get a snapshot of all latest signals |
| `get_clipboard_content` | Get raw clipboard content (requires permission and reason) |

## Example Tool Responses

### get_current_context

```json
{
  "device": {
    "id": "abc123",
    "name": "workstation",
    "platform": "linux"
  },
  "application": {
    "id": "lcsa-mcp",
    "name": "lcsa-mcp"
  },
  "supported_signals": ["clipboard", "selection", "focus"],
  "latest_signals": {
    "clipboard": {
      "content_type": "text",
      "size_bytes": 42,
      "source_app": "firefox",
      "likely_sensitive": false,
      "likely_command": false
    }
  }
}
```

### get_clipboard_content

```json
{
  "preview": "Hello w...",
  "source_app": "firefox",
  "payload": "Hello world"
}
```

## Privacy

LCSA separates signal metadata from content. The `get_current_context` tool returns only metadata (content type, size, source app, sensitivity flags). Raw clipboard content requires explicit permission via `get_clipboard_content` with a stated reason.

## License

Apache-2.0
