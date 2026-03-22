use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalUnsupportedReason {
    PlatformNotSupported,
    BackendNotImplemented,
    RequiresX11Display,
    RequiresAccessibilityPermission,
    RuntimeDependencyMissing,
}

impl SignalUnsupportedReason {
    pub fn as_str(self) -> &'static str {
        match self {
            SignalUnsupportedReason::PlatformNotSupported => "platform_not_supported",
            SignalUnsupportedReason::BackendNotImplemented => "backend_not_implemented",
            SignalUnsupportedReason::RequiresX11Display => "requires_x11_display",
            SignalUnsupportedReason::RequiresAccessibilityPermission => {
                "requires_accessibility_permission"
            }
            SignalUnsupportedReason::RuntimeDependencyMissing => "runtime_dependency_missing",
        }
    }
}

impl fmt::Display for SignalUnsupportedReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "reason", rename_all = "snake_case")]
pub enum SignalSupport {
    Supported,
    Unsupported(SignalUnsupportedReason),
}

impl SignalSupport {
    pub fn as_str(self) -> String {
        match self {
            SignalSupport::Supported => "supported".to_string(),
            SignalSupport::Unsupported(reason) => format!("unsupported:{}", reason.as_str()),
        }
    }
}

impl fmt::Display for SignalSupport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignalSupport::Supported => f.write_str("supported"),
            SignalSupport::Unsupported(reason) => write!(f, "unsupported:{reason}"),
        }
    }
}

impl SignalSupport {
    pub fn is_supported(self) -> bool {
        matches!(self, SignalSupport::Supported)
    }
}
