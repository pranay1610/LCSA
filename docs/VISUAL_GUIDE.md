# Visual Guide

LCSA can be observed as a live substrate from the terminal without writing any
integration code.

Run:

```bash
cargo run -p lcsa-core --example context_live_view
```

You should see:

```text
LCSA Live View
device=devbox platform=Linux

Signal Capabilities
  Clipboard  Supported
  Selection  Supported
  Focus      Supported

Recent Context Events
  - 1711104069 focus target=Browser source=firefox
  - 1711104067 selection type=Text bytes=52 source=firefox
  - 1711104063 clipboard type=Text bytes=16 source=unknown
```

How to trigger visible changes:

- Copy text or images to trigger clipboard events.
- Select text on Linux/macOS/Windows to trigger selection events.
- Switch active applications or windows to trigger focus events.

## Assistant Adapter Demo

To see a real integration shape (not just raw event logs), run:

```bash
cargo run -p lcsa-core --example assistant_context_adapter
```

What this demonstrates:

- Live subscriptions to clipboard, selection, and focus signals.
- Capability-aware behavior (it only subscribes where backend support exists).
- Prompt augmentation: each task you type is enriched with recent local context.
- Explicit gated content access via `:grant-content` and `:revoke-content`.

Useful commands:

- `:context` prints the current context packet.
- `:grant-content` enables clipboard content preview for the session.
- `:revoke-content` disables clipboard content preview.
- `:quit` exits.
