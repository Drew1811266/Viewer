#[path = "support/continuous_review.rs"]
mod support;
use support::*;
use viewer_application::{ReviewTaskCancellation, review_workspace::*};
use viewer_domain::review::continuous::*;
use viewer_domain::{review::FeedbackAnchor, *};

#[tokio::test]
async fn first_feedback_is_current_and_next_asset_needs_no_completion() {
    let fixture = Fixture::new();
    let service = fixture.service();
    let assets = service
        .prepare_assets(
            &[EntityId::from_u128(10), EntityId::from_u128(11)],
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    let first_command = service
        .prepare(
            ReviewCommandId::new(),
            None,
            save(assets[0].id, "收紧左袖口"),
        )
        .await
        .unwrap();
    assert!(
        fixture.repository.current().is_none(),
        "prepare must not publish"
    );
    let first = service.apply(first_command).await.unwrap();
    assert_eq!(first.view.projection.actionable.len(), 1);
    let second_command = service
        .prepare(
            ReviewCommandId::new(),
            Some(first.receipt.snapshot.snapshot_id),
            save(assets[1].id, "保留材质，修正接缝"),
        )
        .await
        .unwrap();
    let second = service.apply(second_command).await.unwrap();
    assert_ne!(first.receipt.snapshot, second.receipt.snapshot);
    assert_eq!(second.view.current.unwrap().state.feedback.len(), 2);
    assert_eq!(fixture.evidence.captures(), 2);
}

fn save(asset_version_id: AssetVersionId, text: &str) -> ReviewWorkspaceCommand {
    ReviewWorkspaceCommand::SaveFeedback {
        feedback_id: None,
        text: text.into(),
        targets: vec![TargetEdit::Add {
            asset_version_id,
            anchor: FeedbackAnchor::Asset,
        }],
    }
}

async fn first(f: &Fixture, service: &ContinuousReviewService) -> ReviewApplyResult {
    let assets = service
        .prepare_assets(
            &[EntityId::from_u128(10)],
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    let command = service
        .prepare(ReviewCommandId::new(), None, save(assets[0].id, "原文保留"))
        .await
        .unwrap();
    let result = service.apply(command).await.unwrap();
    assert!(f.repository.current().is_some());
    result
}
fn key(result: &ReviewApplyResult) -> TargetVersionKey {
    let state = &result.view.current.as_ref().unwrap().state;
    state.target_key(state.feedback[0].targets[0].id).unwrap()
}
fn unknown(result: &ReviewApplyResult) -> ArchiveSelection {
    ArchiveSelection {
        expected_snapshot_id: result.receipt.snapshot.snapshot_id,
        groups: vec![ArchiveGroup {
            basis: ArchiveBasis::Unknown,
            targets: vec![key(result)],
        }],
    }
}

#[tokio::test]
async fn partial_archive_restore_and_empty_current_keep_exact_identities() {
    let f = Fixture::new();
    let service = f.service();
    let first = first(&f, &service).await;
    let selection = unknown(&first);
    let plan = service.preview_archive(selection.clone()).await.unwrap();
    assert_eq!(plan.removed, vec![key(&first)]);
    let command = service
        .prepare(
            ReviewCommandId::new(),
            Some(first.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::Archive(selection),
        )
        .await
        .unwrap();
    let archive = command.generated.archive_id;
    let empty = service.apply(command).await.unwrap();
    assert!(
        empty
            .view
            .current
            .as_ref()
            .unwrap()
            .state
            .feedback
            .is_empty()
    );
    assert_ne!(first.receipt.snapshot, empty.receipt.snapshot);
    let history = service
        .history(viewer_application::review_evidence::HistorySelector::Archive(archive))
        .await
        .unwrap();
    assert_eq!(history.entries[0].feedback[0].text, "原文保留");
    assert_eq!(history.restore_actions, vec![key(&first)]);
    let decisions = vec![RestoreDecision {
        historical_key: key(&first),
        choice: RestoreChoice::UseHistorical,
    }];
    service
        .preview_restore(archive, decisions.clone())
        .await
        .unwrap();
    let command = service
        .prepare(
            ReviewCommandId::new(),
            Some(empty.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::Restore {
                archive_id: archive,
                decisions,
            },
        )
        .await
        .unwrap();
    let restored = service.apply(command).await.unwrap();
    assert_eq!(key(&first), key(&restored));
    assert_eq!(restored.view.projection.actionable.len(), 1);
}

#[tokio::test]
async fn old_basis_archive_retains_later_text_and_deduplicates_coverage() {
    let f = Fixture::new();
    let service = f.service();
    let b = first(&f, &service).await;
    let command = service
        .prepare(
            ReviewCommandId::new(),
            Some(b.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::SaveFeedback {
                feedback_id: Some(key(&b).feedback_id),
                text: "后补要求".into(),
                targets: vec![],
            },
        )
        .await
        .unwrap();
    let c = service.apply(command).await.unwrap();
    let selection = ArchiveSelection {
        expected_snapshot_id: c.receipt.snapshot.snapshot_id,
        groups: vec![ArchiveGroup {
            basis: ArchiveBasis::Known {
                snapshot: b.receipt.snapshot,
                source: ArchiveBasisSource::UserSelected,
            },
            targets: vec![key(&b)],
        }],
    };
    let plan = service.preview_archive(selection.clone()).await.unwrap();
    assert!(plan.removed.is_empty());
    assert_eq!(plan.retained.len(), 1);
    let command = service
        .prepare(
            ReviewCommandId::new(),
            Some(c.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::Archive(selection.clone()),
        )
        .await
        .unwrap();
    let d = service.apply(command).await.unwrap();
    assert_eq!(
        d.view.current.as_ref().unwrap().state.feedback[0].text,
        "后补要求"
    );
    let mut repeated = selection;
    repeated.expected_snapshot_id = d.receipt.snapshot.snapshot_id;
    assert!(service.preview_archive(repeated).await.unwrap().is_noop());
}

#[tokio::test]
async fn shared_text_revision_updates_all_targets_and_partial_archive_only_removes_selected() {
    let f = Fixture::new();
    let service = f.service();
    let assets = service
        .prepare_assets(
            &[EntityId::from_u128(10), EntityId::from_u128(11)],
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    let command = ReviewWorkspaceCommand::SaveFeedback {
        feedback_id: None,
        text: "两张都收紧".into(),
        targets: assets
            .iter()
            .map(|a| TargetEdit::Add {
                asset_version_id: a.id,
                anchor: FeedbackAnchor::Asset,
            })
            .collect(),
    };
    let e = service
        .prepare(ReviewCommandId::new(), None, command)
        .await
        .unwrap();
    let a = service.apply(e).await.unwrap();
    let e = service
        .prepare(
            ReviewCommandId::new(),
            Some(a.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::SaveFeedback {
                feedback_id: Some(key(&a).feedback_id),
                text: "两张都收紧，保留纹理".into(),
                targets: vec![],
            },
        )
        .await
        .unwrap();
    let b = service.apply(e).await.unwrap();
    assert_ne!(key(&a).text_revision_id, key(&b).text_revision_id);
    let state = &b.view.current.as_ref().unwrap().state;
    assert_eq!(state.feedback[0].targets.len(), 2);
    assert_eq!(
        state
            .target_key(state.feedback[0].targets[1].id)
            .unwrap()
            .text_revision_id,
        key(&b).text_revision_id
    );
    let e = service
        .prepare(
            ReviewCommandId::new(),
            Some(b.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::Archive(unknown(&b)),
        )
        .await
        .unwrap();
    let c = service.apply(e).await.unwrap();
    assert_eq!(
        c.view.current.as_ref().unwrap().state.feedback[0]
            .targets
            .len(),
        1
    );
    assert_eq!(
        f.evidence.captures(),
        2,
        "text changes reuse immutable bases"
    );
}

#[tokio::test]
async fn retry_returns_original_receipt_without_rolling_current_back() {
    let f = Fixture::new();
    let service = f.service();
    let b = first(&f, &service).await;
    let id = ReviewCommandId::new();
    let cmd = ReviewWorkspaceCommand::SaveFeedback {
        feedback_id: Some(key(&b).feedback_id),
        text: "第二版".into(),
        targets: vec![],
    };
    let e = service
        .prepare(id, Some(b.receipt.snapshot.snapshot_id), cmd.clone())
        .await
        .unwrap();
    assert_eq!(
        e,
        service
            .prepare(id, Some(b.receipt.snapshot.snapshot_id), cmd)
            .await
            .unwrap()
    );
    let c = service.apply(e.clone()).await.unwrap();
    let next = service
        .prepare(
            ReviewCommandId::new(),
            Some(c.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::Withdraw {
                targets: vec![key(&c)],
            },
        )
        .await
        .unwrap();
    let d = service.apply(next).await.unwrap();
    let retry = service.apply(e).await.unwrap();
    assert_eq!(retry.receipt, c.receipt);
    assert_eq!(
        retry.view.current.as_ref().unwrap().reference,
        d.receipt.snapshot
    );
}

#[tokio::test]
async fn cancelled_save_retains_input_and_never_publishes() {
    let f = Fixture::new();
    let service = f.service();
    let assets = service
        .prepare_assets(
            &[EntityId::from_u128(10)],
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    let e = service
        .prepare(
            ReviewCommandId::new(),
            None,
            save(assets[0].id, "取消也不能丢文字"),
        )
        .await
        .unwrap();
    let cancellation = ReviewTaskCancellation::default();
    cancellation.cancel();
    assert!(matches!(
        service.apply_with_cancellation(e, cancellation).await,
        Err(ReviewWorkspaceError::Cancelled)
    ));
    let view = service.view(ReviewStreamId::from_u128(2)).await.unwrap();
    assert!(view.current.is_none());
    assert_eq!(view.recovery[0].editor_input.text, "取消也不能丢文字");
    assert_eq!(view.recovery[0].failure, ReviewRecoveryFailure::Cancelled);
}

#[tokio::test]
async fn historical_continue_creates_new_identity_and_explicit_origin() {
    let f = Fixture::new();
    let service = f.service();
    let b = first(&f, &service).await;
    let assets = service
        .prepare_assets(
            &[EntityId::from_u128(11)],
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    let history_ref = HistoryRef {
        project_id: ProjectId::from_u128(1),
        stream_id: ReviewStreamId::from_u128(2),
        source: HistorySource::Snapshot {
            snapshot: b.receipt.snapshot,
            keys: vec![key(&b)],
        },
    };
    let e = service
        .prepare(
            ReviewCommandId::new(),
            Some(b.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::ContinueHistorical {
                history_ref: history_ref.clone(),
                bindings: vec![SourceBindingDecision {
                    target_key: key(&b),
                    new_asset_version_id: assets[0].id,
                    anchor: FeedbackAnchor::Asset,
                    confirmation: SourceBindingConfirmation::UserConfirmed,
                }],
            },
        )
        .await
        .unwrap();
    let c = service.apply(e).await.unwrap();
    let new = &c.view.current.as_ref().unwrap().state.feedback[1];
    assert_ne!(new.id, key(&b).feedback_id);
    assert_eq!(new.history_ref, Some(history_ref));
    assert_eq!(new.text, "原文保留");
}

#[tokio::test]
async fn stale_preview_and_forged_context_or_digest_cannot_commit() {
    let f = Fixture::new();
    let service = f.service();
    let b = first(&f, &service).await;
    let archive = service
        .prepare(
            ReviewCommandId::new(),
            Some(b.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::Archive(unknown(&b)),
        )
        .await
        .unwrap();
    let e = service
        .prepare(
            ReviewCommandId::new(),
            Some(b.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::SaveFeedback {
                feedback_id: Some(key(&b).feedback_id),
                text: "新文字".into(),
                targets: vec![],
            },
        )
        .await
        .unwrap();
    let c = service.apply(e.clone()).await.unwrap();
    assert!(matches!(
        service.apply(archive).await,
        Err(ReviewWorkspaceError::Repository(
            ReviewCommitError::StaleSnapshot
        ))
    ));
    let mut forged = e.clone();
    forged.payload_digest = [9; 32];
    assert!(matches!(
        service.apply(forged).await,
        Err(ReviewWorkspaceError::Repository(
            ReviewCommitError::CommandConflict
        ))
    ));
    let mut forged = e;
    forged.context.stream_id = ReviewStreamId::new();
    assert!(matches!(
        service.apply(forged).await,
        Err(ReviewWorkspaceError::WrongContext)
    ));
    assert_eq!(
        f.repository.current().unwrap().reference,
        c.receipt.snapshot
    );
}

#[tokio::test]
async fn empty_feedback_and_unprepared_preview_never_publish() {
    let f = Fixture::new();
    let service = f.service();
    let e = service
        .prepare(
            ReviewCommandId::new(),
            None,
            save(AssetVersionId::new(), "意见"),
        )
        .await
        .unwrap();
    assert!(matches!(
        service.apply(e).await,
        Err(ReviewWorkspaceError::PreviewRequired)
    ));
    let assets = service
        .prepare_assets(
            &[EntityId::from_u128(10)],
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    let e = service
        .prepare(ReviewCommandId::new(), None, save(assets[0].id, "  "))
        .await
        .unwrap();
    assert!(matches!(
        service.apply(e).await,
        Err(ReviewWorkspaceError::Domain(
            ContinuousReviewError::InvalidData
        ))
    ));
    assert!(f.repository.current().is_none());
}

#[tokio::test]
async fn save_failure_keeps_input_and_unknown_commit_retry_reuses_receipt() {
    let f = Fixture::new();
    let service = f.service();
    let assets = service
        .prepare_assets(
            &[EntityId::from_u128(10)],
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    let e = service
        .prepare(ReviewCommandId::new(), None, save(assets[0].id, "失败输入"))
        .await
        .unwrap();
    *f.repository.fail_commit.lock().unwrap() = Some(ReviewCommitError::Io);
    assert!(matches!(
        service.apply(e.clone()).await,
        Err(ReviewWorkspaceError::Repository(ReviewCommitError::Io))
    ));
    let view = service.view(ReviewStreamId::from_u128(2)).await.unwrap();
    assert!(view.current.is_none());
    assert_eq!(view.recovery[0].editor_input.text, "失败输入");
    *f.repository.fail_commit.lock().unwrap() = Some(ReviewCommitError::OutcomeUnknown);
    assert!(matches!(
        service.apply(e.clone()).await,
        Err(ReviewWorkspaceError::Repository(
            ReviewCommitError::OutcomeUnknown
        ))
    ));
    let published = f.repository.current().unwrap().reference;
    let result = service.apply(e).await.unwrap();
    assert_eq!(result.receipt.snapshot, published);
    assert!(result.view.recovery.is_empty());
}

#[tokio::test]
async fn changed_source_is_pending_without_changing_snapshot_and_text_stays_editable() {
    let f = Fixture::new();
    let service = f.service();
    let b = first(&f, &service).await;
    *f.assets.changed.lock().unwrap() = true;
    let view = service.view(ReviewStreamId::from_u128(2)).await.unwrap();
    assert_eq!(view.current.as_ref().unwrap().reference, b.receipt.snapshot);
    assert!(view.projection.actionable.is_empty());
    assert_eq!(view.projection.needs_confirmation.len(), 1);
    let e = service
        .prepare(
            ReviewCommandId::new(),
            Some(b.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::SaveFeedback {
                feedback_id: Some(key(&b).feedback_id),
                text: "旧版意见仍可修改".into(),
                targets: vec![],
            },
        )
        .await
        .unwrap();
    let c = service.apply(e).await.unwrap();
    assert_eq!(c.view.projection.needs_confirmation.len(), 1);
    assert_eq!(f.evidence.captures(), 1);
}

#[tokio::test]
async fn stale_text_is_recoverable_instead_of_being_dropped() {
    let f = Fixture::new();
    let service = f.service();
    let b = first(&f, &service).await;
    let stale = service
        .prepare(
            ReviewCommandId::new(),
            Some(b.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::SaveFeedback {
                feedback_id: Some(key(&b).feedback_id),
                text: "尚未保存的文字".into(),
                targets: vec![],
            },
        )
        .await
        .unwrap();
    let e = service
        .prepare(
            ReviewCommandId::new(),
            Some(b.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::Withdraw {
                targets: vec![key(&b)],
            },
        )
        .await
        .unwrap();
    service.apply(e).await.unwrap();
    assert!(matches!(
        service.apply(stale).await,
        Err(ReviewWorkspaceError::Repository(
            ReviewCommitError::StaleSnapshot
        ))
    ));
    let view = service.view(ReviewStreamId::from_u128(2)).await.unwrap();
    assert!(
        view.recovery
            .iter()
            .any(|r| r.editor_input.text == "尚未保存的文字"
                && r.failure == ReviewRecoveryFailure::StaleSnapshot)
    );
}

#[tokio::test]
async fn post_commit_view_failure_reports_committed_receipt_not_an_unsaved_edit() {
    let f = Fixture::new();
    let service = f.service();
    let assets = service
        .prepare_assets(
            &[EntityId::from_u128(10)],
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    let e = service
        .prepare(ReviewCommandId::new(), None, save(assets[0].id, "已提交"))
        .await
        .unwrap();
    *f.repository.fail_view_after_commit.lock().unwrap() = true;
    let Err(ReviewWorkspaceError::CommittedViewUnavailable(receipt)) = service.apply(e).await
    else {
        panic!("must expose the known committed receipt");
    };
    assert_eq!(receipt.snapshot, f.repository.current().unwrap().reference);
}

#[tokio::test]
async fn text_edit_reuses_local_annotation_pixels_but_updates_text_revision_mapping() {
    let f = Fixture::new();
    let service = f.service();
    let assets = service
        .prepare_assets(
            &[EntityId::from_u128(10)],
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    let anchor = FeedbackAnchor::ImageRect(
        viewer_domain::review::NormalizedRect::new(0.1, 0.1, 0.3, 0.3).unwrap(),
    );
    let e = service
        .prepare(
            ReviewCommandId::new(),
            None,
            ReviewWorkspaceCommand::SaveFeedback {
                feedback_id: None,
                text: "局部意见".into(),
                targets: vec![TargetEdit::Add {
                    asset_version_id: assets[0].id,
                    anchor: anchor.clone(),
                }],
            },
        )
        .await
        .unwrap();
    let b = service.apply(e).await.unwrap();
    let renders = f.evidence.renders();
    let e = service
        .prepare(
            ReviewCommandId::new(),
            Some(b.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::SaveFeedback {
                feedback_id: Some(key(&b).feedback_id),
                text: "仅改文字".into(),
                targets: vec![],
            },
        )
        .await
        .unwrap();
    let c = service.apply(e).await.unwrap();
    assert_eq!(f.evidence.renders(), renders);
    let EvidenceCapability::Image { annotations, .. } =
        &c.view.current.as_ref().unwrap().evidence[0].capability
    else {
        panic!()
    };
    assert_eq!(annotations[0].key, key(&c));
    assert_ne!(annotations[0].key, key(&b));
    let changed = FeedbackAnchor::ImageRect(
        viewer_domain::review::NormalizedRect::new(0.2, 0.2, 0.4, 0.4).unwrap(),
    );
    let e = service
        .prepare(
            ReviewCommandId::new(),
            Some(c.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::SaveFeedback {
                feedback_id: Some(key(&c).feedback_id),
                text: "仅改文字".into(),
                targets: vec![TargetEdit::Redraw {
                    key: key(&c),
                    asset_version_id: assets[0].id,
                    anchor: changed,
                }],
            },
        )
        .await
        .unwrap();
    let d = service.apply(e).await.unwrap();
    assert_eq!(f.evidence.renders(), renders + 1);
    assert_ne!(key(&d).target_revision_id, key(&c).target_revision_id);
}

#[tokio::test]
async fn prepared_command_cache_has_an_aggregate_memory_budget() {
    use viewer_domain::review::{ImageStroke, NormalizedPoint};
    let f = Fixture::new();
    let service = f.service();
    let stroke = ImageStroke::new(
        (0..2048)
            .map(|i| {
                NormalizedPoint::new(
                    if i % 2 == 0 { 0.1 } else { 0.2 },
                    if i % 2 == 0 { 0.1 } else { 0.2 },
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    let mut limited = false;
    for _ in 0..40 {
        let command = ReviewWorkspaceCommand::SaveFeedback {
            feedback_id: None,
            text: "尚未提交的画笔".into(),
            targets: (0..64)
                .map(|_| TargetEdit::Add {
                    asset_version_id: AssetVersionId::from_u128(10),
                    anchor: FeedbackAnchor::ImageStroke(stroke.clone()),
                })
                .collect(),
        };
        match service.prepare(ReviewCommandId::new(), None, command).await {
            Ok(_) => {}
            Err(ReviewWorkspaceError::Domain(ContinuousReviewError::LimitExceeded)) => {
                limited = true;
                break;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    assert!(
        limited,
        "128 entries alone is not a bounded 64 MiB input cache"
    );
    assert!(f.repository.current().is_none());
}

#[tokio::test]
async fn importing_and_adopting_usage_never_archives_or_claims_execution() {
    let f = Fixture::new();
    let original = f.service();
    let b = first(&f, &original).await;
    let importer = usage_importer(&b);
    let service = f.service().with_usage_importer(importer.clone());
    let source = importer.0.lock().unwrap().source.clone();
    let preview = service.inspect_usage(source).await.unwrap();
    assert_eq!(
        f.repository.current().unwrap().reference,
        b.receipt.snapshot
    );
    let command = ReviewWorkspaceCommand::AdoptUsage {
        declaration_id: preview.declaration.id,
    };
    let e = service
        .prepare(
            ReviewCommandId::new(),
            Some(b.receipt.snapshot.snapshot_id),
            command.clone(),
        )
        .await
        .unwrap();
    let c = service.apply(e).await.unwrap();
    assert_eq!(key(&b), key(&c));
    assert_eq!(c.view.projection.actionable.len(), 1);
    let e = service
        .prepare(
            ReviewCommandId::new(),
            Some(c.receipt.snapshot.snapshot_id),
            command,
        )
        .await
        .unwrap();
    assert!(matches!(
        service.apply(e).await,
        Err(ReviewWorkspaceError::NoChanges)
    ));
}

#[tokio::test]
async fn changed_usage_after_inspection_cannot_be_adopted() {
    let f = Fixture::new();
    let original = f.service();
    let b = first(&f, &original).await;
    let importer = usage_importer(&b);
    let service = f.service().with_usage_importer(importer.clone());
    let source = importer.0.lock().unwrap().source.clone();
    let preview = service.inspect_usage(source).await.unwrap();
    importer.0.lock().unwrap().source_digest = [0; 32];
    let e = service
        .prepare(
            ReviewCommandId::new(),
            Some(b.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::AdoptUsage {
                declaration_id: preview.declaration.id,
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        service.apply(e).await,
        Err(ReviewWorkspaceError::Usage(UsageImportError::SourceChanged))
    ));
    assert_eq!(
        f.repository.current().unwrap().reference,
        b.receipt.snapshot
    );
}

#[tokio::test]
async fn selected_usage_is_adopted_atomically_with_archive_but_never_inferred() {
    let f = Fixture::new();
    let original = f.service();
    let b = first(&f, &original).await;
    let importer = usage_importer(&b);
    let service = f.service().with_usage_importer(importer.clone());
    let source = importer.0.lock().unwrap().source.clone();
    let preview = service.inspect_usage(source).await.unwrap();
    let selection = ArchiveSelection {
        expected_snapshot_id: b.receipt.snapshot.snapshot_id,
        groups: vec![ArchiveGroup {
            basis: ArchiveBasis::Known {
                snapshot: b.receipt.snapshot,
                source: ArchiveBasisSource::AgentDeclared {
                    usage_id: preview.declaration.id,
                },
            },
            targets: vec![key(&b)],
        }],
    };
    let e = service
        .prepare(
            ReviewCommandId::new(),
            Some(b.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::Archive(selection),
        )
        .await
        .unwrap();
    *f.repository.fail_commit.lock().unwrap() = Some(ReviewCommitError::Io);
    assert!(service.apply(e.clone()).await.is_err());
    assert!(
        ContinuousReviewRepositoryPort::load_usage(
            f.repository.as_ref(),
            ReviewStreamId::from_u128(2),
            preview.declaration.id
        )
        .unwrap()
        .is_none()
    );
    let c = service.apply(e).await.unwrap();
    assert!(c.view.current.as_ref().unwrap().state.feedback.is_empty());
    assert!(
        ContinuousReviewRepositoryPort::load_usage(
            f.repository.as_ref(),
            ReviewStreamId::from_u128(2),
            preview.declaration.id
        )
        .unwrap()
        .is_some()
    );
}

#[tokio::test]
async fn usage_import_requires_explicit_capability_and_context() {
    let f = Fixture::new();
    let service = f.service();
    assert!(matches!(
        service
            .inspect_usage(RelativePath::parse("usage.json").unwrap())
            .await,
        Err(ReviewWorkspaceError::CapabilityUnavailable)
    ));
    let b = first(&f, &service).await;
    let importer = usage_importer(&b);
    importer.0.lock().unwrap().declaration.stream_id = ReviewStreamId::new();
    let source = importer.0.lock().unwrap().source.clone();
    assert!(matches!(
        f.service()
            .with_usage_importer(importer)
            .inspect_usage(source)
            .await,
        Err(ReviewWorkspaceError::Usage(UsageImportError::WrongContext))
    ));
}

#[tokio::test]
async fn producer_binding_cannot_use_an_unrelated_previous_version_even_if_the_target_id_matches() {
    let f = Fixture::new();
    let original = f.service();
    let b = first(&f, &original).await;
    let importer = usage_importer(&b);
    let service = f.service().with_usage_importer(importer.clone());
    let assets = service
        .prepare_assets(
            &[EntityId::from_u128(11), EntityId::from_u128(12)],
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    let rebind = SourceBindingDecision {
        target_key: key(&b),
        new_asset_version_id: assets[0].id,
        anchor: FeedbackAnchor::Asset,
        confirmation: SourceBindingConfirmation::UserConfirmed,
    };
    let e = service
        .prepare(
            ReviewCommandId::new(),
            Some(b.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::ConfirmSource(rebind),
        )
        .await
        .unwrap();
    let c = service.apply(e).await.unwrap();
    {
        let mut preview = importer.0.lock().unwrap();
        preview.declaration.outputs = vec![UsageOutput {
            relative_path: assets[1].relative_path.clone(),
            blake3: assets[1].evidence.blake3.unwrap(),
            previous_asset_version_id: assets[0].id,
        }];
    }
    let source = importer.0.lock().unwrap().source.clone();
    let preview = service.inspect_usage(source).await.unwrap();
    let e = service
        .prepare(
            ReviewCommandId::new(),
            Some(c.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::ConfirmSource(SourceBindingDecision {
                target_key: key(&c),
                new_asset_version_id: assets[1].id,
                anchor: FeedbackAnchor::Asset,
                confirmation: SourceBindingConfirmation::ProducerVerifiedAndPositionConfirmed {
                    usage_id: preview.declaration.id,
                },
            }),
        )
        .await
        .unwrap();
    assert!(matches!(
        service.apply(e).await,
        Err(ReviewWorkspaceError::Usage(UsageImportError::InvalidScope))
    ));
}

#[tokio::test]
async fn verified_producer_mapping_still_requires_an_explicit_selected_binding() {
    let f = Fixture::new();
    let original = f.service();
    let b = first(&f, &original).await;
    let importer = usage_importer(&b);
    let service = f.service().with_usage_importer(importer.clone());
    let new = service
        .prepare_assets(
            &[EntityId::from_u128(11)],
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap()
        .remove(0);
    let previous = b.view.current.as_ref().unwrap().state.feedback[0].targets[0].asset_version_id;
    importer.0.lock().unwrap().declaration.outputs = vec![UsageOutput {
        relative_path: new.relative_path.clone(),
        blake3: new.evidence.blake3.unwrap(),
        previous_asset_version_id: previous,
    }];
    let source = importer.0.lock().unwrap().source.clone();
    let preview = service.inspect_usage(source).await.unwrap();
    assert_eq!(
        f.repository.current().unwrap().state.feedback[0].targets[0].asset_version_id,
        previous
    );
    let binding = SourceBindingDecision {
        target_key: key(&b),
        new_asset_version_id: new.id,
        anchor: FeedbackAnchor::Asset,
        confirmation: SourceBindingConfirmation::ProducerVerifiedAndPositionConfirmed {
            usage_id: preview.declaration.id,
        },
    };
    let e = service
        .prepare(
            ReviewCommandId::new(),
            Some(b.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::ConfirmSource(binding),
        )
        .await
        .unwrap();
    let c = service.apply(e).await.unwrap();
    assert_eq!(
        c.view.current.as_ref().unwrap().state.feedback[0].targets[0].asset_version_id,
        new.id
    );
    assert_eq!(
        c.view.current.as_ref().unwrap().state.feedback[0].text,
        "原文保留"
    );
    assert_eq!(c.view.projection.actionable.len(), 1);
}
