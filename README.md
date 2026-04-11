# LCSA

[![lcsa-core on crates.io](https://img.shields.io/crates/v/lcsa-core.svg)](https://crates.io/crates/lcsa-core)
[![lcsa-daemon on crates.io](https://img.shields.io/crates/v/lcsa-daemon.svg)](https://crates.io/crates/lcsa-daemon)
[![lcsa-mcp on crates.io](https://img.shields.io/crates/v/lcsa-mcp.svg)](https://crates.io/crates/lcsa-mcp)

LCSA is a local context substrate for AI-native software.

It turns low-level local signals into stable, typed context that applications,
agents, and automations can react to without polling everything or scraping raw
desktop state.

The repo now has three complementary parts:

- `lcsa-core`: the embedded Rust library
- `lcsa-daemon`: an optional JSONL adapter for filesystem semantics
- `lcsa-mcp`: MCP server exposing LCSA to AI tools (Claude, Cursor, etc.)

## Current Signal Model

LCSA is intentionally layered:

- Layer 1: raw platform events
- Layer 2: structural signals such as `ClipboardSignal`
- Layer 3: semantic signals such as `code.updated`

What is implemented today:

- Linux, macOS, and Windows clipboard monitoring via `ContextApi`
- Linux primary-selection monitoring via `ContextApi`
- Linux X11 focus monitoring via `ContextApi` when `DISPLAY` is available
- macOS focus monitoring via `ContextApi` (frontmost app polling)
- Windows focus monitoring via `ContextApi` (foreground process polling)
- macOS selection monitoring via accessibility polling (best effort)
- Windows selection monitoring via UI Automation polling (best effort)
- Cross-device signal envelopes with device/app identity
- Shared envelope model for clipboard, selection, and focus structural signals
- Explicit permission model for clipboard content access
- Filesystem event normalization into semantic signals
- Runnable example and CLI adapter
- Unit tests and daemon behavior tests

## Why The Project Matters

Most local AI tools only see snapshots. LCSA gives them change-aware context.

The core assumption behind the project is sound:

- OS primitives exist, but they are too raw and inconsistent for AI apps.
- App builders should not have to re-implement clipboard, focus, selection, and
  filesystem plumbing every time.
- A useful substrate must separate safe structural signals from explicit content
  access, otherwise every integration becomes either too weak or too risky.

That helps others build:

- editor copilots that react when code or config changes
- secure assistants that see clipboard metadata without exposing raw content
- local automation tools that trigger on intent instead of file churn
- knowledge capture systems that summarize meaningful events instead of full logs
- assistive interfaces that respond to context shifts with lower latency

## Quick Start

Run the embedded clipboard example:

```bash
cargo run -p lcsa-core --example clipboard_monitor
```

Run the cross-device envelope example:

```bash
cargo run -p lcsa-core --example enveloped_clipboard_monitor
```

Probe runtime signal support:

```bash
cargo run -p lcsa-core --example runtime_capabilities
```

Inspect mobile policy mapping across Android/iOS version bands:

```bash
cargo run -p lcsa-core --example mobile_policy_matrix
```

Run a live terminal dashboard:

```bash
cargo run -p lcsa-core --example context_live_view
```

Run an assistant adapter demo (context packet + prompt augmentation):

```bash
cargo run -p lcsa-core --example assistant_context_adapter
```

That example now shows:

- structural clipboard events
- command and sensitivity heuristics
- explicit permission-gated content preview
- how an app can stamp signals with device/app identity

Scan a directory into semantic JSONL:

```bash
cargo run -p lcsa-daemon -- scan .
```

Watch a directory continuously:

```bash
cargo run -p lcsa-daemon -- watch . --initial-scan
```

Run the test suite:

```bash
cargo test
```

## MCP Server

Run the MCP server for AI tool integration:

```bash
cargo run -p lcsa-mcp
```

Install from crates.io:

```bash
cargo install lcsa-mcp
```

Use the included wrapper from this repo (handy for MCP client configs):

```bash
./lcsa-mcp-wrapper
```

Configure Claude Desktop (`~/.config/claude/mcp_servers.json`):

```json
{
  "lcsa": {
    "command": "/absolute/path/to/lcsa-mcp-wrapper"
  }
}
```

See `lcsa-mcp/README.md` for full documentation.

## Example Filesystem Output

```json
{
  "version": "0.1",
  "occurred_at": "2026-03-22T10:12:05Z",
  "source": "filesystem",
  "action": "updated",
  "entity_kind": "code",
  "summary": "Code file updated: src/main.rs",
  "confidence": 0.98,
  "paths": ["src/main.rs"],
  "tags": ["ext:rs", "topdir:src"],
  "metadata": {
    "primary_path": "src/main.rs",
    "extension": "rs"
  }
}
```

## Architecture

The architecture summary lives in `docs/ARCHITECTURE.md`.
Mobile planning details live in `docs/MOBILE_PLATFORM_STRATEGY.md`.
Platform capability matrix lives in `docs/PLATFORM_CAPABILITY_MATRIX.md`.
Live demo walkthrough lives in `docs/VISUAL_GUIDE.md`.

The next strong extensions are:

- selection quality improvements on macOS and Windows accessibility adapters
- terminal command summaries
- browser tab or download events
- TypeScript bindings

## Mobile Version Policy

LCSA should treat mobile as a capability matrix, not a single backend:

- Support window target: latest major release and previous two major releases.
- Android policy: app-local and foreground-friendly by default; avoid daemon-style assumptions.
- iOS policy: user-intent and foreground interaction are first-class constraints.
- Legacy versions remain opt-in compatibility paths, not default behavior.
