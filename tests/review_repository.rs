use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{fs, path::Path};
use tempfile::TempDir;
use viewer_application::{
    ProjectAccess, ReviewCatalog, ReviewRepositoryError, ReviewRepositoryPort,
    ReviewRepositoryProviderPort, ReviewStreamHead,
};
use viewer_domain::review::{
    AssetEvidence, AssetVersion, ReviewDraft, ReviewMedia, ReviewSnapshot,
};
use viewer_domain::{AssetVersionId, ProjectId, RelativePath, ReviewRoundId, ReviewStreamId};
use viewer_infrastructure::review::{
    ProjectReviewRepository, ProjectReviewRepositoryProvider, ReviewRepositoryAccess,
    ReviewRepositoryFaultInjector, ReviewRepositoryFaultPoint, encode_catalog, encode_completed,
    encode_draft,
};

struct PersistentProject {
    directory: TempDir,
    project_id: ProjectId,
}

impl PersistentProject {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let viewer = directory.path().join(".viewer");
        fs::create_dir(&viewer).unwrap();
        fs::write(viewer.join("project.json"), b"manifest sentinel").unwrap();
        fs::write(viewer.join("metadata.sqlite"), b"database sentinel").unwrap();
        Self {
            directory,
            project_id: ProjectId::from_u128(1),
        }
    }

    fn root(&self) -> &Path {
        self.directory.path()
    }

    fn reviews(&self) -> std::path::PathBuf {
        self.root().join(".viewer/reviews")
    }
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

    repository.save_draft(&draft).unwrap();

    assert_eq!(
        repository
            .load_draft(draft.review_stream_id, draft.review_round_id)
            .unwrap(),
        Some(draft)
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
        repository.save_draft(&draft),
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
        repository.save_draft(&wrong_project),
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
    repository.save_draft(&draft).unwrap();
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
        repository.save_draft(&draft),
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
    let index = br#"{"protocolVersion":"viewer.review/2","projectId":"00000000-0000-0000-0000-000000000001","streams":[]}"#;
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

    repository.publish(&first).unwrap();
    repository.publish(&other).unwrap();

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
        repository.publish(&first),
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
    repository.publish(&first).unwrap();
    let stale = completed_round(project.project_id, 1, 12, None);

    assert_eq!(
        repository.publish(&stale),
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
        repository.save_draft(&draft).unwrap();
        let round = draft.complete(2_000).unwrap();

        assert_eq!(
            repository.publish(&round),
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
        recovered_stream.completed_round_ids,
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
    repository.save_draft(&draft).unwrap();
    let snapshot = draft.complete(2_000).unwrap();
    let drafts_directory = project.reviews().join("drafts");
    fs::set_permissions(&drafts_directory, fs::Permissions::from_mode(0o555)).unwrap();

    repository.publish(&snapshot).unwrap();
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
        std::thread::spawn(move || repository.publish(&first))
    };
    let right = {
        let repository = Arc::clone(&repository);
        std::thread::spawn(move || repository.publish(&competing))
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
            .completed_round_ids
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
    let unsupported = br#"{"protocolVersion":"viewer.review/2","status":"draft"}"#;
    fs::write(&path, unsupported).unwrap();

    assert_eq!(
        provider.inspect(),
        Err(ReviewRepositoryError::UnsupportedVersion)
    );
    assert_eq!(fs::read(path).unwrap(), unsupported);
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
    writer.save_draft(&first).unwrap();
    drop(writer);

    assert_eq!(provider.inspect().unwrap().active_draft, Some(first));

    let second = review_draft(
        project.project_id,
        ReviewStreamId::from_u128(1),
        ReviewRoundId::from_u128(12),
    );
    provider.open_writer().unwrap().save_draft(&second).unwrap();
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
    writer.save_draft(&draft).unwrap();

    assert_eq!(
        writer.delete_draft(ReviewStreamId::from_u128(99), draft.review_round_id),
        Err(ReviewRepositoryError::InvalidData)
    );
    assert_eq!(
        writer
            .load_draft(draft.review_stream_id, draft.review_round_id)
            .unwrap(),
        Some(draft.clone())
    );

    writer
        .delete_draft(draft.review_stream_id, draft.review_round_id)
        .unwrap();
    assert_eq!(
        writer.delete_draft(draft.review_stream_id, draft.review_round_id),
        Err(ReviewRepositoryError::NotFound)
    );

    let completed = completed_round(project.project_id, 1, 21, None);
    writer.publish(&completed).unwrap();
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
    writer.save_draft(&draft).unwrap();
    let drafts_directory = project.reviews().join("drafts");
    fs::set_permissions(&drafts_directory, fs::Permissions::from_mode(0o555)).unwrap();

    let delete_result = writer.delete_draft(draft.review_stream_id, draft.review_round_id);
    let readable_result = writer.load_draft(draft.review_stream_id, draft.review_round_id);
    fs::set_permissions(&drafts_directory, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(delete_result, Err(ReviewRepositoryError::Unavailable));
    assert_eq!(readable_result.unwrap(), Some(draft));
}
