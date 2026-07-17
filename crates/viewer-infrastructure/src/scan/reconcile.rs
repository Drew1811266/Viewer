use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
};
use viewer_application::watcher::{
    FileIdentity, ReconcileReason, ReconcileRequest, WATCHER_DEBOUNCE_MS, WatcherEvent,
    WatcherEventKind,
};
use viewer_domain::{OperationId, SessionId, search::Generation};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedChange {
    pub operation_id: OperationId,
    pub old_canonical_path: PathBuf,
    pub new_canonical_path: PathBuf,
    pub expected_identity: FileIdentity,
    pub expires_at_ms: u64,
}

#[derive(Debug, Default)]
pub struct ExpectedChangeLedger {
    changes: Vec<ExpectedChange>,
}

impl ExpectedChangeLedger {
    pub fn register(&mut self, change: ExpectedChange) {
        self.changes.push(change);
    }

    fn matching_operation(
        &mut self,
        event: &WatcherEvent,
        observed_at_ms: u64,
    ) -> Option<OperationId> {
        self.changes
            .retain(|change| change.expires_at_ms >= observed_at_ms);
        let identity = event.identity?;
        self.changes
            .iter()
            .find(|change| {
                change.expected_identity == identity
                    && !event.paths.is_empty()
                    && event.paths.iter().all(|path| {
                        path == &change.old_canonical_path || path == &change.new_canonical_path
                    })
            })
            .map(|change| change.operation_id)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReconcileError {
    #[error("project watcher root is invalid: {0}")]
    InvalidRoot(String),
}

struct PendingEvent {
    event: WatcherEvent,
    expected_operation: Option<OperationId>,
}

pub struct ReconcilePlanner {
    project_root: PathBuf,
    session_id: SessionId,
    generation: Generation,
    ledger: ExpectedChangeLedger,
    pending: Vec<PendingEvent>,
    last_event_ms: Option<u64>,
}

impl ReconcilePlanner {
    pub fn new(
        project_root: PathBuf,
        session_id: SessionId,
        generation: Generation,
        ledger: ExpectedChangeLedger,
    ) -> Result<Self, ReconcileError> {
        let project_root = std::fs::canonicalize(&project_root)
            .map_err(|error| ReconcileError::InvalidRoot(error.to_string()))?;
        if !project_root.is_dir() {
            return Err(ReconcileError::InvalidRoot(
                "watcher root is not a directory".into(),
            ));
        }
        Ok(Self {
            project_root,
            session_id,
            generation,
            ledger,
            pending: Vec::new(),
            last_event_ms: None,
        })
    }

    pub fn push(
        &mut self,
        mut event: WatcherEvent,
        observed_at_ms: u64,
    ) -> Option<ReconcileRequest> {
        let completed = self
            .last_event_ms
            .filter(|last| observed_at_ms.saturating_sub(*last) >= WATCHER_DEBOUNCE_MS)
            .and_then(|_| self.take_request());
        event.paths = event
            .paths
            .into_iter()
            .filter_map(|path| self.accepted_path(&path))
            .collect();
        if event.paths.is_empty() && event.kind != WatcherEventKind::Overflow {
            return completed;
        }
        if event.kind == WatcherEventKind::Overflow && event.paths.is_empty() {
            event.paths.push(self.project_root.clone());
        }
        let expected_operation = self.ledger.matching_operation(&event, observed_at_ms);
        self.pending.push(PendingEvent {
            event,
            expected_operation,
        });
        self.last_event_ms = Some(observed_at_ms);
        completed
    }

    pub fn flush(&mut self, now_ms: u64) -> Option<ReconcileRequest> {
        let ready = self
            .last_event_ms
            .is_some_and(|last| now_ms.saturating_sub(last) >= WATCHER_DEBOUNCE_MS);
        ready.then(|| self.take_request()).flatten()
    }

    fn take_request(&mut self) -> Option<ReconcileRequest> {
        if self.pending.is_empty() {
            self.last_event_ms = None;
            return None;
        }
        let overflow = self
            .pending
            .iter()
            .any(|pending| pending.event.kind == WatcherEventKind::Overflow);
        let mut roots = if overflow {
            let paths: Vec<_> = self
                .pending
                .iter()
                .flat_map(|pending| pending.event.paths.iter())
                .collect();
            vec![smallest_common_parent(&self.project_root, &paths)]
        } else {
            self.pending
                .iter()
                .flat_map(|pending| pending.event.paths.iter())
                .map(|path| {
                    path.parent()
                        .filter(|parent| parent.starts_with(&self.project_root))
                        .unwrap_or(&self.project_root)
                        .to_path_buf()
                })
                .collect()
        };
        minimize_roots(&mut roots);
        let reason = if overflow {
            ReconcileReason::Overflow
        } else {
            let expected: HashSet<_> = self
                .pending
                .iter()
                .filter_map(|pending| pending.expected_operation)
                .collect();
            if self
                .pending
                .iter()
                .all(|pending| pending.expected_operation.is_some())
                && expected.len() == 1
            {
                ReconcileReason::ExpectedViewerChange(*expected.iter().next().unwrap())
            } else {
                ReconcileReason::ExternalChange
            }
        };
        self.pending.clear();
        self.last_event_ms = None;
        Some(ReconcileRequest {
            session_id: self.session_id,
            generation: self.generation,
            roots,
            reason,
        })
    }

    fn accepted_path(&self, path: &Path) -> Option<PathBuf> {
        let normalized = normalize_lexically(path)?;
        let relative = normalized.strip_prefix(&self.project_root).ok()?;
        let visible = relative.components().all(|component| match component {
            Component::Normal(name) => !name.to_string_lossy().starts_with('.'),
            _ => true,
        });
        visible.then_some(normalized)
    }
}

fn normalize_lexically(path: &Path) -> Option<PathBuf> {
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
                if !normalized.pop() {
                    return None;
                }
            }
            Component::Normal(segment) => normalized.push(segment),
        }
    }
    Some(normalized)
}

fn smallest_common_parent(project_root: &Path, paths: &[&PathBuf]) -> PathBuf {
    let mut common = paths
        .first()
        .and_then(|path| path.parent())
        .unwrap_or(project_root)
        .to_path_buf();
    for path in paths.iter().skip(1) {
        let parent = path.parent().unwrap_or(project_root);
        while !parent.starts_with(&common) && common != project_root {
            common.pop();
        }
    }
    if common.starts_with(project_root) {
        common
    } else {
        project_root.to_path_buf()
    }
}

fn minimize_roots(roots: &mut Vec<PathBuf>) {
    roots.sort();
    roots.dedup();
    let mut minimized = Vec::<PathBuf>::new();
    for root in roots.drain(..) {
        if minimized.iter().any(|parent| root.starts_with(parent)) {
            continue;
        }
        minimized.retain(|child| !child.starts_with(&root));
        minimized.push(root);
    }
    *roots = minimized;
}
