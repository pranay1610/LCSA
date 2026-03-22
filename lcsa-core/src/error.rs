use crate::permissions::Capability;
use crate::signals::SignalType;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("permission denied for {0:?}")]
    PermissionDenied(SignalType),

    #[error("capability denied for {0:?}")]
    CapabilityDenied(Capability),

    #[error("platform initialization failed: {0}")]
    PlatformError(String),

    #[error("signal type not supported on this platform: {0:?}")]
    UnsupportedSignal(SignalType),

    #[error("persistence file not found")]
    PersistenceNotFound,

    #[error("persistence error: {0}")]
    PersistenceError(String),
}
