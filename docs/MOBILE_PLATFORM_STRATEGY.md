# Mobile Platform Strategy

## Goal

Keep one `ContextApi` contract while adapting implementation to what each mobile
platform version safely allows.

## Release Window Policy

For mobile, plan and test against:

- latest major release
- previous major release
- two major releases back

Older versions can be supported when needed, but should be explicit and opt-in.

## Android Version Bands

### Android 10 and newer

- Clipboard reads are generally foreground-app or IME scoped.
- LCSA backend model should be foreground-friendly and app-local by default.
- Long-running background daemon assumptions are not a safe default.

### Android 9 and older (legacy)

- Clipboard behavior is more permissive, but weaker from a privacy posture.
- Treat as a compatibility path, not the default product posture.

## iOS Version Bands

### iOS 16 and newer

- Clipboard flows are user-intent gated more aggressively.
- LCSA should prefer user-triggered or app-local context collection patterns.

### iOS 15 and older (legacy)

- Clipboard integration is less consistently user-intent gated.
- Keep support conservative and app-scoped to preserve a stable privacy model.

## Signal-Level Expectations on Mobile

Across Android and iOS, mobile defaults should be:

- `clipboard`: foreground or user-intent oriented
- `selection`: app-local only
- `focus`: app-local only

This is intentionally different from desktop environments, where system-wide
observation can be feasible.

## Engineering Implications for LCSA

- Keep policy in one place (`lcsa-core::mobile_policy`) so app integrations do
  not hardcode OS assumptions.
- Add mobile examples that demonstrate graceful degradation by platform policy.
- Gate unsupported collection modes with explicit `UnsupportedSignal` outcomes.

Use the policy example to validate expected version-band behavior:

```bash
cargo run -p lcsa-core --example mobile_policy_matrix
```
