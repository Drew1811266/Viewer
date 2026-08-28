#[path = "support/continuous_review.rs"]
mod support;
use std::{fs, sync::Arc};
use support::*;
use viewer_application::review_workspace::*;
use viewer_domain::*;
use viewer_infrastructure::review::{ProjectUsageImporter, v3};

fn usage(basis: &StoredContinuousSnapshot) -> v3::ReviewUsageRecord {
    v3::ReviewUsageRecord {
        declaration_id: ReviewUsageId::from_u128(90),
        project_id: ProjectId::from_u128(1),
        review_stream_id: ReviewStreamId::from_u128(2),
        basis: basis.reference,
        targets: vec![key(&basis.state.feedback[0], 0)],
        outputs: vec![],
    }
}
fn write(root: &std::path::Path, record: &v3::ReviewUsageRecord) -> RelativePath {
    fs::create_dir_all(root.join("handoff")).unwrap();
    let source = RelativePath::parse("handoff/usage.json").unwrap();
    fs::write(
        root.join(source.as_str()),
        v3::encode_usage_v1(record).unwrap(),
    )
    .unwrap();
    source
}
fn importer(
    root: &std::path::Path,
    repository: Arc<dyn ContinuousReviewRepositoryPort>,
) -> ProjectUsageImporter {
    ProjectUsageImporter::new(
        root,
        ProjectId::from_u128(1),
        ReviewStreamId::from_u128(2),
        repository,
    )
    .unwrap()
}

#[test]
fn inspection_is_read_only_and_valid_basis_does_not_claim_execution() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    writer.commit(feedback_request()).unwrap();
    let basis = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let declaration = usage(&basis);
    let source = write(root.path(), &declaration);
    let import = importer(root.path(), provider.continuous_reader().unwrap());
    let preview = import.inspect(&source).unwrap();
    assert_eq!(
        preview.declaration.targets,
        vec![key(&basis.state.feedback[0], 0)]
    );
    assert_eq!(preview.declaration.basis, basis.reference);
    assert!(preview.outputs.is_empty());
    assert!(!root.path().join(".viewer/reviews/usage").exists());
    assert_eq!(
        writer
            .load_current_ref(ReviewStreamId::from_u128(2))
            .unwrap(),
        Some(basis.reference)
    );
}

#[test]
fn wrong_context_forged_basis_and_target_versions_are_rejected() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    writer.commit(feedback_request()).unwrap();
    let basis = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let import = importer(root.path(), provider.continuous_reader().unwrap());
    for field in 0..5 {
        let mut r = usage(&basis);
        let error = match field {
            0 => {
                r.project_id = ProjectId::new();
                UsageImportError::WrongContext
            }
            1 => {
                r.review_stream_id = ReviewStreamId::new();
                UsageImportError::WrongContext
            }
            2 => {
                r.basis.blake3 = [0; 32];
                UsageImportError::UnknownBasis
            }
            3 => {
                r.targets[0].text_revision_id = ReviewTextRevisionId::new();
                UsageImportError::InvalidScope
            }
            _ => {
                r.targets[0].target_id = ReviewTargetId::new();
                UsageImportError::InvalidScope
            }
        };
        let path = write(root.path(), &r);
        assert_eq!(import.inspect(&path), Err(error));
    }
}

#[test]
fn outputs_are_separate_candidates_and_bad_mapping_does_not_invalidate_basis() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    writer.commit(feedback_request()).unwrap();
    let basis = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    fs::write(root.path().join("new.mp4"), b"producer output").unwrap();
    let mut r = usage(&basis);
    r.outputs.push(v3::UsageOutputRecord {
        relative_path: RelativePath::parse("new.mp4").unwrap(),
        blake3: *blake3::hash(b"producer output").as_bytes(),
        previous_asset_version_id: basis.state.assets[0].id,
    });
    let import = importer(root.path(), provider.continuous_reader().unwrap());
    let source = write(root.path(), &r);
    assert_eq!(
        import.inspect(&source).unwrap().outputs[0].status,
        UsageOutputStatus::VerifiedCandidate
    );
    r.outputs[0].blake3 = [0; 32];
    write(root.path(), &r);
    assert_eq!(
        import.inspect(&source).unwrap().outputs[0].status,
        UsageOutputStatus::Changed
    );
    r.outputs[0].previous_asset_version_id = AssetVersionId::new();
    write(root.path(), &r);
    let preview = import.inspect(&source).unwrap();
    assert_eq!(
        preview.outputs[0].status,
        UsageOutputStatus::UnknownPreviousAsset
    );
    assert_eq!(preview.declaration.targets, r.targets);
}

#[test]
fn unsafe_declaration_sources_and_output_symlinks_fail_without_following_them() {
    use std::os::unix::fs::symlink;
    let (root, provider) = setup();
    let outside = tempfile::tempdir().unwrap();
    let writer = provider.continuous_writer().unwrap();
    writer.commit(feedback_request()).unwrap();
    let basis = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let mut r = usage(&basis);
    let source = write(outside.path(), &r);
    symlink(outside.path().join("handoff"), root.path().join("handoff")).unwrap();
    let import = importer(root.path(), provider.continuous_reader().unwrap());
    assert_eq!(import.inspect(&source), Err(UsageImportError::UnsafePath));
    fs::write(outside.path().join("secret"), b"outside").unwrap();
    symlink(
        outside.path().join("secret"),
        root.path().join("output.mp4"),
    )
    .unwrap();
    r.outputs.push(v3::UsageOutputRecord {
        relative_path: RelativePath::parse("output.mp4").unwrap(),
        blake3: *blake3::hash(b"outside").as_bytes(),
        previous_asset_version_id: basis.state.assets[0].id,
    });
    fs::write(
        root.path().join("safe.json"),
        v3::encode_usage_v1(&r).unwrap(),
    )
    .unwrap();
    assert_eq!(
        import
            .inspect(&RelativePath::parse("safe.json").unwrap())
            .unwrap()
            .outputs[0]
            .status,
        UsageOutputStatus::Unsafe
    );
    assert_eq!(fs::read(outside.path().join("secret")).unwrap(), b"outside");
}

#[test]
fn adopted_identity_conflict_is_detected_but_identical_canonical_content_is_deduplicated() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    writer.commit(feedback_request()).unwrap();
    let basis = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let r = usage(&basis);
    let source = write(root.path(), &r);
    let import = importer(root.path(), provider.continuous_reader().unwrap());
    let preview = import.inspect(&source).unwrap();
    let mut adopt = request(4, Some(basis.reference));
    adopt.next.state = basis.state.clone();
    adopt.next.state.snapshot_id = ReviewSnapshotId::from_u128(4);
    adopt.next.evidence = basis.evidence.clone();
    adopt.adopted_usage = vec![preview.declaration];
    writer.commit(adopt).unwrap();
    assert!(import.inspect(&source).is_ok());
    let mut conflict = r;
    conflict.targets = vec![key(&basis.state.feedback[0], 1)];
    write(root.path(), &conflict);
    assert_eq!(import.inspect(&source), Err(UsageImportError::Conflict));
}

#[test]
fn declaration_size_limit_is_checked_before_reading_a_sparse_oversize_file() {
    let (root, provider) = setup();
    let import = importer(root.path(), provider.continuous_reader().unwrap());
    fs::File::create(root.path().join("large.json"))
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    assert_eq!(
        import.inspect(&RelativePath::parse("large.json").unwrap()),
        Err(UsageImportError::LimitExceeded)
    );
}

#[test]
fn adopting_a_valid_basis_with_bad_output_keeps_raw_declaration_without_granting_lineage() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    writer.commit(feedback_request()).unwrap();
    let basis = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let mut r = usage(&basis);
    r.outputs.push(v3::UsageOutputRecord {
        relative_path: RelativePath::parse("unknown.mp4").unwrap(),
        blake3: [7; 32],
        previous_asset_version_id: AssetVersionId::new(),
    });
    let source = write(root.path(), &r);
    let import = importer(root.path(), provider.continuous_reader().unwrap());
    let preview = import.inspect(&source).unwrap();
    assert_eq!(
        preview.outputs[0].status,
        UsageOutputStatus::UnknownPreviousAsset
    );
    let mut adopt = request(4, Some(basis.reference));
    adopt.next.state = basis.state.clone();
    adopt.next.state.snapshot_id = ReviewSnapshotId::from_u128(4);
    adopt.next.evidence = basis.evidence.clone();
    adopt.adopted_usage = vec![preview.declaration.clone()];
    writer.commit(adopt).unwrap();
    assert_eq!(
        writer
            .load_usage(ReviewStreamId::from_u128(2), preview.declaration.id)
            .unwrap(),
        Some(preview.declaration)
    );
    assert_eq!(
        writer
            .load_current(ReviewStreamId::from_u128(2))
            .unwrap()
            .unwrap()
            .state
            .feedback,
        basis.state.feedback
    );
}

#[test]
fn exact_json_byte_limit_accepts_whitespace_but_unknown_fields_and_traversal_do_not() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    writer.commit(feedback_request()).unwrap();
    let basis = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let r = usage(&basis);
    let source = write(root.path(), &r);
    let import = importer(root.path(), provider.continuous_reader().unwrap());
    let mut bytes = v3::encode_usage_v1(&r).unwrap();
    bytes.resize(64 * 1024 * 1024, b' ');
    fs::write(root.path().join(source.as_str()), bytes).unwrap();
    assert!(import.inspect(&source).is_ok());
    let mut invalid: serde_json::Value =
        serde_json::from_slice(&v3::encode_usage_v1(&r).unwrap()).unwrap();
    invalid["script"] = serde_json::json!("do not execute");
    fs::write(
        root.path().join(source.as_str()),
        serde_json::to_vec(&invalid).unwrap(),
    )
    .unwrap();
    assert_eq!(
        import.inspect(&source),
        Err(UsageImportError::InvalidDeclaration)
    );
    invalid.as_object_mut().unwrap().remove("script");
    invalid["outputs"] = serde_json::json!([{"relativePath":"../outside.mp4","blake3":"00".repeat(32),"previousAssetVersionId":basis.state.assets[0].id}]);
    fs::write(
        root.path().join(source.as_str()),
        serde_json::to_vec(&invalid).unwrap(),
    )
    .unwrap();
    assert_eq!(
        import.inspect(&source),
        Err(UsageImportError::InvalidDeclaration)
    );
}
