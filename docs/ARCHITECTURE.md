# LCSA Architecture

## Vision

LCSA is a local context substrate for AI-native software. It takes noisy,
platform-specific events and turns them into stable signals that applications,
agents, and automations can subscribe to without scraping the entire desktop.

## Implemented Shape

- `lcsa-core` is the embedded library.
- `lcsa-daemon` is an optional adapter that exposes filesystem signals as JSONL.
- The core design is layered:
  - Layer 1: raw platform events such as clipboard changes and filesystem notifications
  - Layer 2: structural signals such as `ClipboardSignal`
  - Layer 3: semantic signals such as `code.updated` and `document.created`
- Real-world runtime identity is modeled explicitly:
  - `DeviceContext` identifies the device and OS
  - `ApplicationContext` identifies the calling app
  - `SignalEnvelope` packages signals for multi-device or multi-app routing

## Why This Architecture

- Embedded-first keeps latency low and adoption simple.
- Structural signals let apps respond to context without immediately handling raw content.
- Semantic normalization gives downstream AI systems a compact, action-oriented view of change.
- The daemon remains useful as a bridge for shell tools, local pipelines, and observability.

## What Exists Today

- Clipboard monitor for Linux, macOS, and Windows in `lcsa-core`
- Linux primary-selection monitor in `lcsa-core`
- Linux X11 focus monitor in `lcsa-core` (requires `DISPLAY`)
- macOS focus monitor in `lcsa-core`
- Windows focus monitor in `lcsa-core`
- macOS selection monitor (accessibility polling, best effort)
- Windows selection monitor (UI Automation polling, best effort)
- Runtime signal support introspection via `ContextApi::signal_support`
- Cross-device signal envelope types in `lcsa-core`
- Shared structural signal model for clipboard, selection, and focus envelopes
- Permission model that keeps structural signals available while gating raw clipboard content
- Filesystem semantic normalizer in `lcsa-core`
- Filesystem scan/watch adapter in `lcsa-daemon`
- Unit tests and daemon behavior tests
- Runnable examples

## What Comes Next

- Permissions and redaction policies
- More signal sources: terminal activity
- selection quality improvements on macOS and Windows accessibility adapters
- Optional TypeScript bindings
- Optional coordinator process for multi-app sharing

## Mobile Direction

LCSA keeps one contract and multiple capability profiles:

- Mobile support is version-banded and policy-aware, not desktop-equivalent.
- The default target window is latest major release plus two previous major releases.
- Android and iOS should be treated as app-local/foreground-first environments.
- Platform divergence stays in backend adapters and policy classification, not in app code.

See `docs/MOBILE_PLATFORM_STRATEGY.md` for concrete version-band behavior.
See `docs/PLATFORM_CAPABILITY_MATRIX.md` for runtime support expectations.
See `docs/VISUAL_GUIDE.md` for a live terminal view of emitted context.
