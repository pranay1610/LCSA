use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crate::capabilities::SignalSupport;
use crate::error::Error;
use crate::event_bus::EventBus;
use crate::permissions::{Capability, Grant, PermissionRequest, PermissionStore};
use crate::platform::{
    read_clipboard_content, signal_support, spawn_clipboard_monitor, spawn_focus_monitor,
    spawn_selection_monitor, supports_signal,
};
use crate::signals::{ClipboardContent, ClipboardSignal, SignalType, StructuralSignal};
use crate::topology::{ApplicationContext, DeviceContext, SignalEnvelope};

pub struct ContextApi {
    bus: EventBus,
    permissions: PermissionStore,
    device_context: DeviceContext,
    application_context: ApplicationContext,
    clipboard_monitor_started: bool,
    selection_monitor_started: bool,
    focus_monitor_started: bool,
    shutdown: Arc<AtomicBool>,
    shutdown_notify: Arc<(Mutex<bool>, Condvar)>,
}

pub struct SubscriptionHandle {
    id: usize,
    signal_type: SignalType,
}

pub struct ContextApiBuilder {
    device_context: DeviceContext,
    application_context: ApplicationContext,
}

impl ContextApi {
    pub fn new() -> Result<Self, Error> {
        Self::builder().build()
    }

    pub fn builder() -> ContextApiBuilder {
        ContextApiBuilder {
            device_context: DeviceContext::default(),
            application_context: ApplicationContext::default(),
        }
    }

    pub fn device_context(&self) -> &DeviceContext {
        &self.device_context
    }

    pub fn application_context(&self) -> &ApplicationContext {
        &self.application_context
    }

    /// Subscribe to signals with envelope wrapping (includes device/app context).
    /// The handler is called on the event bus dispatcher thread.
    pub fn subscribe_enveloped<F>(
        &mut self,
        signal_type: SignalType,
        handler: F,
    ) -> Result<SubscriptionHandle, Error>
    where
        F: Fn(SignalEnvelope) + Send + 'static,
    {
        if !self.is_signal_supported(signal_type) {
            return Err(Error::UnsupportedSignal(signal_type));
        }

        if !self
            .permissions
            .is_granted(semantic_capability_for(signal_type))
        {
            return Err(Error::PermissionDenied(signal_type));
        }

        self.ensure_monitor(signal_type)?;

        let device_context = self.device_context.clone();
        let application_context = self.application_context.clone();

        // Register handler directly with the dispatcher (no new thread)
        let id = self.bus.register(move |signal| {
            if signal.matches(signal_type) {
                handler(SignalEnvelope::from_signal(
                    device_context.clone(),
                    application_context.clone(),
                    signal,
                ));
            }
        });

        Ok(SubscriptionHandle { id, signal_type })
    }

    pub fn read_clipboard_content(&self) -> Result<ClipboardContent, Error> {
        if !self
            .permissions
            .is_granted(Capability::ReadClipboardContent)
        {
            return Err(Error::CapabilityDenied(Capability::ReadClipboardContent));
        }

        read_clipboard_content()
    }

    pub fn is_signal_supported(&self, signal_type: SignalType) -> bool {
        supports_signal(signal_type)
    }

    pub fn signal_support(&self, signal_type: SignalType) -> SignalSupport {
        signal_support(signal_type)
    }

    pub fn supported_signals(&self) -> Vec<SignalType> {
        [
            SignalType::Clipboard,
            SignalType::Selection,
            SignalType::Focus,
        ]
        .into_iter()
        .filter(|signal_type| self.is_signal_supported(*signal_type))
        .collect()
    }

    /// Subscribe to clipboard signals only.
    /// The handler is called on the event bus dispatcher thread.
    pub fn subscribe<F>(
        &mut self,
        signal_type: SignalType,
        handler: F,
    ) -> Result<SubscriptionHandle, Error>
    where
        F: Fn(ClipboardSignal) + Send + 'static,
    {
        if signal_type != SignalType::Clipboard {
            return Err(Error::UnsupportedSignal(signal_type));
        }

        if !self
            .permissions
            .is_granted(semantic_capability_for(signal_type))
        {
            return Err(Error::PermissionDenied(signal_type));
        }

        self.ensure_monitor(signal_type)?;

        // Register handler directly with the dispatcher (no new thread)
        let id = self.bus.register(move |signal| {
            if let StructuralSignal::Clipboard(clipboard_signal) = signal {
                handler(clipboard_signal);
            }
        });

        Ok(SubscriptionHandle { id, signal_type })
    }

    pub fn unsubscribe(&mut self, handle: SubscriptionHandle) {
        let _ = handle.signal_type;
        self.bus.unregister(handle.id);
    }

    pub fn request_permission(&mut self, request: PermissionRequest) -> Result<Grant, Error> {
        Ok(self.permissions.grant(request))
    }

    pub fn revoke_permission(&mut self, capability: Capability) -> bool {
        self.permissions.revoke(capability)
    }

    pub fn can_access(&self, capability: Capability) -> bool {
        self.permissions.is_granted(capability)
    }

    /// Run the context API, blocking until shutdown is called.
    pub fn run(&self) {
        let (lock, cvar) = &*self.shutdown_notify;
        let mut ready = lock.lock().unwrap();
        while !self.shutdown.load(Ordering::SeqCst) {
            ready = cvar.wait(ready).unwrap();
        }
    }

    /// Signal shutdown and wait for completion.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.bus.shutdown();

        let (lock, cvar) = &*self.shutdown_notify;
        let mut ready = lock.lock().unwrap();
        *ready = true;
        cvar.notify_all();
    }

    /// Check if shutdown has been requested.
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }

    /// Run with Unix signal handling (SIGINT, SIGTERM).
    /// This method blocks until a signal is received or shutdown() is called.
    #[cfg(unix)]
    pub fn run_with_signals(&self) -> Result<(), Error> {
        use signal_hook::consts::{SIGINT, SIGTERM};
        use signal_hook::iterator::Signals;

        let mut signals = Signals::new([SIGINT, SIGTERM])
            .map_err(|e| Error::PlatformError(format!("failed to register signals: {}", e)))?;

        let shutdown = Arc::clone(&self.shutdown);
        let shutdown_notify = Arc::clone(&self.shutdown_notify);

        thread::spawn(move || {
            for _ in signals.forever() {
                shutdown.store(true, Ordering::SeqCst);
                let (lock, cvar) = &*shutdown_notify;
                let mut ready = lock.lock().unwrap();
                *ready = true;
                cvar.notify_all();
                break;
            }
        });

        self.run();
        self.bus.shutdown();
        Ok(())
    }

    fn ensure_monitor(&mut self, signal_type: SignalType) -> Result<(), Error> {
        match signal_type {
            SignalType::Clipboard => self.ensure_clipboard_monitor(),
            SignalType::Selection => self.ensure_selection_monitor(),
            SignalType::Focus => self.ensure_focus_monitor(),
        }
    }

    fn ensure_clipboard_monitor(&mut self) -> Result<(), Error> {
        ensure_started(&mut self.clipboard_monitor_started, || {
            spawn_clipboard_monitor(self.bus.clone())
        })
    }

    fn ensure_selection_monitor(&mut self) -> Result<(), Error> {
        ensure_started(&mut self.selection_monitor_started, || {
            spawn_selection_monitor(self.bus.clone())
        })
    }

    fn ensure_focus_monitor(&mut self) -> Result<(), Error> {
        ensure_started(&mut self.focus_monitor_started, || {
            spawn_focus_monitor(self.bus.clone())
        })
    }
}

impl ContextApiBuilder {
    pub fn device_context(mut self, device_context: DeviceContext) -> Self {
        self.device_context = device_context;
        self
    }

    pub fn application_context(mut self, application_context: ApplicationContext) -> Self {
        self.application_context = application_context;
        self
    }

    pub fn build(self) -> Result<ContextApi, Error> {
        Ok(ContextApi {
            bus: EventBus::new(),
            permissions: PermissionStore::with_defaults(),
            device_context: self.device_context,
            application_context: self.application_context,
            clipboard_monitor_started: false,
            selection_monitor_started: false,
            focus_monitor_started: false,
            shutdown: Arc::new(AtomicBool::new(false)),
            shutdown_notify: Arc::new((Mutex::new(false), Condvar::new())),
        })
    }
}

fn semantic_capability_for(signal_type: SignalType) -> Capability {
    match signal_type {
        SignalType::Clipboard => Capability::ReadClipboardSemantic,
        SignalType::Selection => Capability::ReadSelectionSemantic,
        SignalType::Focus => Capability::ReadFocusSemantic,
    }
}

fn ensure_started<F>(started: &mut bool, start: F) -> Result<(), Error>
where
    F: FnOnce() -> Result<(), Error>,
{
    if *started {
        return Ok(());
    }

    start()?;
    *started = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::capabilities::{SignalSupport, SignalUnsupportedReason};
    use crate::permissions::Scope;
    use crate::topology::{DeviceClass, Platform};

    #[test]
    fn semantic_access_is_available_by_default() {
        let api = ContextApi::new().expect("context api");
        assert!(api.can_access(Capability::ReadClipboardSemantic));
        assert!(api.can_access(Capability::ReadSelectionSemantic));
        assert!(api.can_access(Capability::ReadFocusSemantic));
        assert!(!api.can_access(Capability::ReadClipboardContent));
    }

    #[test]
    fn content_access_requires_explicit_grant() {
        let mut api = ContextApi::new().expect("context api");
        assert!(matches!(
            api.read_clipboard_content(),
            Err(Error::CapabilityDenied(Capability::ReadClipboardContent))
        ));

        let grant = api
            .request_permission(
                PermissionRequest::new(
                    Capability::ReadClipboardContent,
                    Scope::Session,
                    "Need clipboard content for paste-aware assistant behavior",
                )
                .with_ttl(Duration::from_secs(60)),
            )
            .expect("grant permission");

        assert_eq!(grant.capability, Capability::ReadClipboardContent);
        assert!(api.can_access(Capability::ReadClipboardContent));
    }

    #[test]
    fn builder_overrides_runtime_identity() {
        let api = ContextApi::builder()
            .device_context(DeviceContext {
                device_id: "phone:alice".to_string(),
                device_name: "alice-phone".to_string(),
                platform: Platform::Android,
                device_class: DeviceClass::Phone,
                os_version: Some("14".to_string()),
            })
            .application_context(ApplicationContext {
                app_id: "notes-sync".to_string(),
                app_name: "notes-sync".to_string(),
                app_version: Some("1.2.3".to_string()),
            })
            .build()
            .expect("context api");

        assert_eq!(api.device_context().device_id, "phone:alice");
        assert_eq!(api.application_context().app_id, "notes-sync");
    }

    #[test]
    fn clipboard_only_subscription_rejects_non_clipboard_signal_types() {
        let mut api = ContextApi::new().expect("context api");
        let result = api.subscribe(SignalType::Selection, |_| {});
        assert!(matches!(
            result,
            Err(Error::UnsupportedSignal(SignalType::Selection))
        ));
    }

    #[test]
    fn supported_signals_reflect_platform_backends() {
        let api = ContextApi::new().expect("context api");
        let supported = api.supported_signals();
        assert!(supported.contains(&SignalType::Clipboard));
        assert!(api.is_signal_supported(SignalType::Clipboard));
        assert_eq!(
            api.is_signal_supported(SignalType::Selection),
            matches!(
                api.signal_support(SignalType::Selection),
                SignalSupport::Supported
            )
        );
        assert_eq!(
            api.is_signal_supported(SignalType::Focus),
            matches!(
                api.signal_support(SignalType::Focus),
                SignalSupport::Supported
            )
        );
    }

    #[test]
    fn signal_support_returns_reason_for_unsupported_signals() {
        let api = ContextApi::new().expect("context api");

        #[cfg(target_os = "linux")]
        {
            if std::env::var_os("DISPLAY").is_none() {
                assert_eq!(
                    api.signal_support(SignalType::Focus),
                    SignalSupport::Unsupported(SignalUnsupportedReason::RequiresX11Display)
                );
            } else {
                assert_ne!(
                    api.signal_support(SignalType::Focus),
                    SignalSupport::Unsupported(SignalUnsupportedReason::RequiresX11Display)
                );
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            assert_ne!(
                api.signal_support(SignalType::Focus),
                SignalSupport::Unsupported(SignalUnsupportedReason::RequiresX11Display)
            );
        }
    }

    #[test]
    fn shutdown_completes_cleanly() {
        let api = ContextApi::new().expect("context api");
        assert!(!api.is_shutdown());
        api.shutdown();
        assert!(api.is_shutdown());
    }
}
