use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::{fs, path::Path};
use tempfile::TempDir;
use viewer_application::{
    PersistedReviewDraft, ProjectAccess, ReviewArtifactAnnotation, ReviewCatalog,
    ReviewProtocolVersion, ReviewPublication, ReviewRenderedArtifact, ReviewRepositoryError,
    ReviewRepositoryPort, ReviewRepositoryProviderPort, ReviewStreamHead,
};
use viewer_domain::review::{
    AssetEvidence, AssetVersion, Feedback, FeedbackAnchor, FeedbackTarget, NormalizedRect,
    ReviewDraft, ReviewMedia, ReviewSnapshot,
};
use viewer_domain::{
    AssetVersionId, FeedbackId, ProjectId, RelativePath, ReviewRoundId, ReviewStreamId,
};
use viewer_infrastructure::review::{
    ProjectReviewRepository, ProjectReviewRepositoryProvider, ReviewRepositoryAccess,
    ReviewRepositoryFaultInjector, ReviewRepositoryFaultPoint, encode_catalog, encode_completed,
    encode_draft, encode_draft_v2,
};

struct PersistentProject {
    directory: TempDir,
    project_id: ProjectId,
}

impl PersistentProject {
    fn new() -> Self {
        Self::with_project_id(ProjectId::from_u128(1))
    }

    fn with_project_id(project_id: ProjectId) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let viewer = directory.path().join(".viewer");
        fs::create_dir(&viewer).unwrap();
        fs::write(viewer.join("project.json"), b"manifest sentinel").unwrap();
        fs::write(viewer.join("metadata.sqlite"), b"database sentinel").unwrap();
        Self {
            directory,
            project_id,
        }
    }

    fn root(&self) -> &Path {
        self.directory.path()
    }

    fn reviews(&self) -> std::path::PathBuf {
        self.root().join(".viewer/reviews")
    }
}

fn legacy_fixture_project(copy_round: bool) -> (PersistentProject, std::path::PathBuf) {
    let project =
        PersistentProject::with_project_id("00000000-0000-4000-8000-000000000001".parse().unwrap());
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/review-protocol/project/.viewer/reviews");
    fs::create_dir_all(project.reviews().join("rounds")).unwrap();
    let mut index: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.join("index.json")).unwrap()).unwrap();
    index["streams"].as_array_mut().unwrap().remove(0);
    fs::write(
        project.reviews().join("index.json"),
        format!("{}\n", serde_json::to_string_pretty(&index).unwrap()),
    )
    .unwrap();
    let round_path = project
        .reviews()
        .join("rounds/00000000-0000-4000-8000-000000000202.json");
    if copy_round {
        fs::copy(
            fixture.join("rounds/00000000-0000-4000-8000-000000000202.json"),
            &round_path,
        )
        .unwrap();
    }
    (project, round_path)
}

fn review_draft(
    project_id: ProjectId,
    stream_id: ReviewStreamId,
    round_id: ReviewRoundId,
) -> ReviewDraft {
    ReviewDraft::new(
        project_id,
        stream_id,
        round_id,
        None,
        None,
        1_000,
        vec![AssetVersion {
            id: AssetVersionId::from_u128(3),
            source_entity_id: None,
            relative_path: RelativePath::parse("renders/frame.png").unwrap(),
            evidence: AssetEvidence {
                size_bytes: 4,
                modified_ns: 123,
                blake3: None,
            },
            media: ReviewMedia::Image {
                width: Some(10),
                height: Some(20),
            },
            producer_asset_id: None,
            parent_asset_version_id: None,
        }],
    )
    .unwrap()
}

fn persisted_v1(draft: ReviewDraft) -> PersistedReviewDraft {
    PersistedReviewDraft {
        protocol_version: ReviewProtocolVersion::V1,
        draft,
    }
}

fn persisted_v2(draft: ReviewDraft) -> PersistedReviewDraft {
    PersistedReviewDraft {
        protocol_version: ReviewProtocolVersion::V2,
        draft,
    }
}

fn open_writable(
    project: &PersistentProject,
) -> Result<ProjectReviewRepository, ReviewRepositoryError> {
    ProjectReviewRepository::open(
        project.root(),
        project.project_id,
        ReviewRepositoryAccess::ReadWrite,
    )
}

fn completed_round(
    project_id: ProjectId,
    stream: u128,
    round: u128,
    previous: Option<ReviewRoundId>,
) -> ReviewSnapshot {
    let mut draft = review_draft(
        project_id,
        ReviewStreamId::from_u128(stream),
        ReviewRoundId::from_u128(round),
    );
    draft.previous_completed_round_id = previous;
    draft.complete(2_000).unwrap()
}

fn v1_publication(snapshot: ReviewSnapshot) -> ReviewPublication {
    ReviewPublication {
        protocol_version: ReviewProtocolVersion::V1,
        snapshot,
        artifacts: vec![],
    }
}

fn v2_annotated_draft(project_id: ProjectId) -> ReviewDraft {
    let mut draft = review_draft(
        project_id,
        ReviewStreamId::from_u128(21),
        ReviewRoundId::from_u128(22),
    );
    let asset_version_id = draft.assets[0].id;
    let feedback_id = FeedbackId::from_u128(23);
    draft
        .upsert_feedback(
            Feedback::new(
                feedback_id,
                "右手结构需要修正".into(),
                1_100,
                vec![FeedbackTarget {
                    asset_version_id,
                    anchor: FeedbackAnchor::ImageRect(
                        NormalizedRect::new(0.2, 0.3, 0.4, 0.2).unwrap(),
                    ),
                }],
            )
            .unwrap(),
        )
        .unwrap();
    draft
}

fn v2_publication(project_id: ProjectId) -> ReviewPublication {
    let draft = v2_annotated_draft(project_id);
    let asset_version_id = draft.assets[0].id;
    let feedback_id = draft.feedback[0].id;
    let artifact_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/images/alpha.png");
    let artifact_bytes = fs::read(&artifact_path).unwrap();
    ReviewPublication {
        protocol_version: ReviewProtocolVersion::V2,
        snapshot: draft.complete(2_000).unwrap(),
        artifacts: vec![ReviewRenderedArtifact {
            asset_version_id,
            temporary_path: artifact_path,
            media_type: "image/png".into(),
            width: 640,
            height: 480,
            size_bytes: artifact_bytes.len() as u64,
            blake3: *blake3::hash(&artifact_bytes).as_bytes(),
            annotations: vec![ReviewArtifactAnnotation {
                ordinal: 1,
                feedback_id,
            }],
        }],
    }
}

fn v2_two_artifact_publication(project_id: ProjectId) -> ReviewPublication {
    let first = review_draft(
        project_id,
        ReviewStreamId::from_u128(31),
        ReviewRoundId::from_u128(32),
    );
    let mut second_asset = first.assets[0].clone();
    second_asset.id = AssetVersionId::from_u128(4);
    second_asset.relative_path = RelativePath::parse("renders/frame-2.png").unwrap();
    let mut draft = ReviewDraft::new(
        project_id,
        ReviewStreamId::from_u128(31),
        ReviewRoundId::from_u128(32),
        None,
        None,
        1_000,
        vec![first.assets[0].clone(), second_asset],
    )
    .unwrap();
    for (ordinal, asset) in draft.assets.clone().into_iter().enumerate() {
        draft
            .upsert_feedback(
                Feedback::new(
                    FeedbackId::from_u128(40 + ordinal as u128),
                    format!("修正第 {} 张图", ordinal + 1),
                    1_100 + ordinal as i64,
                    vec![FeedbackTarget {
                        asset_version_id: asset.id,
                        anchor: FeedbackAnchor::ImageRect(
                            NormalizedRect::new(0.1, 0.1, 0.2, 0.2).unwrap(),
                        ),
                    }],
                )
                .unwrap(),
            )
            .unwrap();
    }
    let artifact_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/images/alpha.png");
    let artifact_bytes = fs::read(&artifact_path).unwrap();
    let artifacts = draft
        .assets
        .iter()
        .zip(&draft.feedback)
        .map(|(asset, feedback)| ReviewRenderedArtifact {
            asset_version_id: asset.id,
            temporary_path: artifact_path.clone(),
            media_type: "image/png".into(),
            width: 640,
            height: 480,
            size_bytes: artifact_bytes.len() as u64,
            blake3: *blake3::hash(&artifact_bytes).as_bytes(),
            annotations: vec![ReviewArtifactAnnotation {
                ordinal: 1,
                feedback_id: feedback.id,
            }],
        })
        .collect();
    ReviewPublication {
        protocol_version: ReviewProtocolVersion::V2,
        snapshot: draft.complete(2_000).unwrap(),
        artifacts,
    }
}

fn stream(catalog: &ReviewCatalog, id: u128) -> &ReviewStreamHead {
    catalog
        .streams
        .iter()
        .find(|stream| stream.review_stream_id == ReviewStreamId::from_u128(id))
        .unwrap()
}

struct FailOnce {
    point: ReviewRepositoryFaultPoint,
    fired: AtomicBool,
}

struct FailOnCall {
    point: ReviewRepositoryFaultPoint,
    target_call: usize,
    calls: AtomicUsize,
}

impl ReviewRepositoryFaultInjector for FailOnCall {
    fn check(&self, point: ReviewRepositoryFaultPoint) -> Result<(), ReviewRepositoryError> {
        if point == self.point && self.calls.fetch_add(1, Ordering::SeqCst) + 1 == self.target_call
        {
            Err(ReviewRepositoryError::Unavailable)
        } else {
            Ok(())
        }
    }
}

fn fail_on_call(
    point: ReviewRepositoryFaultPoint,
    target_call: usize,
) -> Arc<dyn ReviewRepositoryFaultInjector> {
    Arc::new(FailOnCall {
        point,
        target_call,
        calls: AtomicUsize::new(0),
    })
}

impl ReviewRepositoryFaultInjector for FailOnce {
    fn check(&self, point: ReviewRepositoryFaultPoint) -> Result<(), ReviewRepositoryError> {
        if point == self.point && !self.fired.swap(true, Ordering::SeqCst) {
            Err(ReviewRepositoryError::Unavailable)
        } else {
            Ok(())
        }
    }
}

fn fail_once(point: ReviewRepositoryFaultPoint) -> Arc<dyn ReviewRepositoryFaultInjector> {
    Arc::new(FailOnce {
        point,
        fired: AtomicBool::new(false),
    })
}

fn open_writable_with_faults(
    project: &PersistentProject,
    faults: Arc<dyn ReviewRepositoryFaultInjector>,
) -> Result<ProjectReviewRepository, ReviewRepositoryError> {
    ProjectReviewRepository::open_with_faults(
        project.root(),
        project.project_id,
        ReviewRepositoryAccess::ReadWrite,
        faults,
    )
}

#[test]
fn v2_publish_recovers_after_bundle_rename_before_catalog_replace() {
    let project = PersistentProject::new();
    let publication = v2_publication(project.project_id);
    let round_id = publication.snapshot.review_round_id;
    let repository = open_writable_with_faults(
        &project,
        fail_once(ReviewRepositoryFaultPoint::AfterBundleDurableBeforeIndex),
    )
    .unwrap();

    assert_eq!(
        repository.publish(&publication),
        Err(ReviewRepositoryError::Unavailable)
    );
    drop(repository);

    let reopened = open_writable(&project).unwrap();
    let catalog = reopened.load_catalog().unwrap();
    assert!(
        catalog.streams[0]
            .completed_round_ids()
            .any(|candidate| candidate == round_id)
    );
    assert!(
        project
            .reviews()
            .join(format!("rounds/{round_id}/round.json"))
            .is_file()
    );
    assert_eq!(
        reopened
            .load_completed(
                publication.snapshot.review_stream_id,
                publication.snapshot.review_round_id,
            )
            .unwrap(),
        Some(publication.snapshot)
    );
}

#[test]
fn v2_publish_rejects_artifact_digest_or_path_substitution() {
    let project = PersistentProject::new();
    let mut publication = v2_publication(project.project_id);
    publication.artifacts[0].blake3 = [0; 32];

    assert_eq!(
        open_writable(&project).unwrap().publish(&publication),
        Err(ReviewRepositoryError::InvalidData)
    );
    assert!(
        fs::read_dir(project.reviews().join("rounds"))
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn every_v2_durable_boundary_reopens_to_one_consistent_state() {
    for (point, bundle_is_durable) in [
        (ReviewRepositoryFaultPoint::AfterArtifactFileSync, false),
        (ReviewRepositoryFaultPoint::AfterRoundManifestSync, false),
        (ReviewRepositoryFaultPoint::AfterBundleDirectorySync, false),
        (
            ReviewRepositoryFaultPoint::AfterBundleDurableBeforeIndex,
            true,
        ),
        (ReviewRepositoryFaultPoint::BeforeIndexReplace, true),
        (ReviewRepositoryFaultPoint::AfterIndexReplace, true),
    ] {
        let project = PersistentProject::new();
        let draft = v2_annotated_draft(project.project_id);
        let publication = v2_publication(project.project_id);
        let round_id = publication.snapshot.review_round_id;
        let repository = open_writable_with_faults(&project, fail_once(point)).unwrap();
        repository.save_draft(&persisted_v2(draft)).unwrap();

        assert_eq!(
            repository.publish(&publication),
            Err(ReviewRepositoryError::Unavailable),
            "fault {point:?} did not interrupt publication"
        );
        drop(repository);

        if !bundle_is_durable {
            let reader = ProjectReviewRepository::open(
                project.root(),
                project.project_id,
                ReviewRepositoryAccess::ReadOnly,
            )
            .unwrap();
            assert!(reader.load_catalog().unwrap().streams.is_empty());
            assert!(reader.load_active_draft().unwrap().is_some());
            drop(reader);
        }

        let reopened = open_writable(&project).unwrap();
        let catalog = reopened.load_catalog().unwrap();
        if bundle_is_durable {
            assert!(
                catalog.streams[0]
                    .completed_round_ids()
                    .any(|candidate| candidate == round_id)
            );
            assert!(reopened.load_active_draft().unwrap().is_none());
            assert!(
                project
                    .reviews()
                    .join(format!("rounds/{round_id}/round.json"))
                    .is_file()
            );
        } else {
            assert!(catalog.streams.is_empty());
            assert!(reopened.load_active_draft().unwrap().is_some());
            assert!(
                !project
                    .reviews()
                    .join(format!("rounds/{round_id}"))
                    .exists()
            );
            assert!(
                fs::read_dir(project.reviews().join("rounds"))
                    .unwrap()
                    .next()
                    .is_none()
            );
        }
    }
}

#[test]
fn every_artifact_file_sync_boundary_leaves_only_a_cleanup_safe_transaction() {
    for target_call in [1, 2] {
        let project = PersistentProject::new();
        let publication = v2_two_artifact_publication(project.project_id);
        let repository = open_writable_with_faults(
            &project,
            fail_on_call(
                ReviewRepositoryFaultPoint::AfterArtifactFileSync,
                target_call,
            ),
        )
        .unwrap();

        assert_eq!(
            repository.publish(&publication),
            Err(ReviewRepositoryError::Unavailable)
        );
        let temporary = fs::read_dir(project.reviews().join("rounds"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(
            fs::read_dir(temporary.join("artifacts")).unwrap().count(),
            target_call
        );
        drop(repository);

        let reopened = open_writable(&project).unwrap();
        assert!(reopened.load_catalog().unwrap().streams.is_empty());
        assert!(
            fs::read_dir(project.reviews().join("rounds"))
                .unwrap()
                .next()
                .is_none()
        );
    }
}

#[test]
fn v2_publish_rejects_invalid_artifact_shape_and_annotation_mapping() {
    type PublicationMutation = Box<dyn Fn(&mut ReviewPublication)>;
    let cases: Vec<PublicationMutation> = vec![
        Box::new(|publication| publication.artifacts[0].media_type = "image/jpeg".into()),
        Box::new(|publication| publication.artifacts[0].width = 0),
        Box::new(|publication| publication.artifacts[0].width = 16_777_217),
        Box::new(|publication| publication.artifacts[0].size_bytes = 64 * 1024 * 1024 + 1),
        Box::new(|publication| publication.artifacts[0].annotations[0].ordinal = 2),
        Box::new(|publication| {
            publication.artifacts[0].annotations[0].feedback_id = FeedbackId::from_u128(999)
        }),
        Box::new(|publication| publication.artifacts.push(publication.artifacts[0].clone())),
    ];

    for mutate in cases {
        let project = PersistentProject::new();
        let mut publication = v2_publication(project.project_id);
        mutate(&mut publication);

        assert!(matches!(
            open_writable(&project).unwrap().publish(&publication),
            Err(ReviewRepositoryError::InvalidData | ReviewRepositoryError::LimitExceeded)
        ));
        assert!(
            fs::read_dir(project.reviews().join("rounds"))
                .unwrap()
                .next()
                .is_none()
        );
    }
}

#[test]
fn indexed_v2_bundle_tampering_requires_recovery_without_rewriting_catalog() {
    for tamper in ["digest", "extra"] {
        let project = PersistentProject::new();
        let publication = v2_publication(project.project_id);
        let round_id = publication.snapshot.review_round_id;
        let asset_id = publication.artifacts[0].asset_version_id;
        let repository = open_writable(&project).unwrap();
        repository.publish(&publication).unwrap();
        drop(repository);
        let index_path = project.reviews().join("index.json");
        let index_before = fs::read(&index_path).unwrap();
        let artifacts = project
            .reviews()
            .join(format!("rounds/{round_id}/artifacts"));
        if tamper == "digest" {
            let path = artifacts.join(format!("{asset_id}-annotation.png"));
            let mut bytes = fs::read(&path).unwrap();
            bytes.push(0);
            fs::write(path, bytes).unwrap();
        } else {
            fs::write(artifacts.join("unexpected.png"), b"unexpected").unwrap();
        }

        assert_eq!(
            open_writable(&project).err(),
            Some(ReviewRepositoryError::RecoveryRequired),
            "accepted {tamper} tampering"
        );
        assert_eq!(fs::read(index_path).unwrap(), index_before);
    }
}

#[cfg(unix)]
#[test]
fn indexed_v2_artifact_symlink_is_never_followed() {
    use std::os::unix::fs::symlink;

    let project = PersistentProject::new();
    let publication = v2_publication(project.project_id);
    let round_id = publication.snapshot.review_round_id;
    let asset_id = publication.artifacts[0].asset_version_id;
    let repository = open_writable(&project).unwrap();
    repository.publish(&publication).unwrap();
    drop(repository);
    let artifact = project.reviews().join(format!(
        "rounds/{round_id}/artifacts/{asset_id}-annotation.png"
    ));
    fs::remove_file(&artifact).unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    symlink(outside.path(), artifact).unwrap();

    assert_eq!(
        open_writable(&project).err(),
        Some(ReviewRepositoryError::RecoveryRequired)
    );
}

#[test]
fn malformed_transaction_directory_is_not_deleted_as_owned_state() {
    let project = PersistentProject::new();
    drop(open_writable(&project).unwrap());
    let round_id = ReviewRoundId::from_u128(22);
    let temporary = project
        .reviews()
        .join(format!("rounds/.{round_id}.tmp-000000000000000a"));
    fs::create_dir(&temporary).unwrap();
    fs::write(temporary.join("not-owned.txt"), b"keep").unwrap();
    let index_path = project.reviews().join("index.json");
    let index_before = fs::read(&index_path).unwrap();

    assert_eq!(
        open_writable(&project).err(),
        Some(ReviewRepositoryError::RecoveryRequired)
    );
    assert!(temporary.join("not-owned.txt").is_file());
    assert_eq!(fs::read(index_path).unwrap(), index_before);
}

#[test]
fn v1_publication_rejects_artifacts_and_keeps_the_v1_catalog_protocol() {
    let project = PersistentProject::new();
    let repository = open_writable(&project).unwrap();
    let snapshot = completed_round(project.project_id, 1, 11, None);
    let mut invalid = v1_publication(snapshot.clone());
    invalid.artifacts = v2_publication(project.project_id).artifacts;
    assert_eq!(
        repository.publish(&invalid),
        Err(ReviewRepositoryError::InvalidData)
    );

    repository.publish(&v1_publication(snapshot)).unwrap();

    let index: serde_json::Value =
        serde_json::from_slice(&fs::read(project.reviews().join("index.json")).unwrap()).unwrap();
    assert_eq!(index["protocolVersion"], "viewer.review/1");
}

#[test]
fn first_successful_v2_publish_atomically_migrates_legacy_catalog_records() {
    let (project, legacy_round) = legacy_fixture_project(true);
    let legacy_bytes = fs::read(&legacy_round).unwrap();
    let v1_index_before = fs::read(project.reviews().join("index.json")).unwrap();
    let publication = v2_publication(project.project_id);
    let repository = open_writable(&project).unwrap();

    repository.publish(&publication).unwrap();

    let index_bytes = fs::read(project.reviews().join("index.json")).unwrap();
    let index: serde_json::Value = serde_json::from_slice(&index_bytes).unwrap();
    assert_ne!(index_bytes, v1_index_before);
    assert_eq!(index["protocolVersion"], "viewer.review/2");
    let catalog = repository.load_catalog().unwrap();
    let legacy_record = catalog
        .streams
        .iter()
        .flat_map(|stream| &stream.completed_rounds)
        .find(|record| record.protocol_version == ReviewProtocolVersion::V1)
        .unwrap();
    assert_eq!(
        legacy_record.blake3,
        *blake3::hash(&legacy_bytes).as_bytes()
    );
    assert_eq!(fs::read(legacy_round).unwrap(), legacy_bytes);
}

#[test]
fn reading_v1_catalog_projects_v2_records_without_rewriting_history() {
    let (project, legacy_round) = legacy_fixture_project(true);
    let index_path = project.reviews().join("index.json");
    let index_before = fs::read(&index_path).unwrap();
    let round_before = fs::read(&legacy_round).unwrap();

    let reader = ProjectReviewRepository::open(
        project.root(),
        project.project_id,
        ReviewRepositoryAccess::ReadOnly,
    )
    .unwrap();
    let readonly_catalog = reader.load_catalog().unwrap();
    drop(reader);
    let repository = open_writable(&project).unwrap();
    let catalog = repository.load_catalog().unwrap();
    let record = &catalog.streams[0].completed_rounds[0];

    assert_eq!(readonly_catalog, catalog);
    assert_eq!(record.protocol_version, ReviewProtocolVersion::V1);
    assert_eq!(record.blake3, *blake3::hash(&round_before).as_bytes());
    assert_eq!(fs::read(index_path).unwrap(), index_before);
    assert_eq!(fs::read(legacy_round).unwrap(), round_before);
}

#[test]
fn invalid_v1_history_requires_recovery_and_never_rewrites_the_index() {
    for corruption in ["missing", "malformed", "identity"] {
        let (project, round_path) = legacy_fixture_project(corruption != "missing");
        if corruption == "malformed" {
            fs::write(&round_path, b"{}\n").unwrap();
        } else if corruption == "identity" {
            let mut value: serde_json::Value =
                serde_json::from_slice(&fs::read(&round_path).unwrap()).unwrap();
            value["reviewRoundId"] =
                serde_json::Value::String(ReviewRoundId::from_u128(999).to_string());
            fs::write(
                &round_path,
                format!("{}\n", serde_json::to_string_pretty(&value).unwrap()),
            )
            .unwrap();
        }
        let index_path = project.reviews().join("index.json");
        let index_before = fs::read(&index_path).unwrap();

        assert_eq!(
            open_writable(&project).err(),
            Some(ReviewRepositoryError::RecoveryRequired),
            "accepted {corruption} v1 history"
        );
        assert_eq!(fs::read(index_path).unwrap(), index_before);
    }
}

#[cfg(unix)]
#[test]
fn symlinked_v1_history_requires_recovery_without_following_or_rewriting() {
    use std::os::unix::fs::symlink;

    let (project, round_path) = legacy_fixture_project(false);
    let outside = tempfile::NamedTempFile::new().unwrap();
    symlink(outside.path(), &round_path).unwrap();
    let index_path = project.reviews().join("index.json");
    let index_before = fs::read(&index_path).unwrap();

    assert_eq!(
        open_writable(&project).err(),
        Some(ReviewRepositoryError::RecoveryRequired)
    );
    assert_eq!(fs::read(index_path).unwrap(), index_before);
}

#[test]
fn v1_digest_change_after_projection_aborts_migration_without_rewriting_index() {
    let (project, round_path) = legacy_fixture_project(true);
    let repository = open_writable(&project).unwrap();
    let catalog_document = repository.load_catalog().unwrap();
    let index_path = project.reviews().join("index.json");
    let index_before = fs::read(&index_path).unwrap();
    let mut changed = fs::read(&round_path).unwrap();
    changed.push(b' ');
    fs::write(&round_path, changed).unwrap();

    assert_eq!(
        repository.prepare_v2_catalog_migration(&catalog_document),
        Err(ReviewRepositoryError::RecoveryRequired)
    );
    assert_eq!(fs::read(index_path).unwrap(), index_before);
}

#[test]
fn writable_repository_creates_only_review_paths_and_round_trips_a_draft() {
    let project = PersistentProject::new();
    let manifest_before = fs::read(project.root().join(".viewer/project.json")).unwrap();
    let database_before = fs::read(project.root().join(".viewer/metadata.sqlite")).unwrap();
    let repository = open_writable(&project).unwrap();
    let draft = review_draft(
        project.project_id,
        ReviewStreamId::from_u128(1),
        ReviewRoundId::from_u128(2),
    );

    repository.save_draft(&persisted_v1(draft.clone())).unwrap();

    assert_eq!(
        repository
            .load_draft(draft.review_stream_id, draft.review_round_id)
            .unwrap(),
        Some(persisted_v1(draft))
    );
    assert_eq!(
        fs::read(project.root().join(".viewer/project.json")).unwrap(),
        manifest_before
    );
    assert_eq!(
        fs::read(project.root().join(".viewer/metadata.sqlite")).unwrap(),
        database_before
    );
    assert!(project.reviews().join("index.json").is_file());
    assert!(project.reviews().join("write.lock").is_file());
    assert!(project.reviews().join("rounds").is_dir());
    assert!(
        project
            .reviews()
            .join("drafts/00000000-0000-0000-0000-000000000002.json")
            .is_file()
    );
}

#[test]
fn readonly_absent_repository_creates_nothing_and_rejects_writes() {
    let project = PersistentProject::new();
    let repository = ProjectReviewRepository::open(
        project.root(),
        project.project_id,
        ReviewRepositoryAccess::ReadOnly,
    )
    .unwrap();
    let draft = review_draft(
        project.project_id,
        ReviewStreamId::from_u128(1),
        ReviewRoundId::from_u128(2),
    );

    assert_eq!(repository.load_catalog().unwrap().streams, vec![]);
    assert_eq!(
        repository.save_draft(&persisted_v1(draft)),
        Err(ReviewRepositoryError::ReadOnly)
    );
    assert!(!project.reviews().exists());
}

#[test]
fn readonly_provider_without_viewer_metadata_is_an_empty_side_effect_free_reader() {
    let directory = tempfile::tempdir().unwrap();
    let project_id = ProjectId::from_u128(1);
    let provider = ProjectReviewRepositoryProvider::new_with_access(
        directory.path(),
        project_id,
        ProjectAccess::ReadOnly,
    );

    let inspection = provider.inspect().unwrap();

    assert_eq!(inspection.catalog.project_id, project_id);
    assert!(inspection.catalog.streams.is_empty());
    assert!(inspection.active_draft.is_none());
    assert!(provider.open_reader().is_ok());
    assert!(matches!(
        provider.open_writer(),
        Err(ReviewRepositoryError::ReadOnly)
    ));
    assert!(!directory.path().join(".viewer").exists());
}

#[test]
fn second_writer_is_busy_until_the_first_lease_drops() {
    let project = PersistentProject::new();
    let first = open_writable(&project).unwrap();

    assert!(matches!(
        open_writable(&project),
        Err(ReviewRepositoryError::Busy)
    ));
    drop(first);
    assert!(open_writable(&project).is_ok());
}

#[test]
fn draft_project_and_requested_identity_must_match_the_repository() {
    let project = PersistentProject::new();
    let repository = open_writable(&project).unwrap();
    let wrong_project = review_draft(
        ProjectId::from_u128(99),
        ReviewStreamId::from_u128(1),
        ReviewRoundId::from_u128(2),
    );

    assert_eq!(
        repository.save_draft(&persisted_v1(wrong_project)),
        Err(ReviewRepositoryError::InvalidData)
    );
    assert!(
        !project
            .reviews()
            .join("drafts/00000000-0000-0000-0000-000000000002.json")
            .exists()
    );

    let draft = review_draft(
        project.project_id,
        ReviewStreamId::from_u128(1),
        ReviewRoundId::from_u128(2),
    );
    repository.save_draft(&persisted_v1(draft.clone())).unwrap();
    assert_eq!(
        repository.load_draft(ReviewStreamId::from_u128(44), draft.review_round_id),
        Err(ReviewRepositoryError::InvalidData)
    );
}

#[test]
fn draft_filename_round_identity_is_verified_after_decoding() {
    let project = PersistentProject::new();
    let repository = open_writable(&project).unwrap();
    let stream_id = ReviewStreamId::from_u128(1);
    let requested_round_id = ReviewRoundId::from_u128(2);
    let stored = review_draft(project.project_id, stream_id, ReviewRoundId::from_u128(88));
    fs::write(
        project
            .reviews()
            .join(format!("drafts/{requested_round_id}.json")),
        encode_draft(&stored).unwrap(),
    )
    .unwrap();

    assert_eq!(
        repository.load_draft(stream_id, requested_round_id),
        Err(ReviewRepositoryError::InvalidData)
    );
}

#[test]
fn a_completed_round_id_can_never_be_reused_by_a_draft() {
    let project = PersistentProject::new();
    let repository = open_writable(&project).unwrap();
    let draft = review_draft(
        project.project_id,
        ReviewStreamId::from_u128(1),
        ReviewRoundId::from_u128(2),
    );
    fs::write(
        project
            .reviews()
            .join(format!("rounds/{}.json", draft.review_round_id)),
        b"already published",
    )
    .unwrap();

    assert_eq!(
        repository.save_draft(&persisted_v1(draft.clone())),
        Err(ReviewRepositoryError::Conflict)
    );
    assert!(
        !project
            .reviews()
            .join(format!("drafts/{}.json", draft.review_round_id))
            .exists()
    );
}

#[test]
fn unsupported_index_is_left_byte_for_byte_unchanged() {
    let project = PersistentProject::new();
    fs::create_dir(project.reviews()).unwrap();
    let index = br#"{"protocolVersion":"viewer.review/99","projectId":"00000000-0000-0000-0000-000000000001","streams":[]}"#;
    fs::write(project.reviews().join("index.json"), index).unwrap();

    assert!(matches!(
        open_writable(&project),
        Err(ReviewRepositoryError::UnsupportedVersion)
    ));
    assert_eq!(
        fs::read(project.reviews().join("index.json")).unwrap(),
        index
    );
    assert!(!project.reviews().join("drafts").exists());
    assert!(!project.reviews().join("rounds").exists());
}

#[test]
fn v2_index_is_supported_without_being_rewritten_on_open() {
    let project = PersistentProject::new();
    fs::create_dir(project.reviews()).unwrap();
    let index = br#"{"protocolVersion":"viewer.review/2","projectId":"00000000-0000-0000-0000-000000000001","streams":[]}"#;
    fs::write(project.reviews().join("index.json"), index).unwrap();

    let repository = open_writable(&project).unwrap();

    assert!(repository.load_catalog().unwrap().streams.is_empty());
    assert_eq!(
        fs::read(project.reviews().join("index.json")).unwrap(),
        index
    );
}

#[test]
fn index_project_identity_must_match_the_open_project() {
    let project = PersistentProject::new();
    fs::create_dir(project.reviews()).unwrap();
    let wrong_catalog = ReviewCatalog {
        project_id: ProjectId::from_u128(99),
        streams: vec![],
    };
    fs::write(
        project.reviews().join("index.json"),
        encode_catalog(&wrong_catalog).unwrap(),
    )
    .unwrap();

    assert_eq!(
        open_writable(&project).err(),
        Some(ReviewRepositoryError::InvalidData)
    );
    assert!(!project.reviews().join("drafts").exists());
    assert!(!project.reviews().join("rounds").exists());
}

#[cfg(unix)]
#[test]
fn owned_repository_paths_must_not_be_symlinks() {
    use std::os::unix::fs::symlink;

    for owned_path in [
        ".viewer",
        "reviews",
        "drafts",
        "rounds",
        "index.json",
        "write.lock",
    ] {
        let project = PersistentProject::new();
        let outside = tempfile::tempdir().unwrap();
        match owned_path {
            ".viewer" => {
                fs::remove_dir_all(project.root().join(".viewer")).unwrap();
                symlink(outside.path(), project.root().join(".viewer")).unwrap();
            }
            "reviews" => {
                symlink(outside.path(), project.reviews()).unwrap();
            }
            "drafts" | "rounds" => {
                fs::create_dir(project.reviews()).unwrap();
                symlink(outside.path(), project.reviews().join(owned_path)).unwrap();
            }
            "index.json" | "write.lock" => {
                fs::create_dir(project.reviews()).unwrap();
                let outside_file = outside.path().join("target");
                fs::write(&outside_file, b"outside").unwrap();
                symlink(outside_file, project.reviews().join(owned_path)).unwrap();
            }
            _ => unreachable!(),
        }

        assert_eq!(
            open_writable(&project).err(),
            Some(ReviewRepositoryError::InvalidData),
            "accepted symlinked {owned_path}"
        );
    }
}

#[test]
fn repository_paths_require_exact_case_without_duplicates() {
    let project = PersistentProject::new();
    fs::rename(
        project.root().join(".viewer"),
        project.root().join(".Viewer"),
    )
    .unwrap();

    assert_eq!(
        open_writable(&project).err(),
        Some(ReviewRepositoryError::InvalidData)
    );
}

#[test]
fn publishing_updates_only_the_target_stream_and_never_overwrites_a_round() {
    let project = PersistentProject::new();
    let repository = open_writable(&project).unwrap();
    let first = completed_round(project.project_id, 1, 11, None);
    let other = completed_round(project.project_id, 2, 21, None);

    repository.publish(&v1_publication(first.clone())).unwrap();
    repository.publish(&v1_publication(other.clone())).unwrap();

    let catalog = repository.load_catalog().unwrap();
    assert_eq!(
        stream(&catalog, 1).latest_completed_round_id,
        Some(first.review_round_id)
    );
    assert_eq!(
        stream(&catalog, 2).latest_completed_round_id,
        Some(other.review_round_id)
    );
    assert_eq!(
        repository.publish(&v1_publication(first.clone())),
        Err(ReviewRepositoryError::Conflict)
    );
    assert_eq!(
        repository
            .load_completed(first.review_stream_id, first.review_round_id)
            .unwrap(),
        Some(first)
    );
}

#[test]
fn publishing_rejects_a_stale_stream_head_before_creating_the_round() {
    let project = PersistentProject::new();
    let repository = open_writable(&project).unwrap();
    let first = completed_round(project.project_id, 1, 11, None);
    repository.publish(&v1_publication(first.clone())).unwrap();
    let stale = completed_round(project.project_id, 1, 12, None);

    assert_eq!(
        repository.publish(&v1_publication(stale.clone())),
        Err(ReviewRepositoryError::Conflict)
    );
    assert!(
        !project
            .reviews()
            .join(format!("rounds/{}.json", stale.review_round_id))
            .exists()
    );
    assert_eq!(
        stream(&repository.load_catalog().unwrap(), 1).latest_completed_round_id,
        Some(first.review_round_id)
    );
}

#[test]
fn failed_index_publication_keeps_the_old_head_and_recovers_one_linear_orphan() {
    for point in [
        ReviewRepositoryFaultPoint::AfterRoundDurableBeforeIndex,
        ReviewRepositoryFaultPoint::BeforeIndexReplace,
    ] {
        let project = PersistentProject::new();
        let repository = open_writable_with_faults(&project, fail_once(point)).unwrap();
        let draft = review_draft(
            project.project_id,
            ReviewStreamId::from_u128(1),
            ReviewRoundId::from_u128(11),
        );
        repository.save_draft(&persisted_v1(draft.clone())).unwrap();
        let round = draft.complete(2_000).unwrap();

        assert_eq!(
            repository.publish(&v1_publication(round.clone())),
            Err(ReviewRepositoryError::Unavailable)
        );
        assert!(repository.load_catalog().unwrap().streams.is_empty());
        assert_eq!(
            repository
                .load_completed(round.review_stream_id, round.review_round_id)
                .unwrap(),
            None
        );
        assert!(
            project
                .reviews()
                .join(format!("drafts/{}.json", round.review_round_id))
                .is_file()
        );
        drop(repository);

        let recovered = open_writable(&project).unwrap();
        assert_eq!(
            stream(&recovered.load_catalog().unwrap(), 1).latest_completed_round_id,
            Some(round.review_round_id)
        );
        assert_eq!(
            recovered
                .load_completed(round.review_stream_id, round.review_round_id)
                .unwrap(),
            Some(round)
        );
        assert!(
            !project
                .reviews()
                .join("drafts/00000000-0000-0000-0000-00000000000b.json")
                .exists()
        );
    }
}

#[test]
fn orphan_forks_require_recovery_without_changing_the_index() {
    let project = PersistentProject::new();
    drop(open_writable(&project).unwrap());
    let original_index = fs::read(project.reviews().join("index.json")).unwrap();
    for round in [11, 12] {
        let snapshot = completed_round(project.project_id, 1, round, None);
        fs::write(
            project
                .reviews()
                .join(format!("rounds/{}.json", snapshot.review_round_id)),
            encode_completed(&snapshot).unwrap(),
        )
        .unwrap();
    }

    assert_eq!(
        open_writable(&project).err(),
        Some(ReviewRepositoryError::RecoveryRequired)
    );
    assert_eq!(
        fs::read(project.reviews().join("index.json")).unwrap(),
        original_index
    );
}

#[test]
fn a_unique_multi_round_orphan_chain_recovers_in_causal_order() {
    let project = PersistentProject::new();
    drop(open_writable(&project).unwrap());
    let first = completed_round(project.project_id, 1, 11, None);
    let second = completed_round(project.project_id, 1, 12, Some(first.review_round_id));
    for snapshot in [&second, &first] {
        fs::write(
            project
                .reviews()
                .join(format!("rounds/{}.json", snapshot.review_round_id)),
            encode_completed(snapshot).unwrap(),
        )
        .unwrap();
    }

    let repository = open_writable(&project).unwrap();
    let recovered_stream = stream(&repository.load_catalog().unwrap(), 1).clone();
    assert_eq!(
        recovered_stream.completed_round_ids().collect::<Vec<_>>(),
        vec![first.review_round_id, second.review_round_id]
    );
    assert_eq!(
        recovered_stream.latest_completed_round_id,
        Some(second.review_round_id)
    );
}

#[test]
fn an_unconnectable_or_wrong_project_orphan_never_changes_the_index() {
    for snapshot in [
        completed_round(
            ProjectId::from_u128(1),
            1,
            11,
            Some(ReviewRoundId::from_u128(999)),
        ),
        completed_round(ProjectId::from_u128(99), 1, 11, None),
    ] {
        let project = PersistentProject::new();
        drop(open_writable(&project).unwrap());
        let original_index = fs::read(project.reviews().join("index.json")).unwrap();
        fs::write(
            project
                .reviews()
                .join(format!("rounds/{}.json", snapshot.review_round_id)),
            encode_completed(&snapshot).unwrap(),
        )
        .unwrap();

        assert_eq!(
            open_writable(&project).err(),
            Some(ReviewRepositoryError::RecoveryRequired)
        );
        assert_eq!(
            fs::read(project.reviews().join("index.json")).unwrap(),
            original_index
        );
    }
}

#[test]
fn readonly_open_detects_an_orphan_without_mutating_it() {
    let project = PersistentProject::new();
    drop(open_writable(&project).unwrap());
    let snapshot = completed_round(project.project_id, 1, 11, None);
    let round_path = project
        .reviews()
        .join(format!("rounds/{}.json", snapshot.review_round_id));
    fs::write(&round_path, encode_completed(&snapshot).unwrap()).unwrap();
    let index_before = fs::read(project.reviews().join("index.json")).unwrap();
    let round_before = fs::read(&round_path).unwrap();

    assert_eq!(
        ProjectReviewRepository::open(
            project.root(),
            project.project_id,
            ReviewRepositoryAccess::ReadOnly,
        )
        .err(),
        Some(ReviewRepositoryError::RecoveryRequired)
    );
    assert_eq!(
        fs::read(project.reviews().join("index.json")).unwrap(),
        index_before
    );
    assert_eq!(fs::read(round_path).unwrap(), round_before);
}

#[test]
fn orphan_filename_and_payload_round_ids_must_match() {
    let project = PersistentProject::new();
    drop(open_writable(&project).unwrap());
    let original_index = fs::read(project.reviews().join("index.json")).unwrap();
    let snapshot = completed_round(project.project_id, 1, 11, None);
    fs::write(
        project
            .reviews()
            .join("rounds/00000000-0000-0000-0000-000000000099.json"),
        encode_completed(&snapshot).unwrap(),
    )
    .unwrap();

    assert_eq!(
        open_writable(&project).err(),
        Some(ReviewRepositoryError::RecoveryRequired)
    );
    assert_eq!(
        fs::read(project.reviews().join("index.json")).unwrap(),
        original_index
    );
}

#[cfg(unix)]
#[test]
fn published_history_ignores_a_stale_draft_when_cleanup_cannot_finish() {
    use std::os::unix::fs::PermissionsExt;

    let project = PersistentProject::new();
    let repository = open_writable(&project).unwrap();
    let draft = review_draft(
        project.project_id,
        ReviewStreamId::from_u128(1),
        ReviewRoundId::from_u128(11),
    );
    repository.save_draft(&persisted_v1(draft.clone())).unwrap();
    let snapshot = draft.complete(2_000).unwrap();
    let drafts_directory = project.reviews().join("drafts");
    fs::set_permissions(&drafts_directory, fs::Permissions::from_mode(0o555)).unwrap();

    repository
        .publish(&v1_publication(snapshot.clone()))
        .unwrap();
    let draft_path = drafts_directory.join(format!("{}.json", snapshot.review_round_id));
    assert!(draft_path.exists());
    assert_eq!(
        repository
            .load_draft(snapshot.review_stream_id, snapshot.review_round_id)
            .unwrap(),
        None
    );
    drop(repository);
    let reopened = open_writable(&project).unwrap();
    assert_eq!(
        reopened
            .load_draft(snapshot.review_stream_id, snapshot.review_round_id)
            .unwrap(),
        None
    );
    drop(reopened);

    fs::set_permissions(&drafts_directory, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn concurrent_publication_on_one_repository_serializes_stream_compare_and_set() {
    let project = PersistentProject::new();
    let repository = Arc::new(open_writable(&project).unwrap());
    let first = completed_round(project.project_id, 1, 11, None);
    let competing = completed_round(project.project_id, 1, 12, None);
    let left = {
        let repository = Arc::clone(&repository);
        std::thread::spawn(move || repository.publish(&v1_publication(first)))
    };
    let right = {
        let repository = Arc::clone(&repository);
        std::thread::spawn(move || repository.publish(&v1_publication(competing)))
    };

    let results = [left.join().unwrap(), right.join().unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| **result == Err(ReviewRepositoryError::Conflict))
            .count(),
        1
    );
    assert_eq!(
        repository.load_catalog().unwrap().streams[0]
            .completed_round_ids()
            .len(),
        1
    );
}

#[test]
fn project_repository_implements_the_complete_application_port() {
    let project = PersistentProject::new();
    let repository = open_writable(&project).unwrap();
    let port: &dyn ReviewRepositoryPort = &repository;

    assert_eq!(port.load_catalog().unwrap().project_id, project.project_id);
}

#[test]
fn provider_inspection_is_side_effect_free_for_absent_and_empty_repositories() {
    let project = PersistentProject::new();
    let provider = ProjectReviewRepositoryProvider::new(project.root(), project.project_id);

    let absent = provider.inspect().unwrap();
    assert_eq!(absent.catalog.project_id, project.project_id);
    assert!(absent.catalog.streams.is_empty());
    assert_eq!(absent.active_draft, None);
    let reader = provider.open_reader().unwrap();
    assert!(reader.load_catalog().unwrap().streams.is_empty());
    assert_eq!(reader.load_active_draft().unwrap(), None);
    assert!(!project.reviews().exists());

    drop(provider.open_writer().unwrap());
    let empty = provider.inspect().unwrap();
    assert!(empty.catalog.streams.is_empty());
    assert_eq!(empty.active_draft, None);
}

#[test]
fn provider_preserves_unsupported_draft_versions_without_mutation() {
    let project = PersistentProject::new();
    let provider = ProjectReviewRepositoryProvider::new(project.root(), project.project_id);
    drop(provider.open_writer().unwrap());
    let path = project
        .reviews()
        .join("drafts/00000000-0000-0000-0000-00000000000b.json");
    let unsupported = br#"{"protocolVersion":"viewer.review/99","status":"draft"}"#;
    fs::write(&path, unsupported).unwrap();

    assert_eq!(
        provider.inspect(),
        Err(ReviewRepositoryError::UnsupportedVersion)
    );
    assert_eq!(fs::read(path).unwrap(), unsupported);
}

#[test]
fn repository_preserves_v2_draft_version_across_save_and_discovery() {
    let project = PersistentProject::new();
    let provider = ProjectReviewRepositoryProvider::new(project.root(), project.project_id);
    let draft = review_draft(
        project.project_id,
        ReviewStreamId::from_u128(1),
        ReviewRoundId::from_u128(11),
    );
    let persisted = PersistedReviewDraft {
        protocol_version: ReviewProtocolVersion::V2,
        draft,
    };
    let writer = provider.open_writer().unwrap();

    writer.save_draft(&persisted).unwrap();
    drop(writer);

    let path = project
        .reviews()
        .join("drafts/00000000-0000-0000-0000-00000000000b.json");
    assert_eq!(
        fs::read(&path).unwrap(),
        encode_draft_v2(&persisted.draft).unwrap()
    );
    assert_eq!(provider.inspect().unwrap().active_draft, Some(persisted));
}

#[test]
fn provider_discovers_one_exact_draft_and_refuses_ambiguous_or_invalid_entries() {
    let project = PersistentProject::new();
    let provider = ProjectReviewRepositoryProvider::new(project.root(), project.project_id);
    let writer = provider.open_writer().unwrap();
    let first = review_draft(
        project.project_id,
        ReviewStreamId::from_u128(1),
        ReviewRoundId::from_u128(11),
    );
    writer.save_draft(&persisted_v1(first.clone())).unwrap();
    drop(writer);

    assert_eq!(
        provider.inspect().unwrap().active_draft,
        Some(persisted_v1(first))
    );

    let second = review_draft(
        project.project_id,
        ReviewStreamId::from_u128(1),
        ReviewRoundId::from_u128(12),
    );
    provider
        .open_writer()
        .unwrap()
        .save_draft(&persisted_v1(second.clone()))
        .unwrap();
    assert_eq!(
        provider.inspect(),
        Err(ReviewRepositoryError::RecoveryRequired)
    );

    fs::remove_file(
        project
            .reviews()
            .join(format!("drafts/{}.json", second.review_round_id)),
    )
    .unwrap();
    fs::write(project.reviews().join("drafts/not-a-round.json"), b"{}").unwrap();
    assert_eq!(
        provider.inspect(),
        Err(ReviewRepositoryError::RecoveryRequired)
    );
}

#[test]
fn provider_refuses_draft_payloads_owned_by_another_project() {
    let project = PersistentProject::new();
    let provider = ProjectReviewRepositoryProvider::new(project.root(), project.project_id);
    drop(provider.open_writer().unwrap());
    let foreign = review_draft(
        ProjectId::from_u128(99),
        ReviewStreamId::from_u128(1),
        ReviewRoundId::from_u128(11),
    );
    fs::write(
        project
            .reviews()
            .join(format!("drafts/{}.json", foreign.review_round_id)),
        encode_draft(&foreign).unwrap(),
    )
    .unwrap();

    assert_eq!(
        provider.inspect(),
        Err(ReviewRepositoryError::RecoveryRequired)
    );
}

#[cfg(unix)]
#[test]
fn provider_refuses_a_symlinked_draft_without_following_it() {
    use std::os::unix::fs::symlink;

    let project = PersistentProject::new();
    let provider = ProjectReviewRepositoryProvider::new(project.root(), project.project_id);
    drop(provider.open_writer().unwrap());
    let outside = tempfile::NamedTempFile::new().unwrap();
    symlink(
        outside.path(),
        project
            .reviews()
            .join("drafts/00000000-0000-0000-0000-00000000000b.json"),
    )
    .unwrap();

    assert_eq!(
        provider.inspect(),
        Err(ReviewRepositoryError::RecoveryRequired)
    );
}

#[test]
fn provider_writer_lease_is_held_by_the_returned_port_until_drop() {
    let project = PersistentProject::new();
    let provider = ProjectReviewRepositoryProvider::new(project.root(), project.project_id);
    let first = provider.open_writer().unwrap();

    assert!(matches!(
        provider.open_writer(),
        Err(ReviewRepositoryError::Busy)
    ));
    drop(first);
    assert!(provider.open_writer().is_ok());
}

#[test]
fn delete_draft_checks_exact_ownership_and_never_touches_completed_history() {
    let project = PersistentProject::new();
    let provider = ProjectReviewRepositoryProvider::new(project.root(), project.project_id);
    let writer = provider.open_writer().unwrap();
    let draft = review_draft(
        project.project_id,
        ReviewStreamId::from_u128(1),
        ReviewRoundId::from_u128(11),
    );
    writer.save_draft(&persisted_v1(draft.clone())).unwrap();

    assert_eq!(
        writer.delete_draft(ReviewStreamId::from_u128(99), draft.review_round_id),
        Err(ReviewRepositoryError::InvalidData)
    );
    assert_eq!(
        writer
            .load_draft(draft.review_stream_id, draft.review_round_id)
            .unwrap(),
        Some(persisted_v1(draft.clone()))
    );

    writer
        .delete_draft(draft.review_stream_id, draft.review_round_id)
        .unwrap();
    assert_eq!(
        writer.delete_draft(draft.review_stream_id, draft.review_round_id),
        Err(ReviewRepositoryError::NotFound)
    );

    let completed = completed_round(project.project_id, 1, 21, None);
    writer.publish(&v1_publication(completed.clone())).unwrap();
    assert_eq!(
        writer.delete_draft(completed.review_stream_id, completed.review_round_id),
        Err(ReviewRepositoryError::NotFound)
    );
    assert_eq!(
        writer
            .load_completed(completed.review_stream_id, completed.review_round_id)
            .unwrap(),
        Some(completed)
    );
}

#[cfg(unix)]
#[test]
fn failed_draft_deletion_keeps_the_draft_readable() {
    use std::os::unix::fs::PermissionsExt;

    let project = PersistentProject::new();
    let provider = ProjectReviewRepositoryProvider::new(project.root(), project.project_id);
    let writer = provider.open_writer().unwrap();
    let draft = review_draft(
        project.project_id,
        ReviewStreamId::from_u128(1),
        ReviewRoundId::from_u128(11),
    );
    writer.save_draft(&persisted_v1(draft.clone())).unwrap();
    let drafts_directory = project.reviews().join("drafts");
    fs::set_permissions(&drafts_directory, fs::Permissions::from_mode(0o555)).unwrap();

    let delete_result = writer.delete_draft(draft.review_stream_id, draft.review_round_id);
    let readable_result = writer.load_draft(draft.review_stream_id, draft.review_round_id);
    fs::set_permissions(&drafts_directory, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(delete_result, Err(ReviewRepositoryError::Unavailable));
    assert_eq!(readable_result.unwrap(), Some(persisted_v1(draft)));
}
