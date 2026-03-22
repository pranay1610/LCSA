mod capabilities;
mod context_api;
mod error;
mod event_bus;
mod filesystem;
mod mobile_policy;
mod permissions;
mod platform;
mod signals;
mod topology;

pub use capabilities::{SignalSupport, SignalUnsupportedReason};
pub use context_api::{ContextApi, ContextApiBuilder, SubscriptionHandle};
pub use error::Error;
pub use filesystem::{
    EntityKind, PrimitiveEvent, PrimitiveEventKind, SemanticSignal, SignalAction,
    infer_entity_kind, normalize_event,
};
pub use mobile_policy::{MobileClipboardModel, MobilePolicy, SignalDeliveryModel};
pub use permissions::{Capability, Grant, PermissionRequest, Scope};
pub use signals::{
    ClipboardContent, ClipboardPayload, ClipboardSignal, ContentType, FocusSignal, FocusTarget,
    SelectionSignal, SignalType, StructuralSignal, detect_content_type,
};
pub use topology::{
    ApplicationContext, DeviceClass, DeviceContext, Platform, SignalEnvelope, SignalSource,
};
