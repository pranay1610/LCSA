# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-03-22

### Added

#### lcsa-core
- Cross-platform clipboard monitoring (Linux, macOS, Windows)
- Linux primary-selection monitoring via X11
- Focus tracking (active window/app changes)
- Cross-device signal envelopes with device/app identity
- Structural signals: `ClipboardSignal`, `SelectionSignal`, `FocusSignal`
- Permission model for explicit clipboard content access
- Permission persistence to `~/.local/share/lcsa/permissions.json`
- Event-driven monitoring via XFixes on X11 (replaces polling)
- Shannon entropy-based sensitive text detection (reduces false positives)
- Single dispatcher thread architecture
- Graceful shutdown with SIGINT/SIGTERM handling (`run_with_signals`)
- Source app correlation for selection signals via focus tracking

#### lcsa-daemon
- Filesystem event normalization into semantic JSONL signals
- Directory scanning and continuous watching
- Entity classification (code, config, docs, data, media)

#### lcsa-mcp
- MCP server exposing LCSA via Model Context Protocol
- stdio transport (JSON-RPC 2.0)
- Resources: `lcsa://context/current`, `lcsa://signals/latest/{clipboard,selection,focus}`
- Tools: `get_supported_signals`, `get_current_context`, `get_clipboard_content`
- Integration with Claude Desktop, Cursor, and other MCP clients

### Platform Support

| Platform | Clipboard | Selection | Focus |
|----------|-----------|-----------|-------|
| Linux X11 | Event-driven | Event-driven | Event-driven |
| Linux Wayland | Polling | Polling | Polling |
| macOS | Polling | Accessibility | Polling |
| Windows | Polling | UI Automation | Polling |

[0.1.0]: https://github.com/pranay1610/LCSA/releases/tag/v0.1.0
