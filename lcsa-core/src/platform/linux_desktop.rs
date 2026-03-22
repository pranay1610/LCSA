use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use arboard::{Clipboard, GetExtLinux, LinuxClipboardKind};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xfixes::{ConnectionExt as XFixesConnectionExt, SelectionEventMask};
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt, EventMask, Window};
use x11rb::rust_connection::RustConnection;

use crate::capabilities::{SignalSupport, SignalUnsupportedReason};
use crate::error::Error;
use crate::event_bus::EventBus;
use crate::signals::{FocusSignal, FocusTarget, SelectionSignal, SignalType, StructuralSignal};

const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Shared state for tracking the last focused application.
/// This allows selection signals to include the correct source_app.
static LAST_FOCUSED_APP: std::sync::LazyLock<Arc<Mutex<String>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new("unknown".to_string())));

fn is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some() && std::env::var_os("DISPLAY").is_none()
}

fn has_x11_display() -> bool {
    std::env::var_os("DISPLAY").is_some()
}

// ============================================================================
// Public API
// ============================================================================

pub(crate) fn spawn_selection_monitor(bus: EventBus) -> Result<(), Error> {
    if !selection_support().is_supported() {
        return Err(Error::UnsupportedSignal(SignalType::Selection));
    }

    if is_wayland() || !has_x11_display() {
        spawn_polling_selection_monitor(bus)
    } else {
        spawn_xfixes_selection_monitor(bus)
    }
}

pub(crate) fn spawn_focus_monitor(bus: EventBus) -> Result<(), Error> {
    if !focus_support().is_supported() {
        return Err(Error::UnsupportedSignal(SignalType::Focus));
    }

    if is_wayland() || !has_x11_display() {
        return Err(Error::PlatformError(
            "focus monitoring requires X11 DISPLAY".to_string(),
        ));
    }

    spawn_xfixes_focus_monitor(bus)
}

pub(crate) fn selection_support() -> SignalSupport {
    if Clipboard::new().is_ok() {
        SignalSupport::Supported
    } else {
        SignalSupport::Unsupported(SignalUnsupportedReason::RuntimeDependencyMissing)
    }
}

pub(crate) fn focus_support() -> SignalSupport {
    if !has_x11_display() {
        return SignalSupport::Unsupported(SignalUnsupportedReason::RequiresX11Display);
    }

    match x11rb::connect(None) {
        Ok(_) => SignalSupport::Supported,
        Err(_) => SignalSupport::Unsupported(SignalUnsupportedReason::RuntimeDependencyMissing),
    }
}

// ============================================================================
// XFixes Event-Driven Implementation (X11)
// ============================================================================

struct X11Atoms {
    clipboard: Atom,
    primary: Atom,
    net_active_window: Atom,
    net_wm_name: Atom,
    utf8_string: Atom,
}

impl X11Atoms {
    fn new(conn: &RustConnection) -> Option<Self> {
        Some(Self {
            clipboard: intern_atom(conn, b"CLIPBOARD")?,
            primary: intern_atom(conn, b"PRIMARY")?,
            net_active_window: intern_atom(conn, b"_NET_ACTIVE_WINDOW")?,
            net_wm_name: intern_atom(conn, b"_NET_WM_NAME")?,
            utf8_string: intern_atom(conn, b"UTF8_STRING")?,
        })
    }
}

fn spawn_xfixes_selection_monitor(bus: EventBus) -> Result<(), Error> {
    // Verify XFixes extension is available
    let (conn, screen_num) =
        x11rb::connect(None).map_err(|e| Error::PlatformError(e.to_string()))?;
    let root = conn.setup().roots[screen_num].root;

    // Query XFixes version
    conn.xfixes_query_version(5, 0)
        .map_err(|e| Error::PlatformError(format!("XFixes query failed: {}", e)))?
        .reply()
        .map_err(|e| Error::PlatformError(format!("XFixes not available: {}", e)))?;

    let atoms = X11Atoms::new(&conn)
        .ok_or_else(|| Error::PlatformError("failed to intern atoms".to_string()))?;

    // Register for selection change events on PRIMARY
    conn.xfixes_select_selection_input(
        root,
        atoms.primary,
        SelectionEventMask::SET_SELECTION_OWNER
            | SelectionEventMask::SELECTION_WINDOW_DESTROY
            | SelectionEventMask::SELECTION_CLIENT_CLOSE,
    )
    .map_err(|e| Error::PlatformError(format!("XFixes select failed: {}", e)))?;

    conn.flush()
        .map_err(|e| Error::PlatformError(format!("flush failed: {}", e)))?;

    thread::spawn(move || {
        let mut clipboard = match Clipboard::new() {
            Ok(c) => c,
            Err(_) => return,
        };

        loop {
            match conn.wait_for_event() {
                Ok(Event::XfixesSelectionNotify(event)) => {
                    if event.selection == atoms.primary {
                        // Small delay to let the selection owner populate content
                        thread::sleep(Duration::from_millis(50));

                        if let Some(signal) = read_primary_selection_signal(&mut clipboard) {
                            bus.emit(StructuralSignal::Selection(signal));
                        }
                    }
                }
                Ok(_) => {
                    // Ignore other events
                }
                Err(_) => {
                    // Connection error, exit the loop
                    break;
                }
            }
        }
    });

    Ok(())
}

fn spawn_xfixes_focus_monitor(bus: EventBus) -> Result<(), Error> {
    let (conn, screen_num) =
        x11rb::connect(None).map_err(|e| Error::PlatformError(e.to_string()))?;
    let root = conn.setup().roots[screen_num].root;

    let atoms = X11Atoms::new(&conn)
        .ok_or_else(|| Error::PlatformError("failed to intern atoms".to_string()))?;

    // Subscribe to PropertyNotify events on the root window for _NET_ACTIVE_WINDOW
    conn.change_window_attributes(
        root,
        &x11rb::protocol::xproto::ChangeWindowAttributesAux::new()
            .event_mask(EventMask::PROPERTY_CHANGE),
    )
    .map_err(|e| Error::PlatformError(format!("change_window_attributes failed: {}", e)))?;

    conn.flush()
        .map_err(|e| Error::PlatformError(format!("flush failed: {}", e)))?;

    // Get initial focus
    if let Some(window) = read_active_window(&conn, root, atoms.net_active_window) {
        if let Some(source) = read_focus_source(&conn, window, &atoms) {
            update_last_focused_app(&source);
        }
    }

    thread::spawn(move || {
        loop {
            match conn.wait_for_event() {
                Ok(Event::PropertyNotify(event)) => {
                    if event.atom == atoms.net_active_window {
                        if let Some(window) =
                            read_active_window(&conn, root, atoms.net_active_window)
                        {
                            let source = read_focus_source(&conn, window, &atoms)
                                .unwrap_or_else(|| "unknown".to_string());

                            update_last_focused_app(&source);

                            let target = classify_focus_target(&source);
                            let signal = FocusSignal::new(source, target, false);
                            bus.emit(StructuralSignal::Focus(signal));
                        }
                    }
                }
                Ok(_) => {
                    // Ignore other events
                }
                Err(_) => {
                    // Connection error, exit the loop
                    break;
                }
            }
        }
    });

    Ok(())
}

// ============================================================================
// Polling Fallback (Wayland)
// ============================================================================

fn spawn_polling_selection_monitor(bus: EventBus) -> Result<(), Error> {
    Clipboard::new().map_err(|e| Error::PlatformError(e.to_string()))?;

    thread::spawn(move || {
        let mut clipboard = match Clipboard::new() {
            Ok(c) => c,
            Err(_) => return,
        };

        let mut last_fingerprint: Option<u64> = None;

        loop {
            if let Some(signal) = read_primary_selection_signal(&mut clipboard) {
                let fingerprint = hash_string(&format!("{:?}", signal));
                let has_changed = last_fingerprint
                    .map(|prev| prev != fingerprint)
                    .unwrap_or(true);

                if has_changed {
                    last_fingerprint = Some(fingerprint);
                    bus.emit(StructuralSignal::Selection(signal));
                }
            }

            thread::sleep(POLL_INTERVAL);
        }
    });

    Ok(())
}

// ============================================================================
// Shared Helpers
// ============================================================================

fn update_last_focused_app(app_name: &str) {
    if let Ok(mut locked) = LAST_FOCUSED_APP.lock() {
        *locked = app_name.to_string();
    }
}

fn get_last_focused_app() -> String {
    LAST_FOCUSED_APP
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn read_primary_selection_signal(clipboard: &mut Clipboard) -> Option<SelectionSignal> {
    let text = clipboard
        .get()
        .clipboard(LinuxClipboardKind::Primary)
        .text()
        .ok()?;

    if text.trim().is_empty() {
        return None;
    }

    // Use the tracked focused app as source_app instead of "unknown"
    let source_app = get_last_focused_app();
    Some(SelectionSignal::text(&text, source_app, false))
}

fn read_active_window<C: Connection>(conn: &C, root: Window, atom: Atom) -> Option<Window> {
    let reply = conn
        .get_property(false, root, atom, AtomEnum::WINDOW, 0, 1)
        .ok()?
        .reply()
        .ok()?;

    reply.value32()?.next()
}

fn read_focus_source<C: Connection>(conn: &C, window: Window, atoms: &X11Atoms) -> Option<String> {
    read_text_property(conn, window, atoms.net_wm_name, atoms.utf8_string)
        .or_else(|| read_wm_class(conn, window))
        .or_else(|| {
            read_text_property(
                conn,
                window,
                AtomEnum::WM_NAME.into(),
                AtomEnum::STRING.into(),
            )
        })
}

fn read_text_property<C: Connection>(
    conn: &C,
    window: Window,
    property: Atom,
    property_type: Atom,
) -> Option<String> {
    let reply = conn
        .get_property(false, window, property, property_type, 0, 2048)
        .ok()?
        .reply()
        .ok()?;

    let value = String::from_utf8_lossy(&reply.value).trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

fn read_wm_class<C: Connection>(conn: &C, window: Window) -> Option<String> {
    let reply = conn
        .get_property(false, window, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 1024)
        .ok()?
        .reply()
        .ok()?;

    let value = String::from_utf8_lossy(&reply.value);
    value
        .split('\0')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .next_back()
        .map(|s| s.to_string())
}

fn intern_atom<C: Connection>(conn: &C, name: &[u8]) -> Option<Atom> {
    conn.intern_atom(false, name)
        .ok()?
        .reply()
        .ok()
        .map(|r| r.atom)
}

fn classify_focus_target(source: &str) -> FocusTarget {
    let normalized = source.to_ascii_lowercase();

    if contains_any(
        &normalized,
        &[
            "terminal",
            "wezterm",
            "alacritty",
            "kitty",
            "xterm",
            "konsole",
            "tilix",
            "gnome-terminal",
        ],
    ) {
        return FocusTarget::Terminal;
    }

    if contains_any(
        &normalized,
        &[
            "firefox", "chrome", "chromium", "brave", "edge", "vivaldi", "opera",
        ],
    ) {
        return FocusTarget::Browser;
    }

    if normalized.trim().is_empty() || normalized == "unknown" {
        FocusTarget::Unknown
    } else {
        FocusTarget::Window
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn hash_string(input: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_terminal_focus() {
        assert_eq!(classify_focus_target("WezTerm"), FocusTarget::Terminal);
    }

    #[test]
    fn classifies_browser_focus() {
        assert_eq!(
            classify_focus_target("Mozilla Firefox"),
            FocusTarget::Browser
        );
    }

    #[test]
    fn classifies_unknown_focus() {
        assert_eq!(classify_focus_target("unknown"), FocusTarget::Unknown);
    }

    #[test]
    fn wayland_detection() {
        // Just verify the function doesn't panic
        let _ = is_wayland();
        let _ = has_x11_display();
    }

    #[test]
    fn last_focused_app_tracking() {
        update_last_focused_app("test-app");
        assert_eq!(get_last_focused_app(), "test-app");

        update_last_focused_app("another-app");
        assert_eq!(get_last_focused_app(), "another-app");
    }
}
