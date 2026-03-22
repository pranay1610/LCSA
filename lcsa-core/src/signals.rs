use std::fmt;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::filesystem::SemanticSignal;
use crate::topology::SignalSource;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    Clipboard,
    Selection,
    Focus,
}

impl SignalType {
    pub fn as_str(self) -> &'static str {
        match self {
            SignalType::Clipboard => "clipboard",
            SignalType::Selection => "selection",
            SignalType::Focus => "focus",
        }
    }
}

impl fmt::Display for SignalType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    Text,
    Image,
    Html,
    Code,
    Unknown,
}

impl ContentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContentType::Text => "text",
            ContentType::Image => "image",
            ContentType::Html => "html",
            ContentType::Code => "code",
            ContentType::Unknown => "unknown",
        }
    }
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipboardSignal {
    pub content_type: ContentType,
    pub size_bytes: usize,
    pub source_app: String,
    pub likely_sensitive: bool,
    pub likely_command: bool,
    pub timestamp: SystemTime,
}

impl ClipboardSignal {
    pub fn text(content: &str, source_app: String) -> Self {
        Self {
            content_type: detect_content_type(content),
            size_bytes: content.len(),
            source_app,
            likely_sensitive: is_likely_sensitive_text(content),
            likely_command: is_likely_command_text(content),
            timestamp: SystemTime::now(),
        }
    }

    pub fn image(size_bytes: usize, source_app: String) -> Self {
        Self {
            content_type: ContentType::Image,
            size_bytes,
            source_app,
            likely_sensitive: false,
            likely_command: false,
            timestamp: SystemTime::now(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionSignal {
    pub content_type: ContentType,
    pub size_bytes: usize,
    pub source_app: String,
    pub likely_sensitive: bool,
    pub is_editable: bool,
    pub timestamp: SystemTime,
}

impl SelectionSignal {
    pub fn text(content: &str, source_app: String, is_editable: bool) -> Self {
        Self {
            content_type: detect_content_type(content),
            size_bytes: content.len(),
            source_app,
            likely_sensitive: is_likely_sensitive_text(content),
            is_editable,
            timestamp: SystemTime::now(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusTarget {
    Application,
    Window,
    TextInput,
    Browser,
    Terminal,
    Unknown,
}

impl FocusTarget {
    pub fn as_str(&self) -> &'static str {
        match self {
            FocusTarget::Application => "application",
            FocusTarget::Window => "window",
            FocusTarget::TextInput => "text_input",
            FocusTarget::Browser => "browser",
            FocusTarget::Terminal => "terminal",
            FocusTarget::Unknown => "unknown",
        }
    }
}

impl fmt::Display for FocusTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusSignal {
    pub source_app: String,
    pub target: FocusTarget,
    pub is_editable: bool,
    pub timestamp: SystemTime,
}

impl FocusSignal {
    pub fn new(source_app: String, target: FocusTarget, is_editable: bool) -> Self {
        Self {
            source_app,
            target,
            is_editable,
            timestamp: SystemTime::now(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipboardContent {
    pub payload: ClipboardPayload,
    pub source_app: String,
    pub captured_at: SystemTime,
}

impl ClipboardContent {
    pub fn redacted_preview(&self) -> String {
        match &self.payload {
            ClipboardPayload::Text(text) if is_likely_sensitive_text(text) => {
                format!("{} chars redacted", text.chars().count())
            }
            ClipboardPayload::Text(text) => text.chars().take(80).collect(),
            ClipboardPayload::Image {
                width,
                height,
                size_bytes,
            } => format!("image {}x{} ({} bytes)", width, height, size_bytes),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClipboardPayload {
    Text(String),
    Image {
        width: usize,
        height: usize,
        size_bytes: usize,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "signal", rename_all = "snake_case")]
pub enum StructuralSignal {
    Clipboard(ClipboardSignal),
    Selection(SelectionSignal),
    Focus(FocusSignal),
    Filesystem(SemanticSignal),
}

impl StructuralSignal {
    pub fn signal_type(&self) -> Option<SignalType> {
        match self {
            StructuralSignal::Clipboard(_) => Some(SignalType::Clipboard),
            StructuralSignal::Selection(_) => Some(SignalType::Selection),
            StructuralSignal::Focus(_) => Some(SignalType::Focus),
            StructuralSignal::Filesystem(_) => None,
        }
    }

    pub fn source(&self) -> SignalSource {
        match self {
            StructuralSignal::Clipboard(_) => SignalSource::Clipboard,
            StructuralSignal::Selection(_) => SignalSource::Selection,
            StructuralSignal::Focus(_) => SignalSource::Focus,
            StructuralSignal::Filesystem(_) => SignalSource::Filesystem,
        }
    }

    pub fn matches(&self, signal_type: SignalType) -> bool {
        self.signal_type() == Some(signal_type)
    }
}

pub fn detect_content_type(content: &str) -> ContentType {
    let trimmed = content.trim();
    let lowercase = trimmed.to_ascii_lowercase();

    if lowercase.starts_with("<!doctype html")
        || lowercase.starts_with("<html")
        || (lowercase.contains("<body") && lowercase.contains("</"))
    {
        return ContentType::Html;
    }

    let code_markers = [
        "fn ",
        "def ",
        "class ",
        "import ",
        "from ",
        "const ",
        "let ",
        "var ",
        "function ",
        "#include",
        "SELECT ",
        "{\n",
    ];

    if code_markers.iter().any(|marker| trimmed.contains(marker))
        || (trimmed.lines().count() > 2
            && trimmed.contains('{')
            && trimmed.contains('}')
            && trimmed.contains(';'))
    {
        return ContentType::Code;
    }

    if trimmed.is_empty() {
        ContentType::Unknown
    } else {
        ContentType::Text
    }
}

pub fn is_likely_sensitive_text(content: &str) -> bool {
    let trimmed = content.trim();

    if trimmed.is_empty() || trimmed.contains('\n') {
        return false;
    }

    let jwt_like = trimmed.matches('.').count() == 2 && trimmed.len() > 20;
    let token_prefix = ["sk-", "ghp_", "xoxb-", "AKIA", "-----BEGIN", "eyJ"];

    // Require higher entropy to avoid false positives like "myFile123" or "config_v2"
    // Real secrets/tokens have high entropy (5.0+ bits), normal text has ~4.0-4.5 bits
    let looks_like_secret = trimmed.len() >= 16
        && !trimmed.contains(' ')
        && trimmed.chars().any(|c| c.is_ascii_alphabetic())
        && trimmed.chars().any(|c| c.is_ascii_digit())
        && shannon_entropy(trimmed) > 4.0;

    jwt_like
        || token_prefix
            .iter()
            .any(|prefix| trimmed.starts_with(prefix))
        || looks_like_secret
}

fn shannon_entropy(input: &str) -> f64 {
    if input.is_empty() {
        return 0.0;
    }

    let mut frequency = [0u32; 256];
    for byte in input.bytes() {
        frequency[byte as usize] += 1;
    }

    let len = input.len() as f64;
    frequency
        .iter()
        .filter(|&&count| count > 0)
        .map(|&count| {
            let p = count as f64 / len;
            -p * p.log2()
        })
        .sum()
}

pub fn is_likely_command_text(content: &str) -> bool {
    let trimmed = content.trim();

    if trimmed.is_empty() || trimmed.contains('\n') {
        return false;
    }

    let normalized = trimmed.strip_prefix("$ ").unwrap_or(trimmed);
    let command_prefixes = [
        "cargo ", "git ", "npm ", "pnpm ", "yarn ", "python ", "pip ", "uv ", "docker ",
        "kubectl ", "ls", "cd ", "mkdir ", "rm ", "cp ", "mv ",
    ];

    command_prefixes
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_html() {
        assert_eq!(
            detect_content_type("<!DOCTYPE html><html></html>"),
            ContentType::Html
        );
    }

    #[test]
    fn detects_code() {
        assert_eq!(
            detect_content_type("fn main() {\n println!(\"hi\");\n}"),
            ContentType::Code
        );
    }

    #[test]
    fn detects_plain_text() {
        assert_eq!(detect_content_type("hello world"), ContentType::Text);
    }

    #[test]
    fn marks_sensitive_tokens() {
        assert!(is_likely_sensitive_text("sk-live-1234567890abcdef"));
    }

    #[test]
    fn rejects_false_positive_filenames() {
        // These were triggering false positives before entropy check
        assert!(!is_likely_sensitive_text("myFile123"));
        assert!(!is_likely_sensitive_text("config_v2"));
        assert!(!is_likely_sensitive_text("user2024"));
        assert!(!is_likely_sensitive_text("version1.0"));
        assert!(!is_likely_sensitive_text("data_backup_01"));
    }

    #[test]
    fn detects_high_entropy_secrets() {
        // Real secrets have high entropy
        assert!(is_likely_sensitive_text(
            "ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ123456"
        ));
        assert!(is_likely_sensitive_text(
            "sk-proj-abcdefghijklmnop1234567890"
        ));
        // JWT-like tokens
        assert!(is_likely_sensitive_text(
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U"
        ));
    }

    #[test]
    fn shannon_entropy_values() {
        // Low entropy - repeated characters
        assert!(shannon_entropy("aaaaaaaaaa") < 1.0);
        // Medium entropy - normal text
        let medium = shannon_entropy("hello_world");
        assert!(medium > 2.0 && medium < 4.0);
        // High entropy - random-looking
        let high = shannon_entropy("aB3xK9mP2qR7sT4uV8wY1zC5dE6fG0hI");
        assert!(high > 4.5);
    }

    #[test]
    fn marks_commands() {
        assert!(is_likely_command_text("cargo test"));
    }

    #[test]
    fn selection_signal_tracks_text_metadata() {
        let signal = SelectionSignal::text("let value = 1;", "editor".to_string(), true);
        assert_eq!(signal.content_type, ContentType::Code);
        assert!(signal.is_editable);
        assert_eq!(signal.size_bytes, 14);
    }

    #[test]
    fn structural_signal_maps_to_source() {
        assert_eq!(
            StructuralSignal::Focus(FocusSignal::new(
                "terminal".to_string(),
                FocusTarget::Terminal,
                true,
            ))
            .source(),
            SignalSource::Focus
        );
    }
}
