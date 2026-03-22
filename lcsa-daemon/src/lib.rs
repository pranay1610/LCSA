use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use lcsa_core::{PrimitiveEvent, PrimitiveEventKind};
use notify::event::{AccessKind, CreateKind, MetadataKind, ModifyKind, RemoveKind, RenameMode};
use notify::{Event, EventKind};
use time::OffsetDateTime;
use walkdir::WalkDir;

pub fn snapshot_events(
    path: &Path,
    include_hidden: bool,
    skip_path: Option<&Path>,
) -> Result<Vec<PrimitiveEvent>> {
    let mut events = Vec::new();

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|entry| include_hidden || !is_hidden_within(entry.path(), Some(path)))
    {
        let entry = entry.with_context(|| format!("failed to traverse {}", path.display()))?;
        let item_path = entry.path();

        if item_path == path {
            continue;
        }

        if should_skip(item_path, skip_path) {
            continue;
        }

        events.push(PrimitiveEvent::new(
            "filesystem",
            PrimitiveEventKind::Created,
            vec![item_path.to_path_buf()],
            Some(entry.file_type().is_dir()),
            OffsetDateTime::now_utc(),
        ));
    }

    Ok(events)
}

pub fn primitive_from_notify_event(
    event: Event,
    watch_root: &Path,
    include_hidden: bool,
    skip_path: Option<&Path>,
) -> Option<PrimitiveEvent> {
    if !include_hidden
        && event
            .paths
            .iter()
            .any(|path| is_hidden_within(path, Some(watch_root)))
    {
        return None;
    }

    if event.paths.iter().all(|path| should_skip(path, skip_path)) {
        return None;
    }

    let kind = map_event_kind(&event.kind);
    let is_directory = event
        .paths
        .first()
        .and_then(|path| infer_directory_flag(path.as_path()));

    Some(PrimitiveEvent::new(
        "filesystem",
        kind,
        event.paths,
        is_directory,
        OffsetDateTime::now_utc(),
    ))
}

pub fn should_skip(path: &Path, skip_path: Option<&Path>) -> bool {
    match skip_path {
        Some(skip_path) => same_path(path, skip_path),
        None => false,
    }
}

fn map_event_kind(kind: &EventKind) -> PrimitiveEventKind {
    match kind {
        EventKind::Create(CreateKind::Any)
        | EventKind::Create(CreateKind::File)
        | EventKind::Create(CreateKind::Folder)
        | EventKind::Create(CreateKind::Other) => PrimitiveEventKind::Created,
        EventKind::Modify(ModifyKind::Data(_))
        | EventKind::Modify(ModifyKind::Any)
        | EventKind::Modify(ModifyKind::Other)
        | EventKind::Modify(ModifyKind::Name(RenameMode::Any))
        | EventKind::Modify(ModifyKind::Name(RenameMode::From))
        | EventKind::Modify(ModifyKind::Name(RenameMode::To)) => PrimitiveEventKind::Modified,
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => PrimitiveEventKind::Renamed,
        EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::Any
            | MetadataKind::WriteTime
            | MetadataKind::Permissions
            | MetadataKind::Ownership
            | MetadataKind::Extended
            | MetadataKind::Other,
        )) => PrimitiveEventKind::MetadataChanged,
        EventKind::Remove(RemoveKind::Any)
        | EventKind::Remove(RemoveKind::File)
        | EventKind::Remove(RemoveKind::Folder)
        | EventKind::Remove(RemoveKind::Other) => PrimitiveEventKind::Deleted,
        EventKind::Access(AccessKind::Any)
        | EventKind::Access(AccessKind::Open(_))
        | EventKind::Access(AccessKind::Close(_))
        | EventKind::Access(AccessKind::Read)
        | EventKind::Access(AccessKind::Other) => PrimitiveEventKind::Accessed,
        _ => PrimitiveEventKind::Unknown,
    }
}

fn infer_directory_flag(path: &Path) -> Option<bool> {
    std::fs::metadata(path)
        .ok()
        .map(|metadata| metadata.is_dir())
}

fn is_hidden_within(path: &Path, root: Option<&Path>) -> bool {
    let relative = root
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);

    relative.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .map(|segment| segment.starts_with('.') && segment.len() > 1)
            .unwrap_or(false)
    })
}

fn same_path(left: &Path, right: &Path) -> bool {
    canonicalize_lossy(left)
        .map(|candidate| candidate == right)
        .unwrap_or_else(|| left == right)
}

fn canonicalize_lossy(path: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok().or_else(|| {
        if path.is_absolute() {
            Some(path.to_path_buf())
        } else {
            std::env::current_dir().ok().map(|cwd| cwd.join(path))
        }
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use lcsa_core::normalize_event;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn snapshot_emits_code_signal_for_rust_file() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join("src")).expect("create src");
        fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").expect("write rust file");

        let events = snapshot_events(dir.path(), false, None).expect("snapshot events");
        let signals = events.iter().map(normalize_event).collect::<Vec<_>>();

        assert!(
            signals
                .iter()
                .any(|signal| signal.event_name() == "code.created")
        );
    }

    #[test]
    fn snapshot_skips_hidden_files_by_default() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join(".env"), "SECRET=1\n").expect("write hidden file");
        fs::write(dir.path().join("README.md"), "hello\n").expect("write visible file");

        let events = snapshot_events(dir.path(), false, None).expect("snapshot events");
        let paths = events
            .iter()
            .flat_map(|event| event.paths.iter())
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(paths.iter().all(|path| !path.ends_with(".env")));
        assert!(paths.iter().any(|path| path.ends_with("README.md")));
    }
}
