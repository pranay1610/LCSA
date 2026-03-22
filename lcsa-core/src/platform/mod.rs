#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod clipboard;
#[cfg(target_os = "linux")]
mod linux_desktop;
#[cfg(target_os = "macos")]
mod macos_desktop;
#[cfg(target_os = "windows")]
mod windows_desktop;

use crate::capabilities::{SignalSupport, SignalUnsupportedReason};
use crate::error::Error;
use crate::event_bus::EventBus;
use crate::signals::ClipboardContent;
use crate::signals::SignalType;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(crate) fn spawn_clipboard_monitor(bus: EventBus) -> Result<(), Error> {
    clipboard::spawn_clipboard_monitor(bus)
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(crate) fn read_clipboard_content() -> Result<ClipboardContent, Error> {
    clipboard::read_clipboard_content()
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub(crate) fn spawn_clipboard_monitor(_bus: EventBus) -> Result<(), Error> {
    Err(Error::UnsupportedSignal(SignalType::Clipboard))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub(crate) fn read_clipboard_content() -> Result<ClipboardContent, Error> {
    Err(Error::UnsupportedSignal(SignalType::Clipboard))
}

pub(crate) fn spawn_selection_monitor(_bus: EventBus) -> Result<(), Error> {
    #[cfg(target_os = "linux")]
    {
        return linux_desktop::spawn_selection_monitor(_bus);
    }

    #[cfg(target_os = "macos")]
    {
        return macos_desktop::spawn_selection_monitor(_bus);
    }

    #[cfg(target_os = "windows")]
    {
        return windows_desktop::spawn_selection_monitor(_bus);
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = _bus;
        Err(Error::UnsupportedSignal(SignalType::Selection))
    }
}

pub(crate) fn spawn_focus_monitor(_bus: EventBus) -> Result<(), Error> {
    #[cfg(target_os = "linux")]
    {
        return linux_desktop::spawn_focus_monitor(_bus);
    }

    #[cfg(target_os = "macos")]
    {
        return macos_desktop::spawn_focus_monitor(_bus);
    }

    #[cfg(target_os = "windows")]
    {
        return windows_desktop::spawn_focus_monitor(_bus);
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = _bus;
        Err(Error::UnsupportedSignal(SignalType::Focus))
    }
}

pub(crate) fn supports_signal(signal_type: SignalType) -> bool {
    signal_support(signal_type).is_supported()
}

pub(crate) fn signal_support(signal_type: SignalType) -> SignalSupport {
    match signal_type {
        SignalType::Clipboard => {
            if cfg!(any(
                target_os = "linux",
                target_os = "macos",
                target_os = "windows"
            )) {
                SignalSupport::Supported
            } else {
                SignalSupport::Unsupported(SignalUnsupportedReason::PlatformNotSupported)
            }
        }
        SignalType::Selection => {
            if cfg!(target_os = "linux") {
                #[cfg(target_os = "linux")]
                {
                    return linux_desktop::selection_support();
                }
                #[cfg(not(target_os = "linux"))]
                unreachable!();
            } else if cfg!(target_os = "macos") {
                #[cfg(target_os = "macos")]
                {
                    return macos_desktop::selection_support();
                }
                #[cfg(not(target_os = "macos"))]
                unreachable!();
            } else if cfg!(target_os = "windows") {
                #[cfg(target_os = "windows")]
                {
                    return windows_desktop::selection_support();
                }
                #[cfg(not(target_os = "windows"))]
                unreachable!();
            } else {
                SignalSupport::Unsupported(SignalUnsupportedReason::BackendNotImplemented)
            }
        }
        SignalType::Focus => {
            if cfg!(target_os = "linux") {
                #[cfg(target_os = "linux")]
                {
                    return linux_desktop::focus_support();
                }
                #[cfg(not(target_os = "linux"))]
                unreachable!();
            } else if cfg!(target_os = "macos") {
                #[cfg(target_os = "macos")]
                {
                    return macos_desktop::focus_support();
                }
                #[cfg(not(target_os = "macos"))]
                unreachable!();
            } else if cfg!(target_os = "windows") {
                #[cfg(target_os = "windows")]
                {
                    return windows_desktop::focus_support();
                }
                #[cfg(not(target_os = "windows"))]
                unreachable!();
            } else {
                SignalSupport::Unsupported(SignalUnsupportedReason::BackendNotImplemented)
            }
        }
    }
}
