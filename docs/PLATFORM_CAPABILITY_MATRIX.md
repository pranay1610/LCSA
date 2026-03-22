# Platform Capability Matrix

This matrix documents current `lcsa-core` runtime expectations for structural
signals. It reflects implementation state, not long-term intent.

## Signal Support

| Platform | Clipboard | Selection | Focus |
| --- | --- | --- | --- |
| Linux (X11 session) | supported | supported (primary selection) | supported |
| Linux (no `DISPLAY`) | supported | supported (primary selection) | unsupported (`requires_x11_display`) |
| macOS | supported | supported (best effort, accessibility) | supported (best effort, accessibility) |
| Windows | supported | supported (best effort, UI Automation) | supported (foreground process polling) |
| Android | unsupported (`platform_not_supported`) | unsupported (`backend_not_implemented`) | unsupported (`backend_not_implemented`) |
| iOS | unsupported (`platform_not_supported`) | unsupported (`backend_not_implemented`) | unsupported (`backend_not_implemented`) |

Notes:

- On macOS, focus/selection may return `unsupported (requires_accessibility_permission)` until automation/accessibility access is granted.
- On Linux, focus uses X11 and will return `unsupported (requires_x11_display)` in non-X11 sessions.
- On Windows/macOS, selection is a best-effort adapter and quality can vary by app toolkit.

## Runtime API

Use `ContextApi` runtime inspection instead of hardcoding assumptions:

- `ContextApi::signal_support(signal_type)` returns `SignalSupport`
- `ContextApi::is_signal_supported(signal_type)` returns a bool
- `ContextApi::supported_signals()` returns all currently supported signal types

Example:

```bash
cargo run -p lcsa-core --example runtime_capabilities
```
