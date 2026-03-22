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
        Err(_) => SignalSupport::Unsupported(SignalUnsupportedReason::RuntimeDependencyMissing),
    }
}

pub(crate) fn selection_support() -> SignalSupport {
    match read_selected_text() {
        Ok(_) => SignalSupport::Supported,
        Err(_) => SignalSupport::Unsupported(SignalUnsupportedReason::RuntimeDependencyMissing),
    }
}

fn read_focus_source() -> Result<String, Error> {
    let script = r#"
$signature = @"
using System;
using System.Runtime.InteropServices;
public static class Win32Focus {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);
}
"@
Add-Type -TypeDefinition $signature -ErrorAction SilentlyContinue | Out-Null
$hwnd = [Win32Focus]::GetForegroundWindow()
if ($hwnd -eq [IntPtr]::Zero) { exit 1 }
$pid = 0
[Win32Focus]::GetWindowThreadProcessId($hwnd, [ref]$pid) | Out-Null
$process = Get-Process -Id $pid -ErrorAction SilentlyContinue
if ($null -eq $process) { exit 1 }
Write-Output $process.ProcessName
"#;

    run_powershell(script)
}

fn read_selected_text() -> Result<String, Error> {
    let script = r#"
Add-Type -AssemblyName UIAutomationClient
$focused = [System.Windows.Automation.AutomationElement]::FocusedElement
if ($null -eq $focused) { Write-Output ""; exit 0 }

$text = ""
try {
  $textPattern = $focused.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern)
  if ($null -ne $textPattern) {
    $ranges = $textPattern.GetSelection()
    if ($ranges.Length -gt 0) {
      $text = $ranges[0].GetText(-1)
    }
  }
} catch {}

Write-Output $text
"#;

    run_powershell(script)
}

fn run_powershell(script: &str) -> Result<String, Error> {
    let output = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(script)
        .output()
        .map_err(|error| Error::PlatformError(format!("failed to invoke powershell: {error}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(Error::PlatformError(format!(
            "powershell focus query failed: {stderr}"
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn classify_focus_target(source: &str) -> FocusTarget {
    let normalized = source.to_ascii_lowercase();

    if contains_any(
        &normalized,
        &[
            "windows terminal",
            "terminal",
            "powershell",
            "cmd",
            "wezterm",
            "alacritty",
            "mintty",
            "wt",
        ],
    ) {
        return FocusTarget::Terminal;
    }

    if contains_any(
        &normalized,
        &[
            "chrome", "firefox", "msedge", "edge", "brave", "opera", "iexplore",
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
        assert_eq!(
            classify_focus_target("Windows Terminal"),
            FocusTarget::Terminal
        );
    }

    #[test]
    fn classifies_browser_focus() {
        assert_eq!(classify_focus_target("msedge"), FocusTarget::Browser);
    }
}
