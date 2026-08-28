use async_trait::async_trait;
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
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
    pub fail_commit: Mutex<Option<ReviewCommitError>>,
    pub fail_view_after_commit: Mutex<bool>,
}
impl MemoryRepository {
    pub fn current(&self) -> Option<StoredContinuousSnapshot> {
        self.states.lock().unwrap().last().cloned()
    }
}
impl ContinuousReviewRepositoryPort for MemoryRepository {
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
        state.parent = r.expected;
        for archive in r.archives {
            self.archives
                .lock()
                .unwrap()
                .insert(archive.archive_id, archive);
        }
        states.push(StoredContinuousSnapshot {
            reference,
            production: r.production,
            state,
            command_id: receipt.command_id,
            payload_digest: receipt.payload_digest,
            changes: r.next.changes,
            evidence: r.next.evidence,
        });
        if failure == Some(ReviewCommitError::OutcomeUnknown) {
            Err(ReviewCommitError::OutcomeUnknown)
        } else {
            Ok(receipt)
        }
    }
}
struct Provider(Arc<MemoryRepository>);
impl ContinuousReviewRepositoryProviderPort for Provider {
    fn open_reader(&self) -> Result<Arc<dyn ContinuousReviewRepositoryPort>, ReviewCommitError> {
        Ok(self.0.clone())
    }
    fn open_writer(&self) -> Result<Arc<dyn ContinuousReviewRepositoryPort>, ReviewCommitError> {
        Ok(self.0.clone())
    }
}
#[derive(Default)]
pub struct Assets {
    pub changed: Mutex<bool>,
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
        assets: &[AssetVersion],
        cancel: ReviewTaskCancellation,
    ) -> Result<Vec<SourceCheck>, ReviewAssetError> {
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
pub struct Evidence(AtomicUsize, AtomicUsize);
impl Evidence {
    pub fn captures(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
    pub fn renders(&self) -> usize {
        self.1.load(Ordering::Relaxed)
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
    pub evidence: Arc<Evidence>,
    pub assets: Arc<Assets>,
}
impl Fixture {
    pub fn new() -> Self {
        Self {
            repository: Arc::default(),
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
            Arc::new(Provider(self.repository.clone())),
            self.assets.clone(),
            self.evidence.clone(),
            Arc::new(Codec),
            Arc::new(Clock),
        )
    }
}
