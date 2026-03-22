use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrimitiveEventKind {
    Created,
    Modified,
    Deleted,
    Renamed,
    Accessed,
    MetadataChanged,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrimitiveEvent {
    pub occurred_at: OffsetDateTime,
    pub source: String,
    pub kind: PrimitiveEventKind,
    pub paths: Vec<PathBuf>,
    pub is_directory: Option<bool>,
}

impl PrimitiveEvent {
    pub fn new(
        source: impl Into<String>,
        kind: PrimitiveEventKind,
        paths: Vec<PathBuf>,
        is_directory: Option<bool>,
        occurred_at: OffsetDateTime,
    ) -> Self {
        Self {
            occurred_at,
            source: source.into(),
            kind,
            paths,
            is_directory,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalAction {
    Created,
    Updated,
    Deleted,
    Renamed,
    Accessed,
    MetadataChanged,
    Observed,
}

impl SignalAction {
    pub fn as_str(self) -> &'static str {
        match self {
            SignalAction::Created => "created",
            SignalAction::Updated => "updated",
            SignalAction::Deleted => "deleted",
            SignalAction::Renamed => "renamed",
            SignalAction::Accessed => "accessed",
            SignalAction::MetadataChanged => "metadata_changed",
            SignalAction::Observed => "observed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Code,
    Document,
    Config,
    Data,
    Media,
    Directory,
    Archive,
    Binary,
    Unknown,
}

impl EntityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EntityKind::Code => "code",
            EntityKind::Document => "document",
            EntityKind::Config => "config",
            EntityKind::Data => "data",
            EntityKind::Media => "media",
            EntityKind::Directory => "directory",
            EntityKind::Archive => "archive",
            EntityKind::Binary => "binary",
            EntityKind::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticSignal {
    pub version: String,
    pub occurred_at: String,
    pub source: String,
    pub action: SignalAction,
    pub entity_kind: EntityKind,
    pub summary: String,
    pub confidence: f32,
    pub paths: Vec<String>,
    pub tags: Vec<String>,
    pub metadata: BTreeMap<String, Value>,
}

impl SemanticSignal {
    pub fn event_name(&self) -> String {
        format!("{}.{}", self.entity_kind.as_str(), self.action.as_str())
    }
}

pub fn normalize_event(event: &PrimitiveEvent) -> SemanticSignal {
    let path_kind = infer_entity_kind(
        event.paths.first().map(PathBuf::as_path),
        event.is_directory.unwrap_or(false),
    );

    let action = match event.kind {
        PrimitiveEventKind::Created => SignalAction::Created,
        PrimitiveEventKind::Modified => SignalAction::Updated,
        PrimitiveEventKind::Deleted => SignalAction::Deleted,
        PrimitiveEventKind::Renamed => SignalAction::Renamed,
        PrimitiveEventKind::Accessed => SignalAction::Accessed,
        PrimitiveEventKind::MetadataChanged => SignalAction::MetadataChanged,
        PrimitiveEventKind::Unknown => SignalAction::Observed,
    };

    let entity_kind = if matches!(event.is_directory, Some(true)) {
        EntityKind::Directory
    } else {
        path_kind
    };

    let paths = event
        .paths
        .iter()
        .map(|path| normalize_path(path))
        .collect::<Vec<_>>();

    let primary_path = paths
        .first()
        .cloned()
        .unwrap_or_else(|| "<unknown>".to_string());

    let summary = summarize(action, entity_kind, &paths);
    let confidence = confidence_for(entity_kind, action);

    let mut tags = Vec::new();
    if let Some(path) = event.paths.first() {
        if let Some(ext) = path.extension().and_then(|value| value.to_str()) {
            tags.push(format!("ext:{}", ext.to_ascii_lowercase()));
        }

        if let Some(topdir) = top_level_component(path) {
            tags.push(format!("topdir:{}", topdir));
        }

        if is_hidden(path) {
            tags.push("hidden:true".to_string());
        }
    }

    tags.push(format!("event:{}", action.as_str()));
    tags.push(format!("kind:{}", entity_kind.as_str()));

    let mut metadata = BTreeMap::new();
    metadata.insert(
        "event_name".to_string(),
        Value::String(format!("{}.{}", entity_kind.as_str(), action.as_str())),
    );
    metadata.insert("path_count".to_string(), Value::from(paths.len() as u64));
    metadata.insert("primary_path".to_string(), Value::String(primary_path));

    if let Some(ext) = event
        .paths
        .first()
        .and_then(|path| path.extension())
        .and_then(|value| value.to_str())
    {
        metadata.insert(
            "extension".to_string(),
            Value::String(ext.to_ascii_lowercase()),
        );
    }

    if action == SignalAction::Renamed && paths.len() >= 2 {
        metadata.insert("from_path".to_string(), Value::String(paths[0].clone()));
        metadata.insert("to_path".to_string(), Value::String(paths[1].clone()));
    }

    if let Some(is_directory) = event.is_directory {
        metadata.insert("is_directory".to_string(), Value::Bool(is_directory));
    }

    SemanticSignal {
        version: "0.1".to_string(),
        occurred_at: event
            .occurred_at
            .format(&Rfc3339)
            .unwrap_or_else(|_| event.occurred_at.unix_timestamp().to_string()),
        source: event.source.clone(),
        action,
        entity_kind,
        summary,
        confidence,
        paths,
        tags,
        metadata,
    }
}

pub fn infer_entity_kind(path: Option<&Path>, is_directory: bool) -> EntityKind {
    if is_directory {
        return EntityKind::Directory;
    }

    let Some(path) = path else {
        return EntityKind::Unknown;
    };

    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    match ext.as_deref() {
        Some(
            "rs" | "py" | "js" | "jsx" | "ts" | "tsx" | "go" | "java" | "kt" | "c" | "cc" | "cpp"
            | "h" | "hpp" | "cs" | "rb" | "php" | "swift" | "scala" | "sql" | "ipynb",
        ) => EntityKind::Code,
        Some("md" | "txt" | "pdf" | "doc" | "docx" | "rtf" | "odt" | "pages" | "rst") => {
            EntityKind::Document
        }
        Some("toml" | "yaml" | "yml" | "ini" | "env" | "conf" | "cfg" | "xml") => {
            EntityKind::Config
        }
        Some("json" | "csv" | "tsv" | "parquet" | "feather" | "sqlite" | "db") => EntityKind::Data,
        Some(
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "mp4" | "mov" | "mp3" | "wav"
            | "flac",
        ) => EntityKind::Media,
        Some("zip" | "tar" | "gz" | "bz2" | "xz" | "7z") => EntityKind::Archive,
        Some("bin" | "exe" | "so" | "dylib" | "dll") => EntityKind::Binary,
        _ => infer_from_name(path),
    }
}

fn infer_from_name(path: &Path) -> EntityKind {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();

    if matches!(
        name.as_str(),
        "cargo.toml"
            | "cargo.lock"
            | "package.json"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "dockerfile"
            | ".env"
            | ".gitignore"
            | "makefile"
    ) {
        return EntityKind::Config;
    }

    if name == "readme" || name.starts_with("readme.") || name.starts_with("license") {
        return EntityKind::Document;
    }

    EntityKind::Unknown
}

fn summarize(action: SignalAction, entity_kind: EntityKind, paths: &[String]) -> String {
    let noun = match entity_kind {
        EntityKind::Code => "Code file",
        EntityKind::Document => "Document",
        EntityKind::Config => "Config file",
        EntityKind::Data => "Data file",
        EntityKind::Media => "Media asset",
        EntityKind::Directory => "Directory",
        EntityKind::Archive => "Archive",
        EntityKind::Binary => "Binary artifact",
        EntityKind::Unknown => "File",
    };

    match action {
        SignalAction::Renamed if paths.len() >= 2 => {
            format!("{} renamed: {} -> {}", noun, paths[0], paths[1])
        }
        SignalAction::Created => format!("{} created: {}", noun, first_or_unknown(paths)),
        SignalAction::Updated => format!("{} updated: {}", noun, first_or_unknown(paths)),
        SignalAction::Deleted => format!("{} deleted: {}", noun, first_or_unknown(paths)),
        SignalAction::Accessed => format!("{} accessed: {}", noun, first_or_unknown(paths)),
        SignalAction::MetadataChanged => {
            format!("{} metadata changed: {}", noun, first_or_unknown(paths))
        }
        SignalAction::Observed => format!("{} observed: {}", noun, first_or_unknown(paths)),
        SignalAction::Renamed => format!("{} renamed", noun),
    }
}

fn first_or_unknown(paths: &[String]) -> &str {
    paths.first().map(String::as_str).unwrap_or("<unknown>")
}

fn confidence_for(entity_kind: EntityKind, action: SignalAction) -> f32 {
    let entity_score: f32 = match entity_kind {
        EntityKind::Unknown => 0.65,
        EntityKind::Directory => 0.92,
        EntityKind::Config => 0.97,
        EntityKind::Code => 0.98,
        _ => 0.95,
    };

    let action_adjustment: f32 = match action {
        SignalAction::Observed => -0.12,
        SignalAction::MetadataChanged => -0.06,
        _ => 0.0,
    };

    (entity_score + action_adjustment).clamp(0.0, 1.0)
}

fn normalize_path(path: &Path) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    if raw.is_empty() { ".".to_string() } else { raw }
}

fn top_level_component(path: &Path) -> Option<String> {
    path.components()
        .next()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
}

fn is_hidden(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .map(|segment| segment.starts_with('.') && segment.len() > 1)
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use time::macros::datetime;

    #[test]
    fn classifies_rust_file_as_code() {
        let kind = infer_entity_kind(Some(Path::new("src/lib.rs")), false);
        assert_eq!(kind, EntityKind::Code);
    }

    #[test]
    fn classifies_readme_without_extension_as_document() {
        let kind = infer_entity_kind(Some(Path::new("README")), false);
        assert_eq!(kind, EntityKind::Document);
    }

    #[test]
    fn emits_rename_signal_with_both_paths() {
        let event = PrimitiveEvent::new(
            "filesystem",
            PrimitiveEventKind::Renamed,
            vec![
                PathBuf::from("notes/todo.md"),
                PathBuf::from("notes/done.md"),
            ],
            Some(false),
            datetime!(2026-03-22 10:12:05 UTC),
        );

        let signal = normalize_event(&event);

        assert_eq!(signal.entity_kind, EntityKind::Document);
        assert_eq!(signal.action, SignalAction::Renamed);
        assert_eq!(signal.event_name(), "document.renamed");
        assert_eq!(
            signal.summary,
            "Document renamed: notes/todo.md -> notes/done.md"
        );
        assert_eq!(
            signal.metadata.get("from_path"),
            Some(&Value::String("notes/todo.md".to_string()))
        );
        assert_eq!(
            signal.metadata.get("to_path"),
            Some(&Value::String("notes/done.md".to_string()))
        );
    }

    #[test]
    fn tags_hidden_config_file() {
        let event = PrimitiveEvent::new(
            "filesystem",
            PrimitiveEventKind::Modified,
            vec![PathBuf::from(".env")],
            Some(false),
            datetime!(2026-03-22 10:12:05 UTC),
        );

        let signal = normalize_event(&event);

        assert_eq!(signal.entity_kind, EntityKind::Config);
        assert!(signal.tags.iter().any(|tag| tag == "hidden:true"));
        assert!(signal.tags.iter().any(|tag| tag == "event:updated"));
    }
}
