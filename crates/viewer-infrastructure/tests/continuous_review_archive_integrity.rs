use std::fs;
use viewer_application::review_workspace::*;
use viewer_domain::{ReviewSnapshotId, ReviewStreamId, review::continuous::*};
use viewer_infrastructure::review::v3;
#[path = "support/continuous_review.rs"]
mod support;
use support::*;

#[test]
fn historical_archive_rejects_a_basis_that_was_created_after_its_before_snapshot() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let stream_id = ReviewStreamId::from_u128(2);
    writer.commit(feedback_request()).unwrap();
    let basis = writer.load_current(stream_id).unwrap().unwrap();
    let archive = archive_request(&basis, &basis, 4);
    let checkpoint = archive.archives[0].clone();
    writer.commit(archive).unwrap();
    let current = writer.load_current(stream_id).unwrap().unwrap();
    let selected = key(&basis.state.feedback[0], 0);
    let (state, _) = apply_restore(
        &current.state,
        &checkpoint,
        std::slice::from_ref(&basis.state),
        &[RestoreDecision {
            historical_key: selected,
            choice: RestoreChoice::UseHistorical,
        }],
        ReviewSnapshotId::from_u128(5),
    )
    .unwrap();
    let mut restore = request(5, Some(current.reference));
    restore.next.state = state;
    restore.next.evidence = current.evidence;
    restore.next.changes = vec![ReviewChange {
        target_id: selected.target_id,
        before: None,
        after: Some(selected),
        kind: ReviewChangeKind::Restored,
        archive_id: Some(checkpoint.archive_id),
        historical_key: Some(selected),
    }];
    let restored = writer.commit(restore).unwrap();
    assert_eq!(
        writer
            .load_archive(stream_id, checkpoint.archive_id)
            .unwrap(),
        checkpoint.clone()
    );

    let directory = root.path().join(".viewer/reviews");
    let archive_path = directory.join(format!("archives/{}.json", checkpoint.archive_id));
    let mut record = v3::decode_archive_v3(&fs::read(&archive_path).unwrap()).unwrap();
    // The same exact target exists again in the descendant after restore. Matching
    // target revisions and digests alone cannot authorize this impossible B > C.
    record.checkpoint.groups[0].basis = ArchiveBasis::Known {
        snapshot: restored.snapshot,
        source: ArchiveBasisSource::UserSelected,
    };
    let forged_archive = v3::encode_archive_v3(&record).unwrap();
    fs::write(&archive_path, &forged_archive).unwrap();
    let index_path = directory.join("index.json");
    let mut index = v3::decode_index_v3(&fs::read(&index_path).unwrap()).unwrap();
    index.streams[0].archive_refs[0].blake3 = *blake3::hash(&forged_archive).as_bytes();
    let index_bytes = v3::encode_index_v3(&index).unwrap();
    fs::write(&index_path, &index_bytes).unwrap();
    assert_eq!(
        writer.load_archive(stream_id, checkpoint.archive_id),
        Err(ReviewCommitError::Integrity)
    );
    assert_eq!(
        writer.load_current_ref(stream_id).unwrap(),
        Some(restored.snapshot)
    );
    assert_eq!(fs::read(index_path).unwrap(), index_bytes);
    assert_eq!(fs::read(archive_path).unwrap(), forged_archive);
}
