use notify::{
    EventKind, RecommendedWatcher, RecursiveMode,
    event::{ModifyKind, RenameMode},
};
use notify_debouncer_full::{
    DebounceEventResult, DebouncedEvent, Debouncer, RecommendedCache, new_debouncer,
};
use std::{path::Path, time::Duration};
use viewer_application::watcher::{
    FileIdentity, WATCHER_DEBOUNCE_MS, WatchSubscription, WatcherError, WatcherEvent,
    WatcherEventKind, WatcherPort, WatcherSink,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct MacWatcherPort;

pub struct MacWatchSubscription {
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
}

impl WatchSubscription for MacWatchSubscription {}

impl WatcherPort for MacWatcherPort {
    fn watch(
        &self,
        root: &Path,
        sink: WatcherSink,
    ) -> Result<Box<dyn WatchSubscription>, WatcherError> {
        let root = std::fs::canonicalize(root).map_err(|error| watcher_error(root, &error))?;
        let error_root = root.clone();
        let mut debouncer = new_debouncer(
            Duration::from_millis(WATCHER_DEBOUNCE_MS),
            None,
            move |result: DebounceEventResult| {
                let events = normalize_result(result, &error_root);
                if !events.is_empty() {
                    let _ = sink.blocking_send(events);
                }
            },
        )
        .map_err(|error| WatcherError::Backend {
            path: root.clone(),
            message: error.to_string(),
        })?;
        debouncer
            .watch(&root, RecursiveMode::Recursive)
            .map_err(|error| WatcherError::Backend {
                path: root.clone(),
                message: error.to_string(),
            })?;
        Ok(Box::new(MacWatchSubscription {
            _debouncer: debouncer,
        }))
    }
}

fn normalize_result(result: DebounceEventResult, project_root: &Path) -> Vec<WatcherEvent> {
    match result {
        Ok(events) => events.into_iter().filter_map(normalize_event).collect(),
        Err(errors) => {
            let mut paths: Vec<_> = errors.into_iter().flat_map(|error| error.paths).collect();
            if paths.is_empty() {
                paths.push(project_root.to_path_buf());
            }
            vec![WatcherEvent::overflow(paths)]
        }
    }
}

fn normalize_event(event: DebouncedEvent) -> Option<WatcherEvent> {
    let event = event.event;
    if event.need_rescan() {
        return Some(WatcherEvent::overflow(event.paths));
    }
    let kind = match event.kind {
        EventKind::Create(_) => WatcherEventKind::Added,
        EventKind::Remove(_) => WatcherEventKind::Removed,
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => WatcherEventKind::Renamed,
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => WatcherEventKind::Removed,
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => WatcherEventKind::Added,
        EventKind::Modify(_) | EventKind::Any | EventKind::Other => WatcherEventKind::Modified,
        EventKind::Access(_) => return None,
    };
    let identity = event
        .paths
        .iter()
        .rev()
        .find_map(|path| file_identity(path));
    let mut event = WatcherEvent::new(kind, event.paths);
    event.identity = identity;
    Some(event)
}

fn file_identity(path: &Path) -> Option<FileIdentity> {
    use std::os::unix::fs::MetadataExt;
    std::fs::symlink_metadata(path)
        .ok()
        .map(|metadata| FileIdentity {
            volume: metadata.dev(),
            file: metadata.ino(),
        })
}

fn watcher_error(path: &Path, error: &std::io::Error) -> WatcherError {
    WatcherError::Backend {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}
