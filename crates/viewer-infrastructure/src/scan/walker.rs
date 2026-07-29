use async_trait::async_trait;
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use viewer_application::{
    ScanPort,
    scan::{
        SCAN_BATCH_MAX_LATENCY_MS, SCAN_BATCH_SIZE, ScanError, ScanEvent, ScanRequest, ScanSink,
        ScanTotals,
    },
};
use viewer_domain::{
    EntityId, RelativePath,
    file::{FileKind, FileNode},
};
use walkdir::{DirEntry, WalkDir};

use super::file_classifier::{classify_regular_file, is_ignored_entry_name};

#[derive(Debug, thiserror::Error)]
pub enum SubtreeSnapshotError {
    #[error("reconcile root is invalid")]
    InvalidRoot,
}

pub(crate) struct SubtreeSnapshot {
    pub scopes: Vec<Option<RelativePath>>,
    pub nodes: Vec<FileNode>,
    pub protected: Vec<Option<RelativePath>>,
    pub failed: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProjectWalker;

#[async_trait]
impl ScanPort for ProjectWalker {
    async fn scan(&self, request: ScanRequest, sink: ScanSink) -> Result<(), ScanError> {
        tokio::task::spawn_blocking(move || scan_blocking(request, sink))
            .await
            .map_err(|error| ScanError::RootUnreadable(error.to_string()))?
    }
}

fn scan_blocking(request: ScanRequest, sink: ScanSink) -> Result<(), ScanError> {
    let root = validate_root(&request.root)?;
    let generation = request.generation;
    let _session_id = request.session_id;
    let mut totals = ScanTotals::default();
    let mut folder_batch = TimedBatch::new();
    let mut file_batch = TimedBatch::new();
    let mut queued_file_batches = VecDeque::new();

    let entries = WalkDir::new(&request.root)
        .follow_links(false)
        .into_iter()
        .filter_entry(is_visible_entry);
    for result in entries {
        if let Some(nodes) = folder_batch.take_if_due() {
            send(&sink, ScanEvent::Folders { generation, nodes })?;
        }
        let entry = match result {
            Ok(entry) => entry,
            Err(error) => {
                totals.failed += 1;
                send(
                    &sink,
                    ScanEvent::FailedItem {
                        generation,
                        relative_display: error
                            .path()
                            .and_then(|path| path.strip_prefix(&request.root).ok())
                            .unwrap_or_else(|| error.path().unwrap_or(Path::new("<unknown>")))
                            .to_string_lossy()
                            .into_owned(),
                        code: "walk_error".into(),
                    },
                )?;
                continue;
            }
        };
        if entry.depth() == 0 || entry.file_type().is_symlink() {
            continue;
        }
        match node_from_entry(&root, &request.root, &entry) {
            Ok(Some(node)) if node.kind == FileKind::Directory => {
                totals.folders += 1;
                if let Some(nodes) = folder_batch.push(node) {
                    send(&sink, ScanEvent::Folders { generation, nodes })?;
                }
            }
            Ok(Some(node)) => {
                totals.files += 1;
                if let Some(nodes) = file_batch.push(node) {
                    queued_file_batches.push_back(nodes);
                }
            }
            Ok(None) => {}
            Err((relative_display, code)) => {
                totals.failed += 1;
                send(
                    &sink,
                    ScanEvent::FailedItem {
                        generation,
                        relative_display,
                        code,
                    },
                )?;
            }
        }
    }

    if let Some(nodes) = folder_batch.finish() {
        send(&sink, ScanEvent::Folders { generation, nodes })?;
    }
    if let Some(nodes) = file_batch.finish() {
        queued_file_batches.push_back(nodes);
    }
    while let Some(nodes) = queued_file_batches.pop_front() {
        send(&sink, ScanEvent::Files { generation, nodes })?;
    }
    send(&sink, ScanEvent::Finished { generation, totals })
}

pub(crate) fn snapshot_subtrees(
    project_root: &Path,
    requested_roots: &[PathBuf],
) -> Result<SubtreeSnapshot, SubtreeSnapshotError> {
    let project_root =
        std::fs::canonicalize(project_root).map_err(|_| SubtreeSnapshotError::InvalidRoot)?;
    if requested_roots.is_empty() || !project_root.is_dir() {
        return Err(SubtreeSnapshotError::InvalidRoot);
    }
    let mut roots = requested_roots
        .iter()
        .map(|requested| validate_subtree_root(&project_root, requested))
        .collect::<Result<Vec<_>, _>>()?;
    roots.sort_by(|left, right| left.0.cmp(&right.0));
    roots.dedup_by(|left, right| left.0 == right.0);
    let mut minimized = Vec::<(PathBuf, Option<RelativePath>, bool)>::new();
    for root in roots {
        if minimized
            .iter()
            .any(|(parent, _, _)| root.0.starts_with(parent))
        {
            continue;
        }
        minimized.push(root);
    }

    let mut nodes = Vec::new();
    let mut protected = Vec::new();
    let mut failed = 0_u64;
    for (root, scope, exists) in &minimized {
        if !exists {
            continue;
        }
        let entries = WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(is_visible_entry);
        for result in entries {
            let entry = match result {
                Ok(entry) => entry,
                Err(error) => {
                    failed = failed.saturating_add(1);
                    protected.push(
                        error
                            .path()
                            .and_then(|path| relative_scope(&project_root, path))
                            .unwrap_or_else(|| scope.clone()),
                    );
                    continue;
                }
            };
            if (entry.depth() == 0 && root == &project_root) || entry.file_type().is_symlink() {
                continue;
            }
            match node_from_entry(&project_root, &project_root, &entry) {
                Ok(Some(node)) => nodes.push(node),
                Ok(None) => {}
                Err((display, _)) => {
                    failed = failed.saturating_add(1);
                    protected.push(
                        RelativePath::parse(&display)
                            .ok()
                            .map(Some)
                            .unwrap_or_else(|| scope.clone()),
                    );
                }
            }
        }
    }
    nodes.sort_by(|left, right| {
        let left_depth = left.relative_path.as_str().matches('/').count();
        let right_depth = right.relative_path.as_str().matches('/').count();
        left_depth.cmp(&right_depth).then_with(|| {
            left.relative_path
                .as_str()
                .cmp(right.relative_path.as_str())
        })
    });
    Ok(SubtreeSnapshot {
        scopes: minimized.into_iter().map(|(_, scope, _)| scope).collect(),
        nodes,
        protected,
        failed,
    })
}

fn validate_subtree_root(
    project_root: &Path,
    requested: &Path,
) -> Result<(PathBuf, Option<RelativePath>, bool), SubtreeSnapshotError> {
    let normalized = normalize_absolute(requested).ok_or(SubtreeSnapshotError::InvalidRoot)?;
    let relative = normalized
        .strip_prefix(project_root)
        .map_err(|_| SubtreeSnapshotError::InvalidRoot)?;
    if relative.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_none_or(|name| name.starts_with('.') || name.eq_ignore_ascii_case(".viewer"))
    }) {
        return Err(SubtreeSnapshotError::InvalidRoot);
    }
    let scope = if relative.as_os_str().is_empty() {
        None
    } else {
        Some(
            relative
                .to_str()
                .and_then(|value| RelativePath::parse(value).ok())
                .ok_or(SubtreeSnapshotError::InvalidRoot)?,
        )
    };
    let mut ancestor = normalized.as_path();
    let exists = loop {
        match std::fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink()
                    || !metadata.is_dir()
                    || is_macos_alias(ancestor)
                {
                    return Err(SubtreeSnapshotError::InvalidRoot);
                }
                let canonical = std::fs::canonicalize(ancestor)
                    .map_err(|_| SubtreeSnapshotError::InvalidRoot)?;
                if canonical != ancestor || !canonical.starts_with(project_root) {
                    return Err(SubtreeSnapshotError::InvalidRoot);
                }
                break ancestor == normalized;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ancestor = ancestor.parent().ok_or(SubtreeSnapshotError::InvalidRoot)?;
            }
            Err(_) => return Err(SubtreeSnapshotError::InvalidRoot),
        }
    };
    Ok((normalized, scope, exists))
}

fn relative_scope(project_root: &Path, path: &Path) -> Option<Option<RelativePath>> {
    let relative = path.strip_prefix(project_root).ok()?;
    if relative.as_os_str().is_empty() {
        Some(None)
    } else {
        relative
            .to_str()
            .and_then(|value| RelativePath::parse(value).ok())
            .map(Some)
    }
}

fn normalize_absolute(path: &Path) -> Option<PathBuf> {
    use std::path::Component;
    if !path.is_absolute() {
        return None;
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(segment) => normalized.push(segment),
        }
    }
    Some(normalized)
}

fn validate_root(root: &Path) -> Result<PathBuf, ScanError> {
    let metadata = std::fs::symlink_metadata(root)
        .map_err(|error| ScanError::RootUnreadable(error.to_string()))?;
    if metadata.file_type().is_symlink() {
        return Err(ScanError::RootUnreadable(
            "project root cannot be a symbolic link".into(),
        ));
    }
    if !metadata.is_dir() || is_macos_alias(root) {
        return Err(ScanError::RootUnreadable("path is not a directory".into()));
    }
    std::fs::read_dir(root).map_err(|error| ScanError::RootUnreadable(error.to_string()))?;
    std::fs::canonicalize(root).map_err(|error| ScanError::RootUnreadable(error.to_string()))
}

fn is_visible_entry(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    !is_ignored_entry_name(entry.file_name())
}

fn node_from_entry(
    canonical_root: &Path,
    display_root: &Path,
    entry: &DirEntry,
) -> Result<Option<FileNode>, (String, String)> {
    let path = entry.path();
    let display = path
        .strip_prefix(display_root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned();
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| (display.clone(), "metadata_unreadable".into()))?;
    if metadata.file_type().is_symlink() || is_macos_alias(path) {
        return Ok(None);
    }
    let parent = path
        .parent()
        .ok_or_else(|| (display.clone(), "outside_project".into()))?;
    let canonical_parent =
        std::fs::canonicalize(parent).map_err(|_| (display.clone(), "parent_unreadable".into()))?;
    if !canonical_parent.starts_with(canonical_root) {
        return Err((display, "outside_project".into()));
    }
    let relative = path
        .strip_prefix(display_root)
        .ok()
        .and_then(Path::to_str)
        .and_then(|value| RelativePath::parse(value).ok())
        .ok_or_else(|| (display.clone(), "invalid_relative_path".into()))?;
    let kind = if metadata.is_dir() {
        FileKind::Directory
    } else if metadata.is_file() {
        let Some(kind) = classify_regular_file(path) else {
            return Ok(None);
        };
        kind
    } else {
        return Ok(None);
    };
    Ok(Some(FileNode {
        entity_id: entity_id(&metadata, &relative),
        relative_path: relative,
        kind,
        size: if kind == FileKind::Directory {
            0
        } else {
            metadata.len()
        },
        modified_ns: modified_ns(&metadata),
    }))
}

fn send(sink: &ScanSink, event: ScanEvent) -> Result<(), ScanError> {
    sink.blocking_send(event).map_err(|_| ScanError::Cancelled)
}

struct TimedBatch {
    nodes: Vec<FileNode>,
    started: Instant,
}

impl TimedBatch {
    fn new() -> Self {
        Self {
            nodes: Vec::with_capacity(SCAN_BATCH_SIZE),
            started: Instant::now(),
        }
    }

    fn push(&mut self, node: FileNode) -> Option<Vec<FileNode>> {
        self.nodes.push(node);
        let elapsed = self.started.elapsed();
        if self.nodes.len() >= SCAN_BATCH_SIZE
            || elapsed >= Duration::from_millis(SCAN_BATCH_MAX_LATENCY_MS)
        {
            self.take()
        } else {
            None
        }
    }

    fn finish(&mut self) -> Option<Vec<FileNode>> {
        self.take()
    }

    fn take_if_due(&mut self) -> Option<Vec<FileNode>> {
        (self.started.elapsed() >= Duration::from_millis(SCAN_BATCH_MAX_LATENCY_MS))
            .then(|| self.take())
            .flatten()
    }

    fn take(&mut self) -> Option<Vec<FileNode>> {
        if self.nodes.is_empty() {
            return None;
        }
        self.started = Instant::now();
        Some(std::mem::replace(
            &mut self.nodes,
            Vec::with_capacity(SCAN_BATCH_SIZE),
        ))
    }
}

#[cfg(unix)]
fn entity_id(metadata: &std::fs::Metadata, _relative: &RelativePath) -> EntityId {
    use std::os::unix::fs::MetadataExt;
    EntityId::from_u128((u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()))
}

#[cfg(not(unix))]
fn entity_id(metadata: &std::fs::Metadata, relative: &RelativePath) -> EntityId {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    relative.as_str().hash(&mut hasher);
    metadata.len().hash(&mut hasher);
    EntityId::from_u128(u128::from(hasher.finish()))
}

#[cfg(unix)]
fn modified_ns(metadata: &std::fs::Metadata) -> i128 {
    use std::os::unix::fs::MetadataExt;
    i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec())
}

#[cfg(not(unix))]
fn modified_ns(metadata: &std::fs::Metadata) -> i128 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos() as i128)
}

#[cfg(target_os = "macos")]
pub fn is_macos_alias(path: &Path) -> bool {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    let name = c"com.apple.FinderInfo";
    let mut finder_info = [0_u8; 32];
    // SAFETY: Both C strings are NUL terminated and `finder_info` is a valid
    // writable buffer. This read-only query does not follow directory entries.
    let read = unsafe {
        libc::getxattr(
            path.as_ptr(),
            name.as_ptr(),
            finder_info.as_mut_ptr().cast(),
            finder_info.len(),
            0,
            0,
        )
    };
    read >= 10 && u16::from_be_bytes([finder_info[8], finder_info[9]]) & 0x8000 != 0
}

#[cfg(not(target_os = "macos"))]
pub fn is_macos_alias(_path: &Path) -> bool {
    false
}
