use async_trait::async_trait;
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Notify, watch};
use viewer_application::{
    LocalFileCommandPort, ProjectAccess,
    file_commands::{
        BatchItemStatus, BatchLifecycle, BatchResultCode, ConflictResolution, FileCommand,
        FileCommandAction, FileCommandCancellation, FileCommandItem, FileCommandItemExecution,
        FileCommandKind, FileCommandPreflightState, FileCommandService, FileCommandServiceError,
        LocalFileCommandError, LocalFileCommandOutcome, LocalFileCommandPreflightItem,
        MAX_BATCH_RESULT_PAGE_SIZE, MAX_FILE_COMMAND_ITEMS,
    },
    scheduler::TaskCoordinator,
};
use viewer_domain::{
    EntityId, RelativePath, SessionId, operation::ConflictPolicy, search::Generation,
};

#[derive(Default)]
struct FakePort {
    conflicts: Mutex<HashSet<EntityId>>,
    blocked: Mutex<HashMap<EntityId, BatchResultCode>>,
    outcomes: Mutex<HashMap<EntityId, LocalFileCommandOutcome>>,
    executed: Mutex<Vec<FileCommandItemExecution>>,
    preflight_calls: Mutex<usize>,
    block_first: AtomicBool,
    first_started: Notify,
    release_first: Notify,
    first_seen: AtomicBool,
    reverse_preflight: AtomicBool,
    cancel_session_after_first: Mutex<Option<(Arc<TaskCoordinator>, SessionId)>>,
}

impl FakePort {
    fn set_conflicts(&self, entities: impl IntoIterator<Item = EntityId>) {
        self.conflicts.lock().unwrap().extend(entities);
    }

    fn set_blocked(&self, entity_id: EntityId, code: BatchResultCode) {
        self.blocked.lock().unwrap().insert(entity_id, code);
    }

    fn set_outcome(&self, entity_id: EntityId, outcome: LocalFileCommandOutcome) {
        self.outcomes.lock().unwrap().insert(entity_id, outcome);
    }

    fn executed(&self) -> Vec<FileCommandItemExecution> {
        self.executed.lock().unwrap().clone()
    }

    fn preflight_calls(&self) -> usize {
        *self.preflight_calls.lock().unwrap()
    }
}

#[async_trait]
impl LocalFileCommandPort for FakePort {
    async fn preflight(
        &self,
        _batch_id: viewer_application::file_commands::BatchId,
        command: &FileCommand,
    ) -> Result<Vec<LocalFileCommandPreflightItem>, LocalFileCommandError> {
        *self.preflight_calls.lock().unwrap() += 1;
        let conflicts = self.conflicts.lock().unwrap().clone();
        let blocked = self.blocked.lock().unwrap().clone();
        let mut rows = command
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| LocalFileCommandPreflightItem {
                entity_id: item.entity_id,
                relative_path: RelativePath::parse(&format!("items/{index:05}.png")).unwrap(),
                state: blocked
                    .get(&item.entity_id)
                    .copied()
                    .map(FileCommandPreflightState::Blocked)
                    .unwrap_or_else(|| {
                        if conflicts.contains(&item.entity_id) {
                            FileCommandPreflightState::Conflict
                        } else {
                            FileCommandPreflightState::Ready
                        }
                    }),
            })
            .collect::<Vec<_>>();
        if self.reverse_preflight.load(Ordering::SeqCst) {
            rows.reverse();
        }
        Ok(rows)
    }

    async fn execute_item(
        &self,
        request: FileCommandItemExecution,
        _cancellation: FileCommandCancellation,
    ) -> Result<LocalFileCommandOutcome, LocalFileCommandError> {
        self.executed.lock().unwrap().push(request.clone());
        if !self.first_seen.swap(true, Ordering::SeqCst) {
            self.first_started.notify_one();
            if self.block_first.load(Ordering::SeqCst) {
                self.release_first.notified().await;
            }
            if let Some((coordinator, session_id)) =
                self.cancel_session_after_first.lock().unwrap().take()
            {
                coordinator.cancel_session(session_id);
            }
        }
        Ok(self
            .outcomes
            .lock()
            .unwrap()
            .get(&request.item.entity_id)
            .copied()
            .unwrap_or(LocalFileCommandOutcome::completed(BatchResultCode::Moved)))
    }
}

struct Fixture {
    session_id: SessionId,
    generation: Generation,
    coordinator: Arc<TaskCoordinator>,
    port: Arc<FakePort>,
    service: Arc<FileCommandService>,
}

impl Fixture {
    fn new(access: ProjectAccess) -> Self {
        let session_id = SessionId::new();
        let coordinator = Arc::new(TaskCoordinator::default());
        let generation = coordinator.begin_session(session_id);
        let port = Arc::new(FakePort::default());
        let service = Arc::new(FileCommandService::new(
            session_id,
            access,
            Arc::clone(&coordinator),
            port.clone(),
        ));
        Self {
            session_id,
            generation,
            coordinator,
            port,
            service,
        }
    }

    fn command(&self, kind: FileCommandKind, count: usize) -> FileCommand {
        let destination = EntityId::new();
        let items = (0..count)
            .map(|index| FileCommandItem {
                entity_id: EntityId::new(),
                action: match kind {
                    FileCommandKind::Rename => FileCommandAction::Rename {
                        proposed_name: format!("renamed-{index}.png"),
                        edit_extension: false,
                    },
                    FileCommandKind::Copy => FileCommandAction::Copy {
                        destination_folder: destination,
                    },
                    FileCommandKind::Move => FileCommandAction::Move {
                        destination_folder: destination,
                    },
                    FileCommandKind::Trash => FileCommandAction::Trash,
                },
            })
            .collect();
        FileCommand {
            session_id: self.session_id,
            generation: self.generation,
            kind,
            items,
        }
    }
}

#[tokio::test]
async fn validation_rejects_read_only_stale_empty_duplicate_and_oversized_commands() {
    let readonly = Fixture::new(ProjectAccess::ReadOnly);
    assert_eq!(
        readonly
            .service
            .preflight(readonly.command(FileCommandKind::Trash, 1))
            .await,
        Err(FileCommandServiceError::ReadOnly)
    );
    assert_eq!(readonly.port.preflight_calls(), 0);

    let fixture = Fixture::new(ProjectAccess::ReadWrite);
    let mut stale_session = fixture.command(FileCommandKind::Trash, 1);
    stale_session.session_id = SessionId::new();
    assert_eq!(
        fixture.service.preflight(stale_session).await,
        Err(FileCommandServiceError::StaleSession)
    );
    let mut stale_generation = fixture.command(FileCommandKind::Trash, 1);
    stale_generation.generation = Generation::new(fixture.generation.get() + 1);
    assert_eq!(
        fixture.service.preflight(stale_generation).await,
        Err(FileCommandServiceError::StaleSession)
    );
    assert_eq!(
        fixture
            .service
            .preflight(fixture.command(FileCommandKind::Trash, 0))
            .await,
        Err(FileCommandServiceError::EmptyTargets)
    );
    let mut duplicate = fixture.command(FileCommandKind::Trash, 2);
    duplicate.items[1].entity_id = duplicate.items[0].entity_id;
    assert_eq!(
        fixture.service.preflight(duplicate).await,
        Err(FileCommandServiceError::DuplicateTarget)
    );
    let mut mixed = fixture.command(FileCommandKind::Trash, 1);
    mixed.items[0].action = FileCommandAction::Copy {
        destination_folder: EntityId::new(),
    };
    assert_eq!(
        fixture.service.preflight(mixed).await,
        Err(FileCommandServiceError::ActionKindMismatch)
    );
    assert_eq!(
        fixture
            .service
            .preflight(fixture.command(FileCommandKind::Trash, MAX_FILE_COMMAND_ITEMS + 1,))
            .await,
        Err(FileCommandServiceError::TooManyTargets)
    );
    assert_eq!(fixture.port.preflight_calls(), 0);
}

#[tokio::test]
async fn preflight_preserves_visible_order_and_exposes_blocked_rows_without_execution() {
    let fixture = Fixture::new(ProjectAccess::ReadWrite);
    let command = fixture.command(FileCommandKind::Rename, 4);
    fixture.port.set_conflicts([command.items[1].entity_id]);
    fixture
        .port
        .set_blocked(command.items[2].entity_id, BatchResultCode::InvalidTarget);

    let preflight = fixture.service.preflight(command.clone()).await.unwrap();

    assert_eq!(
        preflight
            .rows()
            .iter()
            .map(|row| row.entity_id)
            .collect::<Vec<_>>(),
        command
            .items
            .iter()
            .map(|item| item.entity_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(preflight.rows()[0].state, FileCommandPreflightState::Ready);
    assert_eq!(
        preflight.rows()[1].state,
        FileCommandPreflightState::Conflict
    );
    assert_eq!(
        preflight.rows()[2].state,
        FileCommandPreflightState::Blocked(BatchResultCode::InvalidTarget)
    );
    assert!(!preflight.is_executable());
    assert_eq!(
        fixture.service.execute(preflight, &[], None).await,
        Err(FileCommandServiceError::PreflightBlocked)
    );
    assert!(fixture.port.executed().is_empty());

    let invalid = Fixture::new(ProjectAccess::ReadWrite);
    invalid.port.reverse_preflight.store(true, Ordering::SeqCst);
    assert_eq!(
        invalid
            .service
            .preflight(invalid.command(FileCommandKind::Trash, 2))
            .await,
        Err(FileCommandServiceError::PreflightContract)
    );
}

#[tokio::test]
async fn execution_applies_one_conflict_choice_to_remaining_and_counts_every_terminal_item() {
    let fixture = Fixture::new(ProjectAccess::ReadWrite);
    let command = fixture.command(FileCommandKind::Copy, 5);
    let ids = command
        .items
        .iter()
        .map(|item| item.entity_id)
        .collect::<Vec<_>>();
    fixture.port.set_conflicts([ids[1], ids[2]]);
    fixture.port.set_outcome(
        ids[0],
        LocalFileCommandOutcome::completed(BatchResultCode::Copied),
    );
    fixture.port.set_outcome(
        ids[1],
        LocalFileCommandOutcome::completed(BatchResultCode::Copied),
    );
    fixture.port.set_outcome(
        ids[2],
        LocalFileCommandOutcome::completed(BatchResultCode::Copied),
    );
    fixture.port.set_outcome(
        ids[3],
        LocalFileCommandOutcome::failed(BatchResultCode::PermissionDenied),
    );
    fixture.port.set_outcome(
        ids[4],
        LocalFileCommandOutcome::skipped(BatchResultCode::ConflictSkipped),
    );
    let preflight = fixture.service.preflight(command).await.unwrap();
    assert_eq!(
        fixture.service.execute(preflight.clone(), &[], None).await,
        Err(FileCommandServiceError::MissingConflictResolution)
    );

    let replay = preflight.clone();
    let choices = [ConflictResolution {
        entity_id: ids[1],
        policy: ConflictPolicy::KeepBoth,
        apply_to_remaining: true,
    }];
    let summary = fixture
        .service
        .execute(preflight, &choices, None)
        .await
        .unwrap();

    assert_eq!(summary.lifecycle(), BatchLifecycle::Completed);
    assert_eq!(summary.requested(), 5);
    assert_eq!(summary.completed(), 3);
    assert_eq!(summary.failed(), 1);
    assert_eq!(summary.skipped(), 1);
    assert_eq!(summary.cancelled(), 0);
    assert!(summary.counts_are_consistent());
    let executed = fixture.port.executed();
    assert_eq!(executed.len(), 5);
    assert_eq!(executed[1].conflict_policy, Some(ConflictPolicy::KeepBoth));
    assert_eq!(executed[2].conflict_policy, Some(ConflictPolicy::KeepBoth));
    let page = summary.result_page(0, 500);
    assert_eq!(page.total, 5);
    assert_eq!(page.items.len(), 5);
    assert_eq!(page.items[3].status, BatchItemStatus::Failed);
    assert_eq!(page.items[3].code, BatchResultCode::PermissionDenied);
    assert_eq!(
        fixture.service.execute(replay, &choices, None).await,
        Err(FileCommandServiceError::AlreadyExecuted)
    );
}

#[tokio::test]
async fn skip_conflict_choice_never_starts_the_file_mutation() {
    let fixture = Fixture::new(ProjectAccess::ReadWrite);
    let command = fixture.command(FileCommandKind::Copy, 1);
    let entity_id = command.items[0].entity_id;
    fixture.port.set_conflicts([entity_id]);
    let preflight = fixture.service.preflight(command).await.unwrap();

    let summary = fixture
        .service
        .execute(
            preflight,
            &[ConflictResolution {
                entity_id,
                policy: ConflictPolicy::Skip,
                apply_to_remaining: false,
            }],
            None,
        )
        .await
        .unwrap();

    assert_eq!(summary.skipped(), 1);
    assert_eq!(
        summary.completed() + summary.failed() + summary.cancelled(),
        0
    );
    assert_eq!(
        summary.result_page(0, 1).items[0].code,
        BatchResultCode::ConflictSkipped
    );
    assert!(fixture.port.executed().is_empty());
}

#[tokio::test]
async fn one_write_lane_cancels_queued_items_but_allows_the_active_atomic_item_to_finish() {
    let fixture = Fixture::new(ProjectAccess::ReadWrite);
    fixture.port.block_first.store(true, Ordering::SeqCst);
    let first = fixture
        .service
        .preflight(fixture.command(FileCommandKind::Move, 3))
        .await
        .unwrap();
    let first_batch = first.batch_id();
    let second = fixture
        .service
        .preflight(fixture.command(FileCommandKind::Trash, 1))
        .await
        .unwrap();
    let (progress_tx, mut progress_rx) = watch::channel(first.initial_progress());
    let service = Arc::clone(&fixture.service);
    let running = tokio::spawn(async move { service.execute(first, &[], Some(progress_tx)).await });
    fixture.port.first_started.notified().await;

    assert_eq!(
        fixture.service.execute(second, &[], None).await,
        Err(FileCommandServiceError::BatchActive)
    );
    progress_rx.borrow_and_update();
    assert!(fixture.service.cancel_pending(first_batch));
    tokio::time::timeout(Duration::from_millis(100), progress_rx.changed())
        .await
        .expect("cancel publishes immediately")
        .unwrap();
    assert_eq!(progress_rx.borrow().lifecycle, BatchLifecycle::Cancelling);
    fixture.port.release_first.notify_one();
    let summary = running.await.unwrap().unwrap();

    assert_eq!(summary.completed(), 1);
    assert_eq!(summary.cancelled(), 2);
    assert_eq!(summary.failed() + summary.skipped(), 0);
    assert!(summary.counts_are_consistent());
    assert_eq!(fixture.port.executed().len(), 1);
    let latest = progress_rx.borrow().clone();
    assert_eq!(latest.lifecycle, BatchLifecycle::Completed);
    assert_eq!(latest.completed, 1);
    assert_eq!(latest.cancelled, 2);
    assert_eq!(latest.active_entity_id, None);
    assert!(!fixture.service.cancel_pending(first_batch));
}

#[tokio::test]
async fn generation_is_revalidated_before_every_item_and_remaining_work_stays_unstarted() {
    let fixture = Fixture::new(ProjectAccess::ReadWrite);
    *fixture.port.cancel_session_after_first.lock().unwrap() =
        Some((Arc::clone(&fixture.coordinator), fixture.session_id));
    let preflight = fixture
        .service
        .preflight(fixture.command(FileCommandKind::Trash, 3))
        .await
        .unwrap();

    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(summary.completed(), 1);
    assert_eq!(summary.cancelled(), 2);
    assert_eq!(fixture.port.executed().len(), 1);
    let page = summary.result_page(0, 10);
    assert_eq!(page.items[1].status, BatchItemStatus::Cancelled);
    assert_eq!(page.items[1].code, BatchResultCode::SessionStale);
    assert_eq!(page.items[2].code, BatchResultCode::SessionStale);
}

#[tokio::test]
async fn result_pages_are_hard_capped_and_watch_progress_coalesces_to_the_latest_state() {
    let fixture = Fixture::new(ProjectAccess::ReadWrite);
    let preflight = fixture
        .service
        .preflight(fixture.command(FileCommandKind::Trash, 205))
        .await
        .unwrap();
    let (progress_tx, progress_rx) = watch::channel(preflight.initial_progress());

    let summary = fixture
        .service
        .execute(preflight, &[], Some(progress_tx))
        .await
        .unwrap();

    assert_eq!(summary.completed(), 205);
    assert_eq!(summary.result_page(0, 10_000).items.len(), 200);
    assert_eq!(summary.result_page(200, 10_000).items.len(), 5);
    assert_eq!(MAX_BATCH_RESULT_PAGE_SIZE, 200);
    let latest = progress_rx.borrow().clone();
    assert_eq!(latest.lifecycle, BatchLifecycle::Completed);
    assert_eq!(latest.completed, 205);
    assert_eq!(latest.processed(), latest.requested);
}
