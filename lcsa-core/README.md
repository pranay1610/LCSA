# lcsa-core

Core library for LCSA (Local Context Substrate API). Provides typed signals for clipboard, selection, and focus events with privacy-preserving defaults.

## Features

- Cross-platform clipboard monitoring (Linux, macOS, Windows)
- Selection monitoring (X11 primary selection, macOS/Windows accessibility)
- Focus tracking (active window/app changes)
- Permission-gated content access
- Event-driven architecture with XFixes on X11
- Structural signals with metadata (no raw content by default)

## Installation

```bash
cargo add lcsa-core
```

## Quick Start

```rust
use lcsa_core::{ContextApi, SignalType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut api = ContextApi::new()?;

    // Subscribe to clipboard changes (metadata only)
    api.subscribe(SignalType::Clipboard, |signal| {
        println!(
            "clipboard: type={:?}, bytes={}, source={}",
            signal.content_type,
            signal.size_bytes,
            signal.source_app
        );
    })?;

    // Run event loop with graceful shutdown
    api.run_with_signals()?;
    Ok(())
}
```

## Permission Model

Raw clipboard content requires explicit permission:

```rust
use lcsa_core::{Capability, PermissionRequest, Scope};

api.request_permission(PermissionRequest::new(
    Capability::ReadClipboardContent,
    Scope::Session,
    "Display clipboard preview to user",
))?;

if api.can_access(Capability::ReadClipboardContent) {
    let content = api.read_clipboard_content()?;
    println!("preview: {}", content.redacted_preview());
}
```

## Signal Types

| Signal | Description |
|--------|-------------|
| `ClipboardSignal` | Content type, size, source app, sensitivity flags |
| `SelectionSignal` | Selected text metadata, editability |
| `FocusSignal` | Active window/app changes |

## Platform Support

| Platform | Clipboard | Selection | Focus |
|----------|-----------|-----------|-------|
| Linux X11 | Event-driven | Event-driven | Event-driven |
| Linux Wayland | Polling | Polling | Polling |
| macOS | Polling | Accessibility | Polling |
| Windows | Polling | UI Automation | Polling |

## License

Apache-2.0
