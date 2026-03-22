use std::thread;
use std::time::Duration;

use arboard::Clipboard;

use crate::error::Error;
use crate::event_bus::EventBus;
use crate::signals::{ClipboardContent, ClipboardPayload, ClipboardSignal, StructuralSignal};

const POLL_INTERVAL: Duration = Duration::from_millis(250);

pub(crate) fn spawn_clipboard_monitor(bus: EventBus) -> Result<(), Error> {
    Clipboard::new().map_err(|error| Error::PlatformError(error.to_string()))?;

    thread::spawn(move || {
        let mut clipboard = match Clipboard::new() {
            Ok(clipboard) => clipboard,
            Err(_) => return,
        };

        let mut last_fingerprint: Option<String> = None;

        loop {
            if let Some((fingerprint, signal)) = read_signal(&mut clipboard) {
                let has_changed = last_fingerprint
                    .as_ref()
                    .map(|previous| previous != &fingerprint)
                    .unwrap_or(true);

                if has_changed {
                    last_fingerprint = Some(fingerprint);
                    bus.emit(StructuralSignal::Clipboard(signal));
                }
            }

            thread::sleep(POLL_INTERVAL);
        }
    });

    Ok(())
}

pub(crate) fn read_clipboard_content() -> Result<ClipboardContent, Error> {
    let mut clipboard =
        Clipboard::new().map_err(|error| Error::PlatformError(error.to_string()))?;

    if let Ok(text) = clipboard.get_text() {
        return Ok(ClipboardContent {
            payload: ClipboardPayload::Text(text),
            source_app: "unknown".to_string(),
            captured_at: std::time::SystemTime::now(),
        });
    }

    if let Ok(image) = clipboard.get_image() {
        return Ok(ClipboardContent {
            payload: ClipboardPayload::Image {
                width: image.width,
                height: image.height,
                size_bytes: image.bytes.len(),
            },
            source_app: "unknown".to_string(),
            captured_at: std::time::SystemTime::now(),
        });
    }

    Err(Error::PlatformError(
        "clipboard did not contain readable text or image data".to_string(),
    ))
}

fn read_signal(clipboard: &mut Clipboard) -> Option<(String, ClipboardSignal)> {
    if let Ok(text) = clipboard.get_text() {
        if text.trim().is_empty() {
            return None;
        }

        let fingerprint = format!("text:{}", hash_string(&text));
        let signal = ClipboardSignal::text(&text, "unknown".to_string());
        return Some((fingerprint, signal));
    }

    if let Ok(image) = clipboard.get_image() {
        let size_bytes = image.bytes.len();
        let fingerprint = format!("image:{}:{}:{}", image.width, image.height, size_bytes);
        let signal = ClipboardSignal::image(size_bytes, "unknown".to_string());
        return Some((fingerprint, signal));
    }

    None
}

fn hash_string(input: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish()
}
