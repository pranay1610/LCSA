use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use lcsa_core::{
    Capability, ContextApi, PermissionRequest, Scope, SignalEnvelope, SignalSupport, SignalType,
    StructuralSignal,
};
use serde_json::json;

const MAX_EVENTS: usize = 20;

#[derive(Clone, Default)]
struct AdapterState {
    recent_focus: Option<String>,
    recent_selection: Option<String>,
    recent_clipboard: Option<String>,
    recent_events: VecDeque<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut api = ContextApi::new()?;
    let state = Arc::new(Mutex::new(AdapterState::default()));

    println!("LCSA Assistant Adapter Demo");
    println!("Type a developer task and press Enter.");
    println!("Commands: :help :context :grant-content :revoke-content :quit");
    println!();

    attach_signal_subscriptions(&mut api, Arc::clone(&state))?;

    let stdin = io::stdin();
    loop {
        print!("ask> ");
        io::stdout().flush()?;

        let mut input = String::new();
        stdin.read_line(&mut input)?;
        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        match input {
            ":help" => print_help(),
            ":context" => print_context_snapshot(&api, &state),
            ":grant-content" => grant_clipboard_content(&mut api)?,
            ":revoke-content" => {
                let revoked = api.revoke_permission(Capability::ReadClipboardContent);
                println!("clipboard content access revoked={revoked}");
            }
            ":quit" | ":exit" => break,
            task => print_augmented_packet(&api, &state, task),
        }
    }

    Ok(())
}

fn attach_signal_subscriptions(
    api: &mut ContextApi,
    state: Arc<Mutex<AdapterState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    for signal_type in [
        SignalType::Clipboard,
        SignalType::Selection,
        SignalType::Focus,
    ] {
        match api.signal_support(signal_type) {
            SignalSupport::Supported => {
                let state = Arc::clone(&state);
                api.subscribe_enveloped(signal_type, move |envelope| {
                    ingest_envelope(&state, envelope);
                })?;
                println!("{signal_type:?}: subscribed");
            }
            unsupported => {
                println!("{signal_type:?}: {unsupported:?}");
            }
        }
    }

    println!();
    Ok(())
}

fn ingest_envelope(state: &Arc<Mutex<AdapterState>>, envelope: SignalEnvelope) {
    let mut state = state.lock().expect("adapter state mutex poisoned");
    let ts = unix_secs(envelope.emitted_at);

    match envelope.payload {
        StructuralSignal::Clipboard(signal) => {
            let summary = format!(
                "clipboard type={:?} bytes={} sensitive={} command={} source={}",
                signal.content_type,
                signal.size_bytes,
                signal.likely_sensitive,
                signal.likely_command,
                signal.source_app
            );
            state.recent_clipboard = Some(summary.clone());
            push_event(&mut state.recent_events, format!("{ts} {summary}"));
        }
        StructuralSignal::Selection(signal) => {
            let preview = format!(
                "selection type={:?} bytes={} editable={} source={}",
                signal.content_type, signal.size_bytes, signal.is_editable, signal.source_app
            );
            state.recent_selection = Some(preview.clone());
            push_event(&mut state.recent_events, format!("{ts} {preview}"));
        }
        StructuralSignal::Focus(signal) => {
            let summary = format!(
                "focus target={:?} editable={} source={}",
                signal.target, signal.is_editable, signal.source_app
            );
            state.recent_focus = Some(summary.clone());
            push_event(&mut state.recent_events, format!("{ts} {summary}"));
        }
        StructuralSignal::Filesystem(signal) => {
            push_event(
                &mut state.recent_events,
                format!(
                    "{ts} filesystem {} {:?}",
                    signal.event_name(),
                    signal.action
                ),
            );
        }
    }
}

fn print_help() {
    println!(":context         show current signal snapshot");
    println!(":grant-content   grant clipboard content access for this session");
    println!(":revoke-content  revoke clipboard content access");
    println!(":quit            exit");
}

fn print_context_snapshot(api: &ContextApi, state: &Arc<Mutex<AdapterState>>) {
    let snapshot = state.lock().expect("adapter state mutex poisoned").clone();
    let packet = build_context_packet(api, &snapshot, "(snapshot)", None);
    println!(
        "{}",
        serde_json::to_string_pretty(&packet).unwrap_or_default()
    );
}

fn grant_clipboard_content(api: &mut ContextApi) -> Result<(), Box<dyn std::error::Error>> {
    api.request_permission(PermissionRequest::new(
        Capability::ReadClipboardContent,
        Scope::Session,
        "Enable content-level context in assistant adapter demo",
    ))?;
    println!("clipboard content access granted for current session");
    Ok(())
}

fn print_augmented_packet(api: &ContextApi, state: &Arc<Mutex<AdapterState>>, task: &str) {
    let snapshot = state.lock().expect("adapter state mutex poisoned").clone();
    let clipboard_preview = api.read_clipboard_content().ok().map(|content| {
        // The preview intentionally keeps sensitive content redacted when needed.
        content.redacted_preview()
    });

    let packet = build_context_packet(api, &snapshot, task, clipboard_preview.clone());
    println!();
    println!("=== Augmented Context Packet ===");
    println!(
        "{}",
        serde_json::to_string_pretty(&packet).unwrap_or_default()
    );
    println!();
    println!("=== Prompt Preview ===");
    println!(
        "{}",
        build_prompt_preview(task, &snapshot, clipboard_preview)
    );
    println!();
}

fn build_context_packet(
    api: &ContextApi,
    snapshot: &AdapterState,
    task: &str,
    clipboard_preview: Option<String>,
) -> serde_json::Value {
    json!({
        "task": task,
        "device": {
            "id": api.device_context().device_id,
            "platform": format!("{:?}", api.device_context().platform),
        },
        "capabilities": {
            "clipboard": format!("{:?}", api.signal_support(SignalType::Clipboard)),
            "selection": format!("{:?}", api.signal_support(SignalType::Selection)),
            "focus": format!("{:?}", api.signal_support(SignalType::Focus)),
            "clipboard_content_access": api.can_access(Capability::ReadClipboardContent),
        },
        "context": {
            "recent_focus": snapshot.recent_focus,
            "recent_selection": snapshot.recent_selection,
            "recent_clipboard_signal": snapshot.recent_clipboard,
            "clipboard_content_preview": clipboard_preview,
            "recent_events": snapshot.recent_events.iter().take(8).cloned().collect::<Vec<_>>(),
        },
        "hints": derive_hints(snapshot),
    })
}

fn build_prompt_preview(
    task: &str,
    snapshot: &AdapterState,
    clipboard_preview: Option<String>,
) -> String {
    let mut lines = vec![
        "You are a coding assistant helping with a local development task.".to_string(),
        format!("Task: {task}"),
    ];

    if let Some(focus) = &snapshot.recent_focus {
        lines.push(format!("Current focus: {focus}"));
    }
    if let Some(selection) = &snapshot.recent_selection {
        lines.push(format!("Recent selection: {selection}"));
    }
    if let Some(clipboard) = &snapshot.recent_clipboard {
        lines.push(format!("Recent clipboard signal: {clipboard}"));
    }
    if let Some(preview) = clipboard_preview {
        lines.push(format!("Clipboard content preview: {preview}"));
    }

    for hint in derive_hints(snapshot) {
        lines.push(format!("Hint: {hint}"));
    }

    lines.join("\n")
}

fn derive_hints(snapshot: &AdapterState) -> Vec<String> {
    let mut hints = Vec::new();

    if let Some(focus) = &snapshot.recent_focus {
        let lowercase = focus.to_ascii_lowercase();
        if lowercase.contains("terminal") {
            hints.push("User is likely in a terminal-driven debugging flow.".to_string());
        }
        if lowercase.contains("browser") {
            hints.push("User may be validating behavior in a browser.".to_string());
        }
    }

    if let Some(selection) = &snapshot.recent_selection {
        if selection.contains("Code") {
            hints.push(
                "Selected text looks code-like; prioritize code-aware suggestions.".to_string(),
            );
        }
    }

    if let Some(clipboard) = &snapshot.recent_clipboard {
        if clipboard.contains("sensitive=true") {
            hints
                .push("Clipboard likely contains sensitive data; avoid verbatim echo.".to_string());
        }
        if clipboard.contains("command=true") {
            hints.push(
                "Clipboard resembles a shell command; offer safe command guidance.".to_string(),
            );
        }
    }

    hints
}

fn push_event(events: &mut VecDeque<String>, line: String) {
    events.push_front(line);
    while events.len() > MAX_EVENTS {
        let _ = events.pop_back();
    }
}

fn unix_secs(ts: SystemTime) -> u64 {
    ts.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
