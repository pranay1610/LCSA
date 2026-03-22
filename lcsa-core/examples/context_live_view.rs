use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

use lcsa_core::{ContextApi, SignalEnvelope, SignalSupport, SignalType, StructuralSignal};

const MAX_EVENTS: usize = 18;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut api = ContextApi::new()?;
    let events = Arc::new(Mutex::new(VecDeque::<String>::new()));

    for signal_type in [
        SignalType::Clipboard,
        SignalType::Selection,
        SignalType::Focus,
    ] {
        if !matches!(api.signal_support(signal_type), SignalSupport::Supported) {
            continue;
        }

        let events = Arc::clone(&events);
        api.subscribe_enveloped(signal_type, move |envelope| {
            let line = summarize_envelope(&envelope);
            let mut queue = events.lock().expect("event queue mutex poisoned");
            queue.push_front(line);
            while queue.len() > MAX_EVENTS {
                let _ = queue.pop_back();
            }
        })?;
    }

    loop {
        draw_dashboard(&api, &events)?;
        thread::sleep(Duration::from_millis(350));
    }
}

fn draw_dashboard(
    api: &ContextApi,
    events: &Arc<Mutex<VecDeque<String>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    print!("\x1B[2J\x1B[H");
    println!("LCSA Live View");
    println!(
        "device={} platform={:?}",
        api.device_context().device_name,
        api.device_context().platform
    );
    println!();
    println!("Signal Capabilities");
    for signal_type in [
        SignalType::Clipboard,
        SignalType::Selection,
        SignalType::Focus,
    ] {
        println!(
            "  {:<10} {:?}",
            format!("{signal_type:?}"),
            api.signal_support(signal_type)
        );
    }
    println!();
    println!("Recent Context Events");
    println!("  (copy text, select text, or switch windows to see updates)");

    let queue = events.lock().expect("event queue mutex poisoned");
    if queue.is_empty() {
        println!("  - waiting for signals...");
    } else {
        for line in queue.iter() {
            println!("  - {line}");
        }
    }

    io::stdout().flush()?;
    Ok(())
}

fn summarize_envelope(envelope: &SignalEnvelope) -> String {
    let secs = envelope
        .emitted_at
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();

    let summary = match &envelope.payload {
        StructuralSignal::Clipboard(signal) => format!(
            "clipboard type={:?} bytes={} source={}",
            signal.content_type, signal.size_bytes, signal.source_app
        ),
        StructuralSignal::Selection(signal) => format!(
            "selection type={:?} bytes={} source={}",
            signal.content_type, signal.size_bytes, signal.source_app
        ),
        StructuralSignal::Focus(signal) => {
            format!(
                "focus target={:?} source={}",
                signal.target, signal.source_app
            )
        }
        StructuralSignal::Filesystem(signal) => format!(
            "filesystem event={} action={:?}",
            signal.event_name(),
            signal.action
        ),
    };

    format!("{secs} {summary}")
}
