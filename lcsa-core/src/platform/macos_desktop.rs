use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::capabilities::{SignalSupport, SignalUnsupportedReason};
use crate::error::Error;
use crate::event_bus::EventBus;
use crate::signals::{FocusSignal, FocusTarget, SelectionSignal, StructuralSignal};

const POLL_INTERVAL: Duration = Duration::from_millis(250);

pub(crate) fn spawn_focus_monitor(bus: EventBus) -> Result<(), Error> {
    if !focus_support().is_supported() {
        return Err(Error::UnsupportedSignal(crate::signals::SignalType::Focus));
    }

    let _ = read_focus_source()?;

    thread::spawn(move || {
        let mut last_source: Option<String> = None;

        loop {
            if let Ok(source) = read_focus_source() {
                if source.is_empty() {
                    thread::sleep(POLL_INTERVAL);
                    continue;
                }

                let changed = last_source
                    .as_ref()
                    .map(|previous| previous != &source)
                    .unwrap_or(true);

                if changed {
                    last_source = Some(source.clone());
                    let target = classify_focus_target(&source);
                    let signal = FocusSignal::new(source, target, false);
                    bus.emit(StructuralSignal::Focus(signal));
                }
            }

            thread::sleep(POLL_INTERVAL);
        }
    });

    Ok(())
}

pub(crate) fn spawn_selection_monitor(bus: EventBus) -> Result<(), Error> {
    if !selection_support().is_supported() {
        return Err(Error::UnsupportedSignal(
            crate::signals::SignalType::Selection,
        ));
    }

    thread::spawn(move || {
        let mut last_fingerprint: Option<String> = None;

        loop {
            if let Ok(text) = read_selected_text() {
                if text.trim().is_empty() {
                    thread::sleep(POLL_INTERVAL);
                    continue;
                }

                let fingerprint = format!("text:{}", hash_string(&text));
                let changed = last_fingerprint
                    .as_ref()
                    .map(|previous| previous != &fingerprint)
                    .unwrap_or(true);

                if changed {
                    last_fingerprint = Some(fingerprint);
                    let source = read_focus_source().unwrap_or_else(|_| "unknown".to_string());
                    let signal = SelectionSignal::text(&text, source, true);
                    bus.emit(StructuralSignal::Selection(signal));
                }
            }

            thread::sleep(POLL_INTERVAL);
        }
    });

    Ok(())
}

pub(crate) fn focus_support() -> SignalSupport {
    match read_focus_source() {
        Ok(_) => SignalSupport::Supported,
        Err(error) if is_accessibility_permission_error(&error) => {
            SignalSupport::Unsupported(SignalUnsupportedReason::RequiresAccessibilityPermission)
        }
        Err(_) => SignalSupport::Unsupported(SignalUnsupportedReason::RuntimeDependencyMissing),
    }
}

pub(crate) fn selection_support() -> SignalSupport {
    match probe_accessibility() {
        Ok(_) => SignalSupport::Supported,
        Err(error) if is_accessibility_permission_error(&error) => {
            SignalSupport::Unsupported(SignalUnsupportedReason::RequiresAccessibilityPermission)
        }
        Err(_) => SignalSupport::Unsupported(SignalUnsupportedReason::RuntimeDependencyMissing),
    }
}

fn read_focus_source() -> Result<String, Error> {
    run_osascript(
        "tell application \"System Events\" to get name of first process whose frontmost is true",
    )
}

fn read_selected_text() -> Result<String, Error> {
    run_osascript(
        "tell application \"System Events\"
            set frontProcess to first process whose frontmost is true
            try
                set focusedElement to value of attribute \"AXFocusedUIElement\" of frontProcess
                set selectedText to value of attribute \"AXSelectedText\" of focusedElement
                return selectedText
            on error
                return \"\"
            end try
        end tell",
    )
}

fn probe_accessibility() -> Result<String, Error> {
    run_osascript(
        "tell application \"System Events\"
            set frontProcess to first process whose frontmost is true
            set focusedElement to value of attribute \"AXFocusedUIElement\" of frontProcess
            return value of attribute \"AXRole\" of focusedElement
        end tell",
    )
}

fn run_osascript(script: &str) -> Result<String, Error> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|error| Error::PlatformError(format!("failed to invoke osascript: {error}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(Error::PlatformError(format!("osascript failed: {stderr}")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn classify_focus_target(source: &str) -> FocusTarget {
    let normalized = source.to_ascii_lowercase();

    if contains_any(
        &normalized,
        &[
            "terminal",
            "iterm",
            "wezterm",
            "kitty",
            "alacritty",
            "warp",
            "ghostty",
        ],
    ) {
        return FocusTarget::Terminal;
    }

    if contains_any(
        &normalized,
        &[
            "safari", "chrome", "firefox", "brave", "edge", "opera", "arc",
        ],
    ) {
        return FocusTarget::Browser;
    }

    if normalized.trim().is_empty() || normalized == "unknown" {
        FocusTarget::Unknown
    } else {
        FocusTarget::Application
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn is_accessibility_permission_error(error: &Error) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("not authorized")
        || message.contains("accessibility")
        || message.contains("1743")
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
    fn classifies_browser_focus() {
        assert_eq!(classify_focus_target("Google Chrome"), FocusTarget::Browser);
    }

    #[test]
    fn classifies_terminal_focus() {
        assert_eq!(classify_focus_target("iTerm2"), FocusTarget::Terminal);
    }
}
