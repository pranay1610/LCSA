use std::time::Duration;

use lcsa_core::{
    ApplicationContext, Capability, ContextApi, DeviceClass, DeviceContext, PermissionRequest,
    Platform, Scope, SignalType,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut api = ContextApi::builder()
        .device_context(DeviceContext {
            device_id: "desktop:devbox".to_string(),
            device_name: "devbox".to_string(),
            platform: Platform::Linux,
            device_class: DeviceClass::Desktop,
            os_version: None,
        })
        .application_context(ApplicationContext {
            app_id: "lcsa-demo".to_string(),
            app_name: "lcsa-demo".to_string(),
            app_version: Some("0.1.0".to_string()),
        })
        .build()?;

    api.request_permission(
        PermissionRequest::new(
            Capability::ReadClipboardContent,
            Scope::Session,
            "Demonstrate cross-device envelopes plus gated content access",
        )
        .with_ttl(Duration::from_secs(300)),
    )?;

    api.subscribe_enveloped(SignalType::Clipboard, |envelope| {
        println!(
            "envelope={} device={} app={} source={:?}",
            envelope.signal_id,
            envelope.device.device_id,
            envelope.application.app_id,
            envelope.source
        );
    })?;

    println!("Monitoring enveloped clipboard signals. Press Ctrl+C to exit.");
    api.run_with_signals()?;
    Ok(())
}
