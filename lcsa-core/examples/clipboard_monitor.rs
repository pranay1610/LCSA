use std::time::Duration;

use lcsa_core::{Capability, ContextApi, PermissionRequest, Scope, SignalType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut api = ContextApi::new()?;

    api.subscribe(SignalType::Clipboard, |signal| {
        println!(
            "clipboard changed: type={:?}, bytes={}, source={}, sensitive={}, command={}",
            signal.content_type,
            signal.size_bytes,
            signal.source_app,
            signal.likely_sensitive,
            signal.likely_command
        );
    })?;

    api.request_permission(
        PermissionRequest::new(
            Capability::ReadClipboardContent,
            Scope::Session,
            "Show how explicit content access works in the example",
        )
        .with_ttl(Duration::from_secs(300)),
    )?;

    if let Ok(content) = api.read_clipboard_content() {
        println!("initial clipboard preview: {}", content.redacted_preview());
    }

    println!("Monitoring clipboard changes. Press Ctrl+C to exit.");
    api.run_with_signals()?;
    Ok(())
}
