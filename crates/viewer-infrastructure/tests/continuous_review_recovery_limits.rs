use std::fs;
use viewer_application::review_workspace::*;
use viewer_domain::{ReviewCommandId, ReviewStreamId};
#[path = "support/continuous_review.rs"]
mod support;
use support::*;

fn draft(sequence: u128) -> RecoveryDraft {
    RecoveryDraft {
        stream_id: ReviewStreamId::from_u128(2),
        command_id: ReviewCommandId::from_u128(sequence),
        expected_snapshot_id: None,
        payload_digest: [3; 32],
        editor_input: RecoveryEditorInput {
            migration: None,
            selections: vec![],
            text: "保留这份未提交输入。".into(),
            feedback_id: None,
            targets: vec![],
            history_ref: None,
        },
        failure: ReviewRecoveryFailure::WriteFailed,
    }
}

#[test]
fn recovery_count_limit_rejects_new_input_but_allows_replacing_an_existing_record() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    writer.save_recovery(&draft(1)).unwrap();
    let directory = root.path().join(".viewer/reviews/recovery");
    let mut template: serde_json::Value = serde_json::from_slice(
        &fs::read(directory.join(format!("{}.json", draft(1).command_id))).unwrap(),
    )
    .unwrap();
    // Seed valid on-disk drafts without performing 10,000 durability barriers.
    for sequence in 2..=10_000 {
        let id = ReviewCommandId::from_u128(sequence);
        template["commandId"] = serde_json::json!(id.to_string());
        fs::write(
            directory.join(format!("{id}.json")),
            serde_json::to_vec(&template).unwrap(),
        )
        .unwrap();
    }
    // A staged file from an interrupted save is not another recovery draft.
    fs::write(
        directory.join(".viewer-review-1-0000000000000001.tmp"),
        b"staged",
    )
    .unwrap();
    assert_eq!(writer.load_recovery().unwrap().len(), 10_000);
    let rejected = draft(10_001);
    assert_eq!(
        writer.save_recovery(&rejected),
        Err(ReviewCommitError::LimitExceeded)
    );
    assert!(
        !directory
            .join(format!("{}.json", rejected.command_id))
            .exists()
    );
    let mut replacement = draft(1);
    replacement.failure = ReviewRecoveryFailure::Cancelled;
    writer.save_recovery(&replacement).unwrap();
    let saved = writer.load_recovery().unwrap();
    assert_eq!(saved.len(), 10_000);
    assert_eq!(saved[0], replacement);
}

#[test]
fn recovery_byte_limit_accounts_for_replacements_before_publishing() {
    const LIMIT: usize = 64 * 1024 * 1024;
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    writer.save_recovery(&draft(1)).unwrap();
    let directory = root.path().join(".viewer/reviews/recovery");
    let first_path = directory.join(format!("{}.json", draft(1).command_id));
    let mut bytes = fs::read(&first_path).unwrap();
    let record_length = bytes.len();
    // JSON trailing whitespace is valid; this exercises the real 64 MiB budget
    // without inventing a relaxed test-only limit or invalid oversized input.
    bytes.resize(LIMIT - record_length, b' ');
    fs::write(&first_path, &bytes).unwrap();
    assert_eq!(writer.load_recovery().unwrap(), vec![draft(1)]);
    writer.save_recovery(&draft(2)).unwrap();
    let second_path = directory.join(format!("{}.json", draft(2).command_id));
    let second_bytes = fs::read(&second_path).unwrap();
    assert_eq!(bytes.len() + second_bytes.len(), LIMIT);
    let mut larger = draft(2);
    larger.failure = ReviewRecoveryFailure::CommitUnknown;
    assert_eq!(
        writer.save_recovery(&larger),
        Err(ReviewCommitError::LimitExceeded)
    );
    assert_eq!(fs::read(&second_path).unwrap(), second_bytes);
    assert_eq!(
        writer.save_recovery(&draft(3)),
        Err(ReviewCommitError::LimitExceeded)
    );
    assert!(
        !directory
            .join(format!("{}.json", draft(3).command_id))
            .exists()
    );
    assert_eq!(writer.load_recovery().unwrap(), vec![draft(1), draft(2)]);
    let mut smaller = draft(2);
    smaller.failure = ReviewRecoveryFailure::Cancelled;
    writer.save_recovery(&smaller).unwrap();
    writer.save_recovery(&draft(2)).unwrap();
    assert_eq!(writer.load_recovery().unwrap(), vec![draft(1), draft(2)]);
    assert_eq!(fs::read(first_path).unwrap(), bytes);
}
