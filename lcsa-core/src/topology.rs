use std::env;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::signals::StructuralSignal;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Linux,
    MacOs,
    Windows,
    Ios,
    Android,
    Browser,
    Unknown,
}

impl Platform {
    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Linux => "linux",
            Platform::MacOs => "macos",
            Platform::Windows => "windows",
            Platform::Ios => "ios",
            Platform::Android => "android",
            Platform::Browser => "browser",
            Platform::Unknown => "unknown",
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceClass {
    Desktop,
    Laptop,
    Tablet,
    Phone,
    Server,
    Browser,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceContext {
    pub device_id: String,
    pub device_name: String,
    pub platform: Platform,
    pub device_class: DeviceClass,
    pub os_version: Option<String>,
}

impl Default for DeviceContext {
    fn default() -> Self {
        let platform = current_platform();
        let device_name = env::var("HOSTNAME")
            .or_else(|_| env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "local-device".to_string());
        let platform_label = platform.as_str();

        Self {
            device_id: format!("{platform_label}:{device_name}"),
            device_name,
            platform,
            device_class: DeviceClass::Desktop,
            os_version: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationContext {
    pub app_id: String,
    pub app_name: String,
    pub app_version: Option<String>,
}

impl Default for ApplicationContext {
    fn default() -> Self {
        Self {
            app_id: "lcsa-client".to_string(),
            app_name: "lcsa-client".to_string(),
            app_version: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalSource {
    Clipboard,
    Filesystem,
    Selection,
    Focus,
    Terminal,
    Browser,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignalEnvelope {
    pub signal_id: String,
    pub emitted_at: SystemTime,
    pub source: SignalSource,
    pub device: DeviceContext,
    pub application: ApplicationContext,
    pub payload: StructuralSignal,
}

impl SignalEnvelope {
    pub fn new(
        source: SignalSource,
        device: DeviceContext,
        application: ApplicationContext,
        payload: StructuralSignal,
    ) -> Self {
        let micros = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_micros())
            .unwrap_or_default();

        Self {
            signal_id: format!("{source:?}-{micros}").to_ascii_lowercase(),
            emitted_at: SystemTime::now(),
            source,
            device,
            application,
            payload,
        }
    }

    pub fn from_signal(
        device: DeviceContext,
        application: ApplicationContext,
        payload: StructuralSignal,
    ) -> Self {
        let source = payload.source();
        Self::new(source, device, application, payload)
    }
}

fn current_platform() -> Platform {
    #[cfg(target_os = "linux")]
    {
        Platform::Linux
    }
    #[cfg(target_os = "macos")]
    {
        Platform::MacOs
    }
    #[cfg(target_os = "windows")]
    {
        Platform::Windows
    }
    #[cfg(target_os = "ios")]
    {
        Platform::Ios
    }
    #[cfg(target_os = "android")]
    {
        Platform::Android
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "windows",
        target_os = "ios",
        target_os = "android"
    )))]
    {
        Platform::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signals::{ClipboardSignal, StructuralSignal};

    #[test]
    fn default_device_context_has_platform() {
        let device = DeviceContext::default();
        assert_ne!(device.platform, Platform::Unknown);
    }

    #[test]
    fn envelope_wraps_payload_with_identity() {
        let envelope = SignalEnvelope::from_signal(
            DeviceContext::default(),
            ApplicationContext::default(),
            StructuralSignal::Clipboard(ClipboardSignal::text(
                "cargo test",
                "terminal".to_string(),
            )),
        );

        assert_eq!(envelope.source, SignalSource::Clipboard);
        assert!(matches!(envelope.payload, StructuralSignal::Clipboard(_)));
        assert!(envelope.signal_id.starts_with("clipboard-"));
    }

    #[test]
    fn envelope_infers_selection_source() {
        let envelope = SignalEnvelope::from_signal(
            DeviceContext::default(),
            ApplicationContext::default(),
            StructuralSignal::Selection(crate::signals::SelectionSignal::text(
                "hello",
                "editor".to_string(),
                true,
            )),
        );

        assert_eq!(envelope.source, SignalSource::Selection);
        assert!(envelope.signal_id.starts_with("selection-"));
    }
}
