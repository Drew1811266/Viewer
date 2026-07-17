use viewer_application::watcher::{
    FileIdentity, ReconcileReason, WatcherEvent, WatcherEventKind, WatcherPort,
};
use viewer_domain::{OperationId, SessionId, search::Generation};
use viewer_infrastructure::scan::reconcile::{
    ExpectedChange, ExpectedChangeLedger, ReconcilePlanner,
};
#[cfg(target_os = "macos")]
use viewer_platform_macos::watcher::MacWatcherPort;

#[cfg(target_os = "macos")]
#[test]
fn watcher_reconcile_macos_adapter_starts_and_releases_a_recursive_watch() {
    let project = tempfile::tempdir().unwrap();
    let (sink, _events) = tokio::sync::mpsc::channel(8);
    let subscription = MacWatcherPort.watch(project.path(), sink).unwrap();
    drop(subscription);
}

#[test]
fn watcher_reconcile_coalesces_a_create_write_rename_burst_to_one_parent() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    let original = root.join("products/id-1/front.png");
    let renamed = root.join("products/id-1/hero.png");
    let mut planner = ReconcilePlanner::new(
        root.clone(),
        SessionId::new(),
        Generation::new(7),
        ExpectedChangeLedger::default(),
    )
    .unwrap();

    assert!(
        planner
            .push(WatcherEvent::added(original.clone()), 0)
            .is_none()
    );
    assert!(
        planner
            .push(WatcherEvent::modified(original.clone()), 100)
            .is_none()
    );
    assert!(
        planner
            .push(WatcherEvent::renamed(original, renamed), 200)
            .is_none()
    );
    assert!(planner.flush(449).is_none());
    let request = planner.flush(450).expect("debounced request");

    assert_eq!(request.generation, Generation::new(7));
    assert_eq!(request.roots, [root.join("products/id-1")]);
    assert_eq!(request.reason, ReconcileReason::ExternalChange);
    assert!(planner.flush(1_000).is_none());
}

#[test]
fn watcher_reconcile_marks_expected_viewer_events_but_still_reconciles_once() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    let old_path = root.join("products/id-1/front.png");
    let new_path = root.join("products/id-1/hero.png");
    let operation_id = OperationId::new();
    let identity = FileIdentity {
        volume: 42,
        file: 99,
    };
    let ledger = ExpectedChangeLedger::default();
    ledger.register(ExpectedChange {
        operation_id,
        old_canonical_path: old_path.clone(),
        new_canonical_path: new_path.clone(),
        expected_identity: identity,
        expires_at_ms: 1_000,
    });
    let mut planner =
        ReconcilePlanner::new(root.clone(), SessionId::new(), Generation::new(8), ledger).unwrap();

    planner.push(
        WatcherEvent::renamed(old_path, new_path.clone()).with_identity(identity),
        10,
    );
    planner.push(WatcherEvent::modified(new_path).with_identity(identity), 20);
    let request = planner.flush(270).expect("one expected reconciliation");

    assert_eq!(request.roots, [root.join("products/id-1")]);
    assert_eq!(
        request.reason,
        ReconcileReason::ExpectedViewerChange(operation_id)
    );
    assert!(planner.flush(520).is_none());
}

#[test]
fn watcher_reconcile_overflow_uses_the_smallest_known_common_ancestor() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    let mut planner = ReconcilePlanner::new(
        root.clone(),
        SessionId::new(),
        Generation::new(9),
        ExpectedChangeLedger::default(),
    )
    .unwrap();
    planner.push(
        WatcherEvent::new(
            WatcherEventKind::Overflow,
            vec![
                root.join("products/id-1/front.png"),
                root.join("products/id-2/back.png"),
            ],
        ),
        0,
    );

    let request = planner.flush(250).expect("overflow reconciliation");
    assert_eq!(request.roots, [root.join("products")]);
    assert_eq!(request.reason, ReconcileReason::Overflow);
}

#[test]
fn watcher_reconcile_does_not_match_expired_or_wrong_identity_entries() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    let path = root.join("notes.txt");
    let ledger = ExpectedChangeLedger::default();
    ledger.register(ExpectedChange {
        operation_id: OperationId::new(),
        old_canonical_path: path.clone(),
        new_canonical_path: path.clone(),
        expected_identity: FileIdentity { volume: 1, file: 2 },
        expires_at_ms: 10,
    });
    let mut planner =
        ReconcilePlanner::new(root, SessionId::new(), Generation::new(10), ledger).unwrap();
    planner.push(
        WatcherEvent::modified(path).with_identity(FileIdentity { volume: 1, file: 3 }),
        20,
    );

    let request = planner.flush(270).unwrap();
    assert_eq!(request.reason, ReconcileReason::ExternalChange);
}

#[test]
fn watcher_reconcile_ignores_reserved_hidden_and_outside_paths_and_clamps_root() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    let mut planner = ReconcilePlanner::new(
        root.clone(),
        SessionId::new(),
        Generation::new(11),
        ExpectedChangeLedger::default(),
    )
    .unwrap();
    planner.push(WatcherEvent::modified(root.join(".viewer/index.sqlite")), 0);
    planner.push(WatcherEvent::modified(root.join(".hidden/front.png")), 10);
    planner.push(
        WatcherEvent::modified(outside.path().join("outside.png")),
        20,
    );
    assert!(planner.flush(270).is_none());

    planner.push(WatcherEvent::modified(root.clone()), 300);
    let request = planner.flush(550).unwrap();
    assert_eq!(request.roots, [root]);
}
