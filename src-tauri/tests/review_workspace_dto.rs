use serde_json::{Value, json};
use viewer_application::review_workspace::ReviewWorkspaceCommand;
use viewer_desktop::dto::review_workspace::{
    PrepareReviewAssetsRequestDto, PreparedReviewCommandDto, ReviewHistoryViewDto,
    ReviewWorkspaceErrorDto, ReviewWorkspaceViewDto,
};

fn id(n: u32) -> String {
    format!("00000000-0000-0000-0000-{n:012x}")
}
fn envelope() -> Value {
    json!({
        "context": {"projectId": id(1), "streamId": id(2), "production": null},
        "commandId": id(3), "expectedSnapshotId": null, "payloadDigest": "ab".repeat(32),
        "generated": {
            "snapshotId": id(4), "feedbackId": id(5), "textRevisionId": id(6), "archiveId": id(7),
            "targets": [{"targetId": id(8), "targetRevisionId": id(9)}], "migration": [], "createdAtMs": 1234
        },
        "usageSelections": [{"id": id(10), "candidate": {
            "canonicalDigest": "cd".repeat(32), "sourceDigest": "ef".repeat(32), "source": "producer/usage.json"
        }}],
        "command": {"kind": "save_feedback", "feedbackId": null, "text": "袖口收紧，保留材质", "targets": [
            {"kind": "add", "assetVersionId": id(11), "anchor": {"kind": "image_rect", "x": 0.1, "y": 0.2, "width": 0.3, "height": 0.4}}
        ]}
    })
}

#[test]
fn review_workspace_complete_envelope_roundtrip_preserves_retry_identity() {
    let value = envelope();
    let dto: PreparedReviewCommandDto = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(dto.0.generated.created_at_ms, 1234);
    assert_eq!(dto.0.generated.targets[0].0.to_string(), id(8));
    assert_eq!(
        dto.0.usage_selections[0]
            .candidate
            .as_ref()
            .unwrap()
            .source
            .as_str(),
        "producer/usage.json"
    );
    assert!(
        matches!(&dto.0.command, ReviewWorkspaceCommand::SaveFeedback { text, .. } if text == "袖口收紧，保留材质")
    );
    assert_eq!(serde_json::to_value(dto).unwrap(), value);
}

#[test]
fn review_workspace_rejects_partial_ambiguous_and_forged_wire_fields() {
    for field in [
        "context",
        "generated",
        "usageSelections",
        "expectedSnapshotId",
    ] {
        let mut value = envelope();
        value.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<PreparedReviewCommandDto>(value).is_err(),
            "missing {field}"
        );
    }
    for (pointer, invalid) in [
        ("/payloadDigest", json!("AB".repeat(32))),
        (
            "/context/projectId",
            json!("00000000000000000000000000000001"),
        ),
        ("/generated/createdAtMs", json!(9007199254740992_u64)),
        (
            "/usageSelections/0/candidate/source",
            json!("../private.json"),
        ),
        ("/command/targets/0/anchor/width", json!(1.4)),
        ("/command/text", json!("x".repeat(65_537))),
    ] {
        let mut value = envelope();
        *value.pointer_mut(pointer).unwrap() = invalid;
        assert!(
            serde_json::from_value::<PreparedReviewCommandDto>(value).is_err(),
            "accepted {pointer}"
        );
    }
    for pointer in [
        "",
        "/context",
        "/generated",
        "/usageSelections/0/candidate",
        "/command",
        "/command/targets/0/anchor",
    ] {
        let mut value = envelope();
        value
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("path".into(), json!("/private/secret"));
        assert!(
            serde_json::from_value::<PreparedReviewCommandDto>(value).is_err(),
            "unknown field at {pointer}"
        );
    }
    let mut value = envelope();
    value["command"]["targets"][0]["anchor"] = json!({"kind": "asset", "path": "/private/secret"});
    assert!(serde_json::from_value::<PreparedReviewCommandDto>(value).is_err());
}

#[test]
fn review_workspace_prebinding_accepts_only_session_generation_and_indexed_ids() {
    let request = json!({"sessionId": id(1), "generation": 2, "entityIds": [id(3)]});
    let dto: PrepareReviewAssetsRequestDto = serde_json::from_value(request.clone()).unwrap();
    assert_eq!(dto.entity_ids.len(), 1);
    let mut unsafe_request = request;
    unsafe_request["path"] = json!("/private/image.png");
    assert!(serde_json::from_value::<PrepareReviewAssetsRequestDto>(unsafe_request).is_err());
}

#[test]
fn review_workspace_result_preserves_commit_receipt_and_empty_current_is_not_history() {
    use viewer_application::review_workspace::*;
    use viewer_domain::{review::continuous::*, *};
    let receipt = ReviewCommitReceipt {
        command_id: ReviewCommandId::from_u128(3),
        payload_digest: [0xab; 32],
        snapshot: SnapshotRef {
            snapshot_id: ReviewSnapshotId::from_u128(4),
            blake3: [0xcd; 32],
        },
    };
    let error =
        ReviewWorkspaceErrorDto::from(ReviewWorkspaceError::CommittedViewUnavailable(receipt));
    let value = serde_json::to_value(error).unwrap();
    assert_eq!(value["code"], "committed_view_unavailable");
    assert_eq!(value["committedReceipt"]["commandId"], id(3));
    assert_eq!(value["committedReceipt"]["snapshot"]["snapshotId"], id(4));
    let view = ReviewWorkspaceView {
        stream_id: ReviewStreamId::from_u128(2),
        current: None,
        source_checks: vec![],
        projection: CurrentReviewProjection {
            actionable: vec![],
            needs_confirmation: vec![],
        },
        recovery: vec![],
        migration: None,
        capabilities: ReviewWorkspaceCapabilities {
            continuous_editing: true,
            usage_import: true,
            migration: false,
        },
    };
    let value = serde_json::to_value(ReviewWorkspaceViewDto::from(view)).unwrap();
    assert!(value["current"].is_null());
    assert_eq!(value["projection"]["actionable"], json!([]));
    let history = HistoryView {
        selector: viewer_application::review_evidence::HistorySelector::Archive(
            ReviewArchiveId::from_u128(7),
        ),
        entries: vec![],
        legacy: None,
        limitations: vec![ReviewHistoryLimitation::BackgroundOnly],
        restore_actions: vec![],
    };
    let value = serde_json::to_value(ReviewHistoryViewDto::from(history)).unwrap();
    assert_eq!(
        value["selector"],
        json!({"kind":"archive", "archiveId":id(7)})
    );
    assert!(value.get("actionable").is_none());
    assert!(value.get("current").is_none());
}

#[test]
fn review_workspace_evidence_request_cannot_supply_path_digest_or_unknown_role() {
    use viewer_desktop::dto::review_workspace::ReviewEvidenceRequestDto;
    let valid = json!({"sessionId":id(1),"generation":2,"selector":{"kind":"snapshot","snapshot":{"snapshotId":id(3),"blake3":"ab".repeat(32)}},"assetVersionId":id(4),"role":"base"});
    assert!(serde_json::from_value::<ReviewEvidenceRequestDto>(valid.clone()).is_ok());
    for field in ["path", "blake3", "url"] {
        let mut value = valid.clone();
        value[field] = json!("/private/image.png");
        assert!(serde_json::from_value::<ReviewEvidenceRequestDto>(value).is_err());
    }
    let mut value = valid;
    value["role"] = json!("source_path");
    assert!(serde_json::from_value::<ReviewEvidenceRequestDto>(value).is_err());
}

#[test]
fn review_workspace_rejects_oversized_target_and_brush_arrays_during_decode() {
    let mut value = envelope();
    value["command"]["targets"] = json!(vec![value["command"]["targets"][0].clone(); 10_001]);
    assert!(serde_json::from_value::<PreparedReviewCommandDto>(value).is_err());
    let mut value = envelope();
    value["command"]["targets"][0]["anchor"] =
        json!({"kind":"image_stroke","points":vec![json!({"x":0.1,"y":0.2});2049]});
    assert!(serde_json::from_value::<PreparedReviewCommandDto>(value).is_err());
}

#[test]
fn review_workspace_all_command_variants_preserve_nested_choices_and_generated_ids() {
    let key = json!({"feedbackId":id(20),"textRevisionId":id(21),"targetId":id(22),"targetRevisionId":id(23)});
    let snapshot = json!({"snapshotId":id(30),"blake3":"aa".repeat(32)});
    let legacy = json!({"roundId":id(40),"feedbackId":id(41),"targetIndex":0});
    let history = json!({"projectId":id(1),"streamId":id(2),"source":{"kind":"snapshot","snapshot":snapshot,"keys":[key]}});
    let binding = json!({"targetKey":key,"newAssetVersionId":id(50),"anchor":{"kind":"asset"},"confirmation":{"kind":"producer_verified_and_position_confirmed","usageId":id(51)}});
    let migration_binding = json!({"legacyTarget":legacy,"newAssetVersionId":id(50),"anchor":{"kind":"asset"},"positionConfirmed":true});
    let commands = vec![
        json!({"kind":"withdraw","targets":[key]}),
        json!({"kind":"archive","expectedSnapshotId":id(30),"groups":[{"basis":{"kind":"known","snapshot":snapshot,"source":{"kind":"agent_declared","usageId":id(51)}},"targets":[key]}]}),
        json!({"kind":"restore","archiveId":id(7),"decisions":[{"historicalKey":key,"choice":{"kind":"continue_as_new","feedbackId":id(60),"textRevisionId":id(61),"targetId":id(62),"targetRevisionId":id(63),"targetAssetVersionId":id(50),"confirmedAnchor":null,"createdAtMs":1234}}]}),
        json!({"kind":"continue_historical","historyRef":history,"bindings":[binding]}),
        {
            let mut value = binding.clone();
            value["kind"] = json!("confirm_source");
            value
        },
        json!({"kind":"confirm_applicability","key":key,"assetVersionId":id(50),"anchor":{"kind":"asset"}}),
        json!({"kind":"adopt_usage","declarationId":id(51)}),
        json!({"kind":"migrate","inspectionDigest":"ab".repeat(32),"choice":{"kind":"continue_selected","legacyTargets":[legacy],"bindings":[migration_binding]}}),
        json!({"kind":"continue_legacy","historyRef":{"projectId":id(1),"streamId":id(2),"source":{"kind":"legacy","roundId":id(40),"recordBlake3":"ef".repeat(32),"targets":[legacy]}},"bindings":[migration_binding]}),
    ];
    for command in commands {
        let mut value = envelope();
        value["command"] = command;
        value["context"]["production"] = json!({"taskId":"task-a","batchId":"batch-a"});
        value["generated"]["migration"] = json!([{"roundId":id(40),"legacyFeedbackId":id(41),"feedbackId":id(60),"textRevisionId":id(61),"targets":[{"targetIndex":0,"targetId":id(62),"targetRevisionId":id(63)}]}]);
        let decoded: PreparedReviewCommandDto = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), value);
    }
    let mut value = envelope();
    value["command"] = json!({"kind":"migrate","inspectionDigest":"ab".repeat(32),"choice":{"kind":"keep_history_only","extra":true}});
    assert!(serde_json::from_value::<PreparedReviewCommandDto>(value).is_err());
}
