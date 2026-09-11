#![allow(dead_code)]

use async_trait::async_trait;
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use viewer_application::{
    review_assets::{ContinuousReviewAssetPort, SourceRelocationDecision},
    review_evidence::*,
    review_workspace::*,
    *,
};
use viewer_domain::{
    review::{continuous::*, *},
    *,
};

#[derive(Default)]
pub struct MemoryRepository {
    states: Mutex<Vec<StoredContinuousSnapshot>>,
    recovery: Mutex<HashMap<ReviewCommandId, RecoveryDraft>>,
    archives: Mutex<HashMap<ReviewArchiveId, ArchiveCheckpoint>>,
    archive_order: Mutex<Vec<ReviewArchiveId>>,
    pub fail_commit: Mutex<Option<ReviewCommitError>>,
    pub fail_view_after_commit: Mutex<bool>,
    pub cancel_after_commit: Mutex<Option<ReviewTaskCancellation>>,
    usage: Mutex<HashMap<ReviewUsageId, ReviewUsageDeclaration>>,
    pub migration: Mutex<Option<MigrationInspection>>,
    legacy: Mutex<Option<MigrationInspection>>,
    fail_open: AtomicBool,
    opens: AtomicUsize,
}
impl MemoryRepository {
    pub fn seed_history_fixture(
        &self,
        states: Vec<StoredContinuousSnapshot>,
        archive: ArchiveCheckpoint,
    ) {
        *self.states.lock().unwrap() = states;
        self.archive_order.lock().unwrap().push(archive.archive_id);
        self.archives
            .lock()
            .unwrap()
            .insert(archive.archive_id, archive);
    }
    pub fn current(&self) -> Option<StoredContinuousSnapshot> {
        self.states.lock().unwrap().last().cloned()
    }
    pub fn fail_if_opened(&self) {
        self.fail_open.store(true, Ordering::Release);
    }
    pub fn opens(&self) -> usize {
        self.opens.load(Ordering::Acquire)
    }
}
impl ContinuousReviewRepositoryPort for MemoryRepository {
    fn sync_publication(&self) -> Result<(), ReviewCommitError> {
        Ok(())
    }

    fn load_history_selectors(
        &self,
        stream: ReviewStreamId,
    ) -> Result<Vec<HistorySelector>, ReviewCommitError> {
        let archive_order = self.archive_order.lock().unwrap().clone();
        let archives = self.archives.lock().unwrap();
        let mut selectors = archive_order
            .iter()
            .map(|id| {
                archives
                    .get(id)
                    .filter(|archive| archive.stream_id == stream)
                    .map(|_| HistorySelector::Archive(*id))
                    .ok_or(ReviewCommitError::Integrity)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(inspection) = self.legacy.lock().unwrap().as_ref() {
            selectors.extend(
                inspection
                    .legacy_records
                    .iter()
                    .filter(|reference| reference.stream_id == stream)
                    .map(|reference| HistorySelector::Legacy(reference.round_id)),
            );
        }
        Ok(selectors)
    }

    fn load_legacy(
        &self,
        stream: ReviewStreamId,
        round: ReviewRoundId,
    ) -> Result<LegacyReviewRecord, ReviewCommitError> {
        let inspection = self.legacy.lock().unwrap();
        let inspection = inspection.as_ref().ok_or(ReviewCommitError::Integrity)?;
        let reference = inspection
            .legacy_records
            .iter()
            .find(|r| r.round_id == round && r.stream_id == stream)
            .ok_or(ReviewCommitError::Integrity)?
            .clone();
        let contents = if let Some(draft) = inspection
            .active_draft
            .as_ref()
            .filter(|d| d.draft.review_round_id == round)
        {
            LegacyReviewContents::Draft(draft.draft.clone())
        } else {
            LegacyReviewContents::Completed(
                inspection
                    .completed_candidates
                    .iter()
                    .find(|c| c.review_round_id == round)
                    .ok_or(ReviewCommitError::Integrity)?
                    .clone(),
            )
        };
        Ok(LegacyReviewRecord {
            reference,
            contents,
        })
    }
    fn load_usage(
        &self,
        _: ReviewStreamId,
        id: ReviewUsageId,
    ) -> Result<Option<ReviewUsageDeclaration>, ReviewCommitError> {
        Ok(self.usage.lock().unwrap().get(&id).cloned())
    }
    fn load_coverage(
        &self,
        _: ReviewStreamId,
        head: SnapshotRef,
        keys: &[TargetVersionKey],
    ) -> Result<Vec<ArchiveCoverage>, ReviewCommitError> {
        let states = self.states.lock().unwrap();
        let position = states
            .iter()
            .position(|s| s.reference == head)
            .ok_or(ReviewCommitError::Integrity)?;
        let mut seen = std::collections::HashSet::new();
        let mut result = vec![];
        for state in states[..=position].iter().rev() {
            for change in state.changes.iter().rev() {
                if let (Some(id), Some(key)) = (change.archive_id, change.historical_key)
                    && keys.contains(&key)
                    && seen.insert((id, key))
                {
                    result.push(ArchiveCoverage {
                        archive_id: id,
                        key,
                        active: change.kind == ReviewChangeKind::Archived,
                    });
                }
            }
        }
        Ok(result)
    }
    fn load_evidence(
        &self,
        _: ReviewStreamId,
        _: &HistorySelector,
        asset: AssetVersionId,
        role: EvidenceRole,
    ) -> Result<BoundReviewImage, ReviewCommitError> {
        let current = self.current().ok_or(ReviewCommitError::EvidenceAbsent)?;
        let asset = current
            .state
            .assets
            .iter()
            .find(|a| a.id == asset)
            .unwrap()
            .clone();
        if role != EvidenceRole::Base {
            return Err(ReviewCommitError::EvidenceAbsent);
        }
        Ok(png(asset))
    }
    fn save_recovery(&self, draft: &RecoveryDraft) -> Result<(), ReviewCommitError> {
        let mut values = self.recovery.lock().unwrap();
        if values.len() >= 100 {
            return Err(ReviewCommitError::LimitExceeded);
        }
        values.insert(draft.command_id, draft.clone());
        Ok(())
    }
    fn load_recovery(&self) -> Result<Vec<RecoveryDraft>, ReviewCommitError> {
        Ok(self.recovery.lock().unwrap().values().cloned().collect())
    }
    fn resolve_recovery(&self, id: ReviewCommandId) -> Result<CommandLookup, ReviewCommitError> {
        self.find_command(ReviewStreamId::from_u128(2), id)
    }
    fn load_current_ref(
        &self,
        _: ReviewStreamId,
    ) -> Result<Option<SnapshotRef>, ReviewCommitError> {
        Ok(self.current().map(|s| s.reference))
    }
    fn load_current(
        &self,
        _: ReviewStreamId,
    ) -> Result<Option<StoredContinuousSnapshot>, ReviewCommitError> {
        if *self.fail_view_after_commit.lock().unwrap() && self.current().is_some() {
            return Err(ReviewCommitError::Io);
        }
        Ok(self.current())
    }
    fn load_snapshot(
        &self,
        _: ReviewStreamId,
        r: &SnapshotRef,
    ) -> Result<StoredContinuousSnapshot, ReviewCommitError> {
        self.states
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.reference == *r)
            .cloned()
            .ok_or(ReviewCommitError::Integrity)
    }
    fn load_archive(
        &self,
        _: ReviewStreamId,
        id: ReviewArchiveId,
    ) -> Result<ArchiveCheckpoint, ReviewCommitError> {
        self.archives
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or(ReviewCommitError::Integrity)
    }
    fn find_command(
        &self,
        _: ReviewStreamId,
        id: ReviewCommandId,
    ) -> Result<CommandLookup, ReviewCommitError> {
        Ok(self
            .states
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.command_id == id)
            .map_or(CommandLookup::Absent, |s| {
                CommandLookup::Found(ReviewCommitReceipt {
                    command_id: id,
                    payload_digest: s.payload_digest,
                    snapshot: s.reference,
                })
            }))
    }
    fn commit(&self, r: ReviewCommitRequest) -> Result<ReviewCommitReceipt, ReviewCommitError> {
        let failure = self.fail_commit.lock().unwrap().take();
        if let Some(error) = failure.filter(|e| *e != ReviewCommitError::OutcomeUnknown) {
            return Err(error);
        }
        let mut states = self.states.lock().unwrap();
        if states.len() >= 100 {
            return Err(ReviewCommitError::LimitExceeded);
        }
        if states.last().map(|s| s.reference) != r.expected {
            return Err(ReviewCommitError::StaleSnapshot);
        }
        let reference = SnapshotRef {
            snapshot_id: r.next.state.snapshot_id,
            blake3: r.next.payload_digest,
        };
        let receipt = ReviewCommitReceipt {
            command_id: r.next.command_id,
            payload_digest: r.next.payload_digest,
            snapshot: reference,
        };
        let mut state = r.next.state;
        for usage in r.adopted_usage {
            self.usage.lock().unwrap().insert(usage.id, usage);
        }
        state.parent = r.expected;
        for archive in r.archives {
            self.archive_order.lock().unwrap().push(archive.archive_id);
            self.archives
                .lock()
                .unwrap()
                .insert(archive.archive_id, archive);
        }
        states.push(StoredContinuousSnapshot {
            reference,
            publication_protocol: r.next.publication_protocol,
            production: r.production,
            state,
            command_id: receipt.command_id,
            payload_digest: receipt.payload_digest,
            changes: r.next.changes,
            evidence: r.next.evidence,
        });
        if let Some(cancel) = self.cancel_after_commit.lock().unwrap().take() {
            cancel.cancel();
        }
        if failure == Some(ReviewCommitError::OutcomeUnknown) {
            Err(ReviewCommitError::OutcomeUnknown)
        } else {
            Ok(receipt)
        }
    }
}
#[derive(Default)]
pub struct MemoryAuthoringRepository {
    states: Mutex<Vec<StoredAuthoringState>>,
}

impl ContinuousReviewAuthoringStorePort for MemoryAuthoringRepository {
    fn load_heads(&self, _: ReviewStreamId) -> Result<ReviewHeads, ReviewCommitError> {
        Ok(ReviewHeads {
            authoring: self.states.lock().unwrap().last().map(|value| value.head),
            published: None,
        })
    }

    fn load_current(
        &self,
        _: ReviewStreamId,
    ) -> Result<Option<StoredAuthoringState>, ReviewCommitError> {
        Ok(self.states.lock().unwrap().last().cloned())
    }

    fn load_snapshot(
        &self,
        _: ReviewStreamId,
        sequence: ReviewAuthoringSequence,
    ) -> Result<StoredAuthoringState, ReviewCommitError> {
        self.states
            .lock()
            .unwrap()
            .iter()
            .find(|value| value.head.sequence == sequence)
            .cloned()
            .ok_or(ReviewCommitError::Integrity)
    }

    fn find_command(
        &self,
        _: ReviewStreamId,
        command: ReviewCommandId,
    ) -> Result<AuthoringCommandLookup, ReviewCommitError> {
        Ok(self
            .states
            .lock()
            .unwrap()
            .iter()
            .find(|value| value.command_id == command)
            .map_or(AuthoringCommandLookup::Absent, |value| {
                AuthoringCommandLookup::Found(ReviewAuthoringReceipt {
                    command_id: value.command_id,
                    payload_digest: value.payload_digest,
                    head: value.head,
                })
            }))
    }

    fn commit(
        &self,
        _: ReviewStreamId,
        command: ReviewCommandId,
        payload_digest: [u8; 32],
        prepare: &mut dyn FnMut(
            Option<&StoredAuthoringState>,
        )
            -> Result<ReviewAuthoringCommitRequest, ReviewWorkspaceError>,
    ) -> Result<ReviewAuthoringReceipt, ReviewWorkspaceError> {
        let mut states = self.states.lock().unwrap();
        if let Some(value) = states.iter().find(|value| value.command_id == command) {
            if value.payload_digest != payload_digest {
                return Err(ReviewCommitError::CommandConflict.into());
            }
            return Ok(ReviewAuthoringReceipt {
                command_id: value.command_id,
                payload_digest: value.payload_digest,
                head: value.head,
            });
        }
        let request = prepare(states.last())?;
        if request.expected_snapshot_id != states.last().map(|value| value.head.snapshot_id)
            || request.next.head.sequence != states.len() as u64 + 1
            || request.next.command_id != command
            || request.next.payload_digest != payload_digest
        {
            return Err(ReviewCommitError::Integrity.into());
        }
        let receipt = ReviewAuthoringReceipt {
            command_id: request.next.command_id,
            payload_digest: request.next.payload_digest,
            head: request.next.head,
        };
        states.push(request.next);
        Ok(receipt)
    }
}

impl ReviewMaterializationQueuePort for MemoryAuthoringRepository {
    fn next(&self, _: i64) -> Result<Option<ClaimedReviewMaterialization>, ReviewCommitError> {
        Ok(None)
    }
    fn retry(
        &self,
        _: &ClaimedReviewMaterialization,
        _: ReviewMaterializationFailure,
        _: i64,
    ) -> Result<(), ReviewCommitError> {
        Err(ReviewCommitError::LookupUnavailable)
    }
    fn block(
        &self,
        _: &ClaimedReviewMaterialization,
        _: ReviewMaterializationFailure,
    ) -> Result<(), ReviewCommitError> {
        Err(ReviewCommitError::LookupUnavailable)
    }
    fn mark_published(
        &self,
        _: &ClaimedReviewMaterialization,
        _: ReviewPublicationReceipt,
    ) -> Result<(), ReviewCommitError> {
        Err(ReviewCommitError::LookupUnavailable)
    }
    fn status(&self, _: ReviewStreamId) -> Result<ReviewPublicationStatus, ReviewCommitError> {
        Ok(ReviewPublicationStatus::Pending {
            pending_revisions: self.states.lock().unwrap().len() as u32,
        })
    }
    fn requeue_expired(&self, _: i64) -> Result<u32, ReviewCommitError> {
        Ok(0)
    }
}

struct Provider(Arc<MemoryRepository>, Arc<MemoryAuthoringRepository>);
impl ContinuousReviewRepositoryProviderPort for Provider {
    fn save_migration_recovery(&self, draft: &RecoveryDraft) -> Result<(), ReviewCommitError> {
        self.0.save_recovery(draft)
    }
    fn load_migration_recovery(&self) -> Result<Vec<RecoveryDraft>, ReviewCommitError> {
        self.0.load_recovery()
    }
    fn inspect_migration(&self) -> Result<Option<MigrationInspection>, ReviewCommitError> {
        Ok(self.0.migration.lock().unwrap().clone())
    }
    fn migrate(
        &self,
        request: MigrationCommitRequest,
    ) -> Result<ReviewCommitReceipt, ReviewCommitError> {
        let inspection = self
            .0
            .migration
            .lock()
            .unwrap()
            .clone()
            .ok_or(ReviewCommitError::MigrationRequired)?;
        *self.0.legacy.lock().unwrap() = Some(inspection.clone());
        let state =
            prepare_migration_state(&inspection, &request.envelope, &request.next.state.assets)
                .map_err(|_| ReviewCommitError::StaleSnapshot)?;
        if state != request.next.state {
            return Err(ReviewCommitError::Integrity);
        }
        let result = self.0.commit(ReviewCommitRequest {
            expected: None,
            production: request.envelope.context.production,
            next: request.next,
            archives: vec![],
            adopted_usage: vec![],
            staged_evidence: request.staged_evidence,
        });
        if result.is_ok() || result == Err(ReviewCommitError::OutcomeUnknown) {
            *self.0.migration.lock().unwrap() = None;
        }
        result
    }
    fn open_reader(&self) -> Result<Arc<dyn ContinuousReviewRepositoryPort>, ReviewCommitError> {
        self.0.opens.fetch_add(1, Ordering::AcqRel);
        assert!(
            !self.0.fail_open.load(Ordering::Acquire),
            "v3 repository opened"
        );
        if self.0.migration.lock().unwrap().is_some() {
            return Err(ReviewCommitError::MigrationRequired);
        }
        Ok(self.0.clone())
    }
    fn open_writer(&self) -> Result<Arc<dyn ContinuousReviewRepositoryPort>, ReviewCommitError> {
        self.0.opens.fetch_add(1, Ordering::AcqRel);
        assert!(
            !self.0.fail_open.load(Ordering::Acquire),
            "v3 repository opened"
        );
        if self.0.migration.lock().unwrap().is_some() {
            return Err(ReviewCommitError::MigrationRequired);
        }
        Ok(self.0.clone())
    }
    fn open_authoring_reader(
        &self,
    ) -> Result<Arc<dyn ContinuousReviewAuthoringRepositoryPort>, ReviewCommitError> {
        Ok(self.1.clone())
    }
    fn open_authoring_writer(
        &self,
    ) -> Result<Arc<dyn ContinuousReviewAuthoringRepositoryPort>, ReviewCommitError> {
        Ok(self.1.clone())
    }
}
#[derive(Default)]
pub struct Assets {
    pub changed: Mutex<bool>,
    fail_source_check: AtomicBool,
    source_checks: AtomicUsize,
}
impl Assets {
    pub fn fail_if_sources_checked(&self) {
        self.fail_source_check.store(true, Ordering::Release);
    }
    pub fn source_checks(&self) -> usize {
        self.source_checks.load(Ordering::Acquire)
    }
}
#[async_trait]
impl ContinuousReviewAssetPort for Assets {
    async fn prepare_additions(
        &self,
        ids: &[EntityId],
        cancel: ReviewTaskCancellation,
    ) -> Result<Vec<PreparedReviewAsset>, ReviewAssetError> {
        if cancel.is_cancelled() {
            return Err(ReviewAssetError::Cancelled);
        }
        Ok(ids
            .iter()
            .map(|id| PreparedReviewAsset {
                entity_id: *id,
                asset: asset(*id),
                failure: None,
                change_revision: 1,
                source_path: "/test-only/no-io.png".into(),
            })
            .collect())
    }
    async fn check_sources(
        &self,
        assets: &mut [AssetVersion],
        cancel: ReviewTaskCancellation,
    ) -> Result<Vec<SourceCheck>, ReviewAssetError> {
        self.source_checks.fetch_add(1, Ordering::AcqRel);
        assert!(
            !self.fail_source_check.load(Ordering::Acquire),
            "review sources checked"
        );
        if cancel.is_cancelled() {
            return Err(ReviewAssetError::Cancelled);
        }
        Ok(assets
            .iter()
            .map(|a| SourceCheck {
                asset_version_id: a.id,
                checked_at_ms: 1,
                status: if *self.changed.lock().unwrap() {
                    SourceCheckStatus::Changed
                } else {
                    SourceCheckStatus::Match
                },
            })
            .collect())
    }
    async fn confirm_relocation(
        &self,
        _: &AssetVersion,
        _: SourceRelocationDecision,
        _: ReviewTaskCancellation,
    ) -> Result<(), ReviewAssetError> {
        Err(ReviewAssetError::UnconfirmedLocation)
    }
}
fn asset(id: EntityId) -> AssetVersion {
    AssetVersion {
        id: id.to_string().parse().unwrap(),
        source_entity_id: Some(id),
        relative_path: RelativePath::parse(&format!("{id}.png")).unwrap(),
        evidence: AssetEvidence {
            size_bytes: PNG.len() as u64,
            modified_ns: 1,
            blake3: Some(PNG_DIGEST),
        },
        media: ReviewMedia::Image {
            width: Some(640),
            height: Some(480),
        },
        producer_asset_id: None,
        parent_asset_version_id: None,
    }
}
const PNG: &[u8] = include_bytes!("../../../../tests/fixtures/images/alpha.png");
const PNG_DIGEST: [u8; 32] = [
    0xc7, 0x65, 0x6f, 0xdc, 0x8b, 0x64, 0x41, 0xfb, 0x96, 0x93, 0xef, 0x94, 0xef, 0xde, 0xc4, 0xe6,
    0xa2, 0x26, 0xba, 0x74, 0x16, 0x71, 0x95, 0xff, 0x29, 0x9d, 0x04, 0x23, 0x50, 0x9a, 0x7d, 0xc8,
];
fn png(asset: AssetVersion) -> BoundReviewImage {
    BoundReviewImage::from_verified_png(
        asset,
        EvidenceRef {
            blake3: PNG_DIGEST,
            size_bytes: PNG.len() as u64,
            width: 640,
            height: 480,
        },
        EvidenceRole::Base,
        PNG.to_vec(),
    )
    .unwrap()
}
#[derive(Default)]
pub struct Evidence(AtomicUsize, AtomicUsize, AtomicBool);
impl Evidence {
    pub fn captures(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
    pub fn renders(&self) -> usize {
        self.1.load(Ordering::Relaxed)
    }
    pub fn fail_if_called(&self) {
        self.2.store(true, Ordering::Release);
    }
}
struct Staging;
impl ReviewEvidenceStaging for Staging {
    fn files(&self) -> &[PreparedEvidenceFile] {
        &[]
    }
}
#[async_trait]
impl ReviewEvidencePort for Evidence {
    async fn capture_base(
        &self,
        asset: PreparedReviewAsset,
        cancel: ReviewTaskCancellation,
    ) -> Result<BoundReviewImage, ReviewArtifactError> {
        assert!(!self.2.load(Ordering::Acquire), "review evidence captured");
        if cancel.is_cancelled() {
            return Err(ReviewArtifactError::Cancelled);
        }
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(png(asset.asset))
    }
    async fn render(
        &self,
        r: ReviewEvidenceRequest,
    ) -> Result<ReviewEvidenceResult, ReviewArtifactError> {
        assert!(!self.2.load(Ordering::Acquire), "review evidence rendered");
        r.validate()?;
        self.1.fetch_add(1, Ordering::Relaxed);
        Ok(ReviewEvidenceResult {
            base_ref: r.base.reference().clone(),
            annotated_ref: (!r.annotations.is_empty()).then(|| r.base.reference().clone()),
            annotations: r
                .annotations
                .iter()
                .map(|a| EvidenceAnnotation {
                    ordinal: a.ordinal,
                    key: a.key,
                })
                .collect(),
            staging: Arc::new(Staging),
        })
    }
}
struct Clock;
impl ClockPort for Clock {
    fn unix_millis(&self) -> i64 {
        100
    }
}
struct Codec;
impl ReviewCommandCodecPort for Codec {
    fn usage_digest(
        &self,
        declaration: &ReviewUsageDeclaration,
    ) -> Result<[u8; 32], ReviewWorkspaceError> {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        format!("{declaration:?}").hash(&mut h);
        let mut digest = [0; 32];
        digest[..8].copy_from_slice(&h.finish().to_le_bytes());
        Ok(digest)
    }
    fn digest(&self, e: &ReviewCommandEnvelope) -> Result<[u8; 32], ReviewWorkspaceError> {
        let mut clean = e.clone();
        clean.payload_digest = [0; 32];
        let mut h = std::collections::hash_map::DefaultHasher::new();
        format!("{clean:?}").hash(&mut h);
        let mut digest = [0; 32];
        digest[..8].copy_from_slice(&h.finish().to_le_bytes());
        Ok(digest)
    }
}
pub struct Fixture {
    pub repository: Arc<MemoryRepository>,
    pub authoring: Arc<MemoryAuthoringRepository>,
    pub evidence: Arc<Evidence>,
    pub assets: Arc<Assets>,
}

pub struct Importer(pub Mutex<UsageImportPreview>);
impl UsageImportPort for Importer {
    fn inspect(&self, source: &RelativePath) -> Result<UsageImportPreview, UsageImportError> {
        let preview = self.0.lock().unwrap();
        if preview.source != *source {
            return Err(UsageImportError::UnsafePath);
        }
        let mut result = preview.clone();
        result.canonical_digest = Codec
            .usage_digest(&result.declaration)
            .map_err(|_| UsageImportError::InvalidDeclaration)?;
        Ok(result)
    }
}
pub fn usage_importer(result: &ReviewApplyResult) -> Arc<Importer> {
    let state = &result.view.current.as_ref().unwrap().authoring.state;
    let target = state.target_key(state.feedback[0].targets[0].id).unwrap();
    Arc::new(Importer(Mutex::new(UsageImportPreview {
        declaration: ReviewUsageDeclaration {
            id: ReviewUsageId::from_u128(80),
            project_id: state.project_id,
            stream_id: state.stream_id,
            basis: result.receipt.snapshot,
            targets: vec![target],
            outputs: vec![],
        },
        canonical_digest: [8; 32],
        source: RelativePath::parse("handoff/usage.json").unwrap(),
        source_digest: [9; 32],
        outputs: vec![],
    })))
}
impl Fixture {
    pub fn legacy_draft(&self) -> MigrationInspection {
        let mut draft = ReviewDraft::new(
            ProjectId::from_u128(1),
            ReviewStreamId::from_u128(2),
            ReviewRoundId::from_u128(50),
            None,
            None,
            1,
            vec![asset(EntityId::from_u128(10))],
        )
        .unwrap();
        draft
            .upsert_feedback(Feedback {
                id: FeedbackId::from_u128(51),
                text: "保留这条原文".into(),
                created_at_ms: 2,
                targets: vec![FeedbackTarget {
                    asset_version_id: draft.assets[0].id,
                    anchor: FeedbackAnchor::Asset,
                }],
            })
            .unwrap();
        let inspection = MigrationInspection {
            legacy_protocol: ReviewProtocolVersion::V2,
            index_digest: [1; 32],
            inspection_digest: [2; 32],
            legacy_records: vec![LegacyReviewReference {
                stream_id: draft.review_stream_id,
                round_id: draft.review_round_id,
                protocol: ReviewProtocolVersion::V2,
                is_draft: true,
                blake3: [3; 32],
            }],
            active_draft: Some(PersistedReviewDraft {
                protocol_version: ReviewProtocolVersion::V2,
                draft,
            }),
            completed_candidates: vec![],
            limitations: vec![
                ReviewHistoryLimitation::LegacyEvidenceAbsent,
                ReviewHistoryLimitation::ExternalCopiesCannotBeRevoked,
            ],
        };
        *self.repository.migration.lock().unwrap() = Some(inspection.clone());
        inspection
    }
    pub fn new() -> Self {
        Self {
            repository: Arc::default(),
            authoring: Arc::default(),
            evidence: Arc::default(),
            assets: Arc::default(),
        }
    }
    pub fn service(&self) -> ContinuousReviewService {
        ContinuousReviewService::new(
            ReviewWorkspaceContext {
                project_id: ProjectId::from_u128(1),
                stream_id: ReviewStreamId::from_u128(2),
                production: None,
            },
            Arc::new(Provider(self.repository.clone(), self.authoring.clone())),
            self.assets.clone(),
            self.evidence.clone(),
            Arc::new(Codec),
            Arc::new(Clock),
        )
    }
    pub fn authoring_service(&self) -> ContinuousReviewService {
        self.service()
    }
}
