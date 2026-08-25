use std::{fs, path::Path};
use tempfile::TempDir;
use viewer_application::{ReviewCatalog, ReviewRepositoryError};
use viewer_domain::review::{AssetEvidence, AssetVersion, ReviewDraft, ReviewMedia};
use viewer_domain::{AssetVersionId, ProjectId, RelativePath, ReviewRoundId, ReviewStreamId};
use viewer_infrastructure::review::{
    ProjectReviewRepository, ReviewRepositoryAccess, encode_catalog, encode_draft,
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
            relative_path: RelativePath::parse("renders/frame.png").unwrap(),
            evidence: AssetEvidence {
                size_bytes: 4,
                modified_ns: 123,
                blake3: None,
            },
            media: ReviewMedia::Image {
                width: 10,
                height: 20,
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
