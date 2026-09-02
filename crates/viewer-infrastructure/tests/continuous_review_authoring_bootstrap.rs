use rusqlite::Connection;
use std::{collections::BTreeMap, fs, path::Path};
use tempfile::TempDir;
use viewer_application::{ProjectAccess, review_workspace::*};
use viewer_domain::{
    AssetVersionId, FeedbackId, ProjectId, RelativePath, ReviewCommandId, ReviewSnapshotId,
    ReviewStreamId, ReviewTargetId, ReviewTargetRevisionId, ReviewTextRevisionId,
    review::{AssetEvidence, AssetVersion, ReviewMedia, continuous::*},
};
use viewer_infrastructure::{
    portable::PortableProjectMetadata, review::ProjectReviewRepositoryProvider,
};

struct BootstrapFixture {
    root: TempDir,
    project_id: ProjectId,
    stream: ReviewStreamId,
    provider: ProjectReviewRepositoryProvider,
}

impl BootstrapFixture {
    fn verified_v3() -> Self {
        let root = TempDir::new().unwrap();
        let metadata = PortableProjectMetadata::open(root.path(), ProjectAccess::ReadWrite, 1)
            .expect("create portable metadata");
        let project_id = metadata.project_id();
        drop(metadata);
        let stream = ReviewStreamId::from_u128(2);
        let provider = ProjectReviewRepositoryProvider::new(root.path(), project_id);
        let writer = provider.continuous_writer().unwrap();
        let snapshot_id = ReviewSnapshotId::from_u128(3);
        let asset = AssetVersion {
            id: AssetVersionId::from_u128(10),
            source_entity_id: None,
            relative_path: RelativePath::parse("clip.mp4").unwrap(),
            evidence: AssetEvidence {
                size_bytes: 100,
                modified_ns: 200,
                blake3: Some([9; 32]),
            },
            media: ReviewMedia::Video {
                duration_us: Some(1_000_000),
                display_width: Some(640),
                display_height: Some(480),
            },
            producer_asset_id: None,
            parent_asset_version_id: None,
        };
        let feedback = VersionedFeedback {
            id: FeedbackId::from_u128(20),
            text_revision_id: ReviewTextRevisionId::from_u128(21),
            text: "收紧袖口".into(),
            created_at_ms: 2,
            targets: vec![VersionedTarget::asset(
                ReviewTargetId::from_u128(30),
                ReviewTargetRevisionId::from_u128(31),
                asset.id,
            )],
            history_ref: None,
        };
        let key = TargetVersionKey {
            feedback_id: feedback.id,
            text_revision_id: feedback.text_revision_id,
            target_id: feedback.targets[0].id,
            target_revision_id: feedback.targets[0].revision_id,
        };
        let mut state = ContinuousReviewState::empty(project_id, stream, snapshot_id);
        state.assets = vec![asset.clone()];
        state.feedback = vec![feedback.clone()];
        writer
            .commit(ReviewCommitRequest {
                expected: None,
                production: None,
                next: PreparedContinuousSnapshot {
                    state,
                    command_id: ReviewCommandId::from_u128(4),
                    payload_digest: [5; 32],
                    changes: vec![ReviewChange {
                        target_id: feedback.targets[0].id,
                        before: None,
                        after: Some(key),
                        kind: ReviewChangeKind::Added,
                        archive_id: None,
                        historical_key: None,
                    }],
                    evidence: vec![ReviewEvidenceBinding {
                        asset_version_id: asset.id,
                        capability: EvidenceCapability::NotImage,
                    }],
                },
                archives: vec![],
                adopted_usage: vec![],
                staged_evidence: vec![],
            })
            .unwrap();
        drop(writer);
        Self {
            root,
            project_id,
            stream,
            provider,
        }
    }

    fn review_tree_bytes(&self) -> BTreeMap<String, Vec<u8>> {
        collect_files(&self.root.path().join(".viewer/reviews"))
    }

    fn materialization_job_count(&self) -> i64 {
        Connection::open(self.root.path().join(".viewer/metadata.sqlite"))
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM review_materialization_jobs",
                [],
                |row| row.get(0),
            )
            .unwrap()
    }
}

#[test]
fn bootstrap_preserves_every_existing_v3_byte_and_is_idempotent() {
    let fixture = BootstrapFixture::verified_v3();
    let before = fixture.review_tree_bytes();

    let first = fixture
        .provider
        .bootstrap_authoring(fixture.stream)
        .unwrap();
    let second = fixture
        .provider
        .bootstrap_authoring(fixture.stream)
        .unwrap();

    assert_eq!(first, second);
    assert_eq!(first.authoring, first.published);
    assert_eq!(fixture.review_tree_bytes(), before);
    assert_eq!(fixture.materialization_job_count(), 0);
    let current = fixture
        .provider
        .authoring_reader()
        .unwrap()
        .load_current(fixture.stream)
        .unwrap()
        .unwrap();
    assert_eq!(current.head, first.authoring.unwrap());
    assert_eq!(current.state.snapshot_id, ReviewSnapshotId::from_u128(3));
    assert_eq!(current.command_id, ReviewCommandId::from_u128(4));
    assert_eq!(current.payload_digest, [5; 32]);
    assert_eq!(current.state.project_id, fixture.project_id);
    assert_eq!(current.state.feedback[0].text, "收紧袖口");
}

#[test]
fn empty_stream_bootstraps_two_null_heads_without_a_job() {
    let root = TempDir::new().unwrap();
    let metadata = PortableProjectMetadata::open(root.path(), ProjectAccess::ReadWrite, 1).unwrap();
    let project_id = metadata.project_id();
    drop(metadata);
    let stream = ReviewStreamId::from_u128(20);
    let provider = ProjectReviewRepositoryProvider::new(root.path(), project_id);

    let heads = provider.bootstrap_authoring(stream).unwrap();

    assert_eq!(
        heads,
        ReviewHeads {
            authoring: None,
            published: None
        }
    );
    assert_eq!(
        Connection::open(root.path().join(".viewer/metadata.sqlite"))
            .unwrap()
            .query_row("SELECT COUNT(*) FROM review_authoring_streams", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        1
    );
    assert_eq!(
        Connection::open(root.path().join(".viewer/metadata.sqlite"))
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM review_materialization_jobs",
                [],
                |row| { row.get::<_, i64>(0) }
            )
            .unwrap(),
        0
    );
}

#[test]
fn read_only_bootstrap_does_not_modify_review_or_authoring_data() {
    let fixture = BootstrapFixture::verified_v3();
    let before_review = fixture.review_tree_bytes();
    let before_database = fs::read(fixture.root.path().join(".viewer/metadata.sqlite")).unwrap();
    let provider = ProjectReviewRepositoryProvider::new_with_access(
        fixture.root.path(),
        fixture.project_id,
        ProjectAccess::ReadOnly,
    );

    assert_eq!(
        provider.bootstrap_authoring(fixture.stream),
        Err(ReviewCommitError::ReadOnly)
    );
    assert_eq!(fixture.review_tree_bytes(), before_review);
    assert_eq!(
        fs::read(fixture.root.path().join(".viewer/metadata.sqlite")).unwrap(),
        before_database
    );
}

#[test]
fn injected_sqlite_failure_rolls_back_and_retry_bootstraps_once() {
    let fixture = BootstrapFixture::verified_v3();
    let database = fixture.root.path().join(".viewer/metadata.sqlite");
    let connection = Connection::open(&database).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_authoring_bootstrap
             BEFORE INSERT ON review_authoring_snapshots
             BEGIN
               SELECT RAISE(ABORT, 'injected');
             END;",
        )
        .unwrap();
    drop(connection);

    assert_eq!(
        fixture.provider.bootstrap_authoring(fixture.stream),
        Err(ReviewCommitError::Integrity)
    );
    let connection = Connection::open(&database).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM review_authoring_streams", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        0
    );
    connection
        .execute_batch("DROP TRIGGER fail_authoring_bootstrap")
        .unwrap();
    drop(connection);

    let heads = fixture
        .provider
        .bootstrap_authoring(fixture.stream)
        .unwrap();
    assert_eq!(heads.authoring, heads.published);
    assert_eq!(fixture.materialization_job_count(), 0);
}

#[test]
fn legacy_review_data_still_requires_explicit_migration() {
    let root = TempDir::new().unwrap();
    let viewer = root.path().join(".viewer");
    let reviews = viewer.join("reviews");
    fs::create_dir_all(reviews.join("rounds")).unwrap();
    let project_id: ProjectId = "00000000-0000-4000-8000-000000000001".parse().unwrap();
    fs::write(
        viewer.join("project.json"),
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "projectId": project_id.to_string(),
            "createdAtMs": 1,
        }))
        .unwrap(),
    )
    .unwrap();
    let mut index: serde_json::Value = serde_json::from_slice(
        &fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/review-protocol/review-index-v1.valid.json"),
        )
        .unwrap(),
    )
    .unwrap();
    index["streams"].as_array_mut().unwrap().remove(0);
    fs::write(
        reviews.join("index.json"),
        serde_json::to_vec(&index).unwrap(),
    )
    .unwrap();
    for name in ["review-round-v1.valid.json"] {
        let bytes = fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/review-protocol")
                .join(name),
        )
        .unwrap();
        let round = viewer_infrastructure::review::decode_completed(&bytes).unwrap();
        fs::write(
            reviews.join(format!("rounds/{}.json", round.review_round_id)),
            bytes,
        )
        .unwrap();
    }
    let metadata = PortableProjectMetadata::open(root.path(), ProjectAccess::ReadWrite, 1).unwrap();
    drop(metadata);
    let provider = ProjectReviewRepositoryProvider::new(root.path(), project_id);

    assert_eq!(
        provider.bootstrap_authoring("00000000-0000-4000-8000-000000000102".parse().unwrap()),
        Err(ReviewCommitError::MigrationRequired)
    );
    assert_eq!(
        Connection::open(root.path().join(".viewer/metadata.sqlite"))
            .unwrap()
            .query_row("SELECT COUNT(*) FROM review_authoring_streams", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        0
    );
}

fn collect_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut result = BTreeMap::new();
    if !root.exists() {
        return result;
    }
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            let file_type = entry.file_type().unwrap();
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                result.insert(
                    entry
                        .path()
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    result
}
