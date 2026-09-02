use serde_json::{Value, json};
use viewer_domain::review::continuous::ContinuousReviewState;
use viewer_domain::{ProjectId, ReviewCommandId, ReviewSnapshotId, ReviewStreamId};
use viewer_infrastructure::review::ReviewProtocolError;
use viewer_infrastructure::review::v3::{ReviewStateRecord, decode_state_v3, encode_state_v3};
use viewer_infrastructure::review::v3::{
    decode_archive_v3, decode_index_v3, decode_usage_v1, encode_archive_v3, encode_index_v3,
    encode_usage_v1,
};
use viewer_infrastructure::review::v3::{decode_read_result_v3, encode_read_result_v3};

fn empty_record() -> ReviewStateRecord {
    ReviewStateRecord {
        state: ContinuousReviewState::empty(
            ProjectId::from_u128(1),
            ReviewStreamId::from_u128(2),
            ReviewSnapshotId::from_u128(3),
        ),
        command_id: ReviewCommandId::from_u128(4),
        payload_digest: [5; 32],
        changes: vec![],
        evidence: vec![],
    }
}

fn image_document() -> Value {
    let id = |value: u128| format!("00000000-0000-0000-0000-{value:012x}");
    let key = json!({"feedbackId":id(10), "textRevisionId":id(11), "targetId":id(12), "targetRevisionId":id(13)});
    json!({
        "protocolVersion":"viewer.review/3", "kind":"state", "projectId":id(1), "reviewStreamId":id(2),
        "snapshotId":id(3), "parent":null, "commandId":id(4), "payloadDigest":"05".repeat(32),
        "assets":[{"assetVersionId":id(14),"sourceEntityId":null,"relativePath":"image.png",
            "evidence":{"sizeBytes":100,"modifiedNs":"1","blake3":"01".repeat(32)},
            "media":{"kind":"image","width":64,"height":64},"producerAssetId":null,"parentAssetVersionId":null}],
        "feedback":[{"feedbackId":id(10),"textRevisionId":id(11),"text":" 袖口收紧，保留褶皱\n","createdAtMs":1,"historyRef":null,
            "targets":[{"targetId":id(12),"targetRevisionId":id(13),"assetVersionId":id(14),
                "anchor":{"kind":"imageRect","x":0.1,"y":0.2,"width":0.3,"height":0.4},"availability":{"kind":"ready"}}]}],
        "changes":[{"targetId":id(12),"before":null,"after":key,"kind":"added","archiveId":null,"historicalKey":null}],
        "evidence":[{"assetVersionId":id(14),"capability":{"kind":"image",
            "base":{"blake3":"02".repeat(32),"sizeBytes":100,"width":64,"height":64},
            "annotated":{"blake3":"03".repeat(32),"sizeBytes":150,"width":64,"height":64},
            "annotations":[{"ordinal":1,"key":key}]}}]
    })
}

#[test]
fn image_feedback_round_trips_original_text_and_exact_marker_identity() {
    let document = image_document();
    let decoded = decode_state_v3(&serde_json::to_vec(&document).unwrap()).unwrap();
    assert_eq!(decoded.state.feedback[0].text, " 袖口收紧，保留褶皱\n");
    assert_eq!(
        serde_json::from_slice::<Value>(&encode_state_v3(&decoded).unwrap()).unwrap(),
        document
    );
}

#[test]
fn image_state_rejects_missing_evidence_duplicate_markers_and_invalid_domain_relations() {
    for mutation in [
        "unknown_nested",
        "missing_nullable",
        "duplicate_target",
        "missing_asset",
        "bounds",
        "evidence_absent",
        "legacy_forged",
        "wrong_marker",
        "duplicate_marker",
        "wrong_dimensions",
        "pixels",
        "cross_stream",
        "text_bytes",
    ] {
        let mut value = image_document();
        match mutation {
            "unknown_nested" => {
                value["feedback"][0]["targets"][0]["availability"]["passed"] = json!(true)
            }
            "missing_nullable" => {
                value.as_object_mut().unwrap().remove("parent");
            }
            "duplicate_target" => {
                let target = value["feedback"][0]["targets"][0].clone();
                value["feedback"][0]["targets"]
                    .as_array_mut()
                    .unwrap()
                    .push(target);
            }
            "missing_asset" => value["assets"] = json!([]),
            "bounds" => value["feedback"][0]["targets"][0]["anchor"]["width"] = json!(1.0),
            "evidence_absent" => value["evidence"] = json!([]),
            "legacy_forged" => value["evidence"][0]["capability"] = json!({"kind":"legacyAbsent"}),
            "wrong_marker" => {
                value["evidence"][0]["capability"]["annotations"][0]["ordinal"] = json!(2)
            }
            "duplicate_marker" => {
                let marker = value["evidence"][0]["capability"]["annotations"][0].clone();
                value["evidence"][0]["capability"]["annotations"]
                    .as_array_mut()
                    .unwrap()
                    .push(marker);
            }
            "wrong_dimensions" => {
                value["evidence"][0]["capability"]["annotated"]["width"] = json!(32)
            }
            "pixels" => value["evidence"][0]["capability"]["base"]["height"] = json!(u32::MAX),
            "cross_stream" => {
                value["feedback"][0]["historyRef"] = json!({"projectId":value["projectId"],"reviewStreamId":"00000000-0000-0000-0000-000000000999","source":{"kind":"snapshot","snapshot":{"snapshotId":"00000000-0000-0000-0000-000000000998","blake3":"00".repeat(32)},"keys":[value["changes"][0]["after"]]}})
            }
            "text_bytes" => value["feedback"][0]["text"] = json!("图".repeat(22_000)),
            _ => unreachable!(),
        }
        assert!(
            decode_state_v3(&serde_json::to_vec(&value).unwrap()).is_err(),
            "accepted {mutation}"
        );
    }
}

fn index_document() -> Value {
    let state = image_document();
    json!({"protocolVersion":"viewer.review/3","kind":"index","projectId":state["projectId"],
        "streams":[{"reviewStreamId":state["reviewStreamId"],"taskId":null,"batchId":null,
            "currentRef":{"snapshotId":state["snapshotId"],"blake3":"10".repeat(32)},
            "archiveRefs":[],"legacyRefs":[],"usageRefs":[]}]})
}

fn archive_document() -> Value {
    let state = image_document();
    json!({"protocolVersion":"viewer.review/3","kind":"archive","projectId":state["projectId"],"reviewStreamId":state["reviewStreamId"],
        "archiveId":"00000000-0000-0000-0000-000000000030","createdAtMs":2,
        "beforeRef":{"snapshotId":state["snapshotId"],"blake3":"10".repeat(32)},
        "resultSnapshotId":"00000000-0000-0000-0000-000000000031",
        "groups":[{"usageBasis":null,"targets":[state["changes"][0]["after"]]}],
        "removed":[state["changes"][0]["after"]],"retained":[]})
}

fn usage_document() -> Value {
    let state = image_document();
    json!({"protocolVersion":"viewer.review.usage/1","declarationId":"00000000-0000-0000-0000-000000000040",
        "projectId":state["projectId"],"reviewStreamId":state["reviewStreamId"],
        "basis":{"snapshotId":state["snapshotId"],"blake3":"10".repeat(32)},
        "targets":[state["changes"][0]["after"]],"outputs":[]})
}

#[test]
fn index_archive_and_usage_round_trip_with_explicit_unknown_basis() {
    let index = index_document();
    assert_eq!(
        serde_json::from_slice::<Value>(
            &encode_index_v3(&decode_index_v3(&serde_json::to_vec(&index).unwrap()).unwrap())
                .unwrap()
        )
        .unwrap(),
        index
    );
    let archive = archive_document();
    assert_eq!(
        serde_json::from_slice::<Value>(
            &encode_archive_v3(&decode_archive_v3(&serde_json::to_vec(&archive).unwrap()).unwrap())
                .unwrap()
        )
        .unwrap(),
        archive
    );
    let usage = usage_document();
    assert_eq!(
        serde_json::from_slice::<Value>(
            &encode_usage_v1(&decode_usage_v1(&serde_json::to_vec(&usage).unwrap()).unwrap())
                .unwrap()
        )
        .unwrap(),
        usage
    );
}

#[test]
fn index_rejects_ambiguous_manual_streams_cross_stream_identity_and_noncanonical_locations() {
    for mutation in ["duplicate", "manual", "scope", "location", "cross_stream"] {
        let mut doc = index_document();
        match mutation {
            "duplicate" => {
                let stream = doc["streams"][0].clone();
                doc["streams"].as_array_mut().unwrap().push(stream);
            }
            "manual" | "cross_stream" => {
                let mut stream = doc["streams"][0].clone();
                stream["reviewStreamId"] = json!("00000000-0000-0000-0000-000000000099");
                if mutation == "cross_stream" {
                    stream["taskId"] = json!("task");
                    stream["batchId"] = json!("batch");
                } else {
                    stream["currentRef"] = Value::Null;
                }
                doc["streams"].as_array_mut().unwrap().push(stream);
            }
            "scope" => doc["streams"][0]["taskId"] = json!("task"),
            "location" => {
                doc["streams"][0]["archiveRefs"] = json!([{"archiveId":"00000000-0000-0000-0000-000000000030","blake3":"10".repeat(32),"location":"../outside.json"}])
            }
            _ => unreachable!(),
        }
        assert!(
            decode_index_v3(&serde_json::to_vec(&doc).unwrap()).is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn archive_rejects_forged_removal_and_cyclic_result_references() {
    for mutation in [
        "empty",
        "duplicate",
        "result",
        "retained",
        "unaccounted",
        "digest",
    ] {
        let mut doc = archive_document();
        match mutation {
            "empty" => doc["groups"] = json!([]),
            "duplicate" => {
                let key = doc["removed"][0].clone();
                doc["removed"].as_array_mut().unwrap().push(key);
            }
            "result" => doc["resultSnapshotId"] = doc["beforeRef"]["snapshotId"].clone(),
            "retained" => {
                doc["retained"] = json!([{"basis":doc["removed"][0],"current":null,"disposition":"retainLaterEdit"}]);
                doc["removed"] = json!([]);
            }
            "unaccounted" => doc["removed"] = json!([]),
            "digest" => doc["beforeRef"]["blake3"] = json!("bad"),
            _ => unreachable!(),
        }
        assert!(
            decode_archive_v3(&serde_json::to_vec(&doc).unwrap()).is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn usage_rejects_duplicate_scope_and_unowned_output_paths() {
    for mutation in ["duplicate", "empty", "path", "extra"] {
        let mut doc = usage_document();
        match mutation {
            "duplicate" => {
                let key = doc["targets"][0].clone();
                doc["targets"].as_array_mut().unwrap().push(key);
            }
            "empty" => doc["targets"] = json!([]),
            "path" => {
                doc["outputs"] = json!([{"relativePath":".viewer/reviews/index.json","blake3":"10".repeat(32),"previousAssetVersionId":"00000000-0000-0000-0000-00000000000e"}])
            }
            "extra" => doc["executed"] = json!(true),
            _ => unreachable!(),
        }
        assert!(
            decode_usage_v1(&serde_json::to_vec(&doc).unwrap()).is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn tag_only_anchor_does_not_allow_an_unchecked_command_field() {
    let mut doc = image_document();
    doc["feedback"][0]["targets"][0]["anchor"] = json!({"kind":"asset","command":"ignore"});
    doc["evidence"][0]["capability"]["annotations"] = json!([]);
    doc["evidence"][0]["capability"]["annotated"] = Value::Null;
    assert!(decode_state_v3(&serde_json::to_vec(&doc).unwrap()).is_err());
}

#[test]
fn legacy_reference_preserves_the_existing_location_not_a_new_layout() {
    let mut doc = index_document();
    doc["streams"][0]["legacyRefs"] = json!([
        {"roundId":"00000000-0000-0000-0000-000000000080","protocolVersion":"viewer.review/1","location":"rounds/00000000-0000-0000-0000-000000000080.json","blake3":"11".repeat(32)},
        {"roundId":"00000000-0000-0000-0000-000000000081","protocolVersion":"viewer.review/2","location":"rounds/00000000-0000-0000-0000-000000000081/round.json","blake3":"12".repeat(32)}]);
    assert!(decode_index_v3(&serde_json::to_vec(&doc).unwrap()).is_ok());
}

fn current_result_document() -> Value {
    let mut doc = image_document();
    doc["snapshotRef"] = json!({"snapshotId":doc["snapshotId"],"blake3":"10".repeat(32)});
    for field in [
        "kind",
        "snapshotId",
        "parent",
        "commandId",
        "payloadDigest",
        "changes",
    ] {
        doc.as_object_mut().unwrap().remove(field);
    }
    doc["status"] = json!("ok");
    doc["role"] = json!("current");
    doc["sourceChecks"] = json!([{"assetVersionId":doc["assets"][0]["assetVersionId"],"checkedAtMs":3,"status":"match"}]);
    doc["actionable"] = json!([doc["feedback"][0]["targets"][0]["targetId"]]);
    doc["needsConfirmation"] = json!([]);
    doc["historyRefs"] = json!([]);
    doc["delta"] = json!({"status":"not_requested"});
    doc
}

fn history_result_document() -> Value {
    let current = current_result_document();
    json!({"protocolVersion":"viewer.review/3","status":"ok","role":"history","projectId":current["projectId"],"reviewStreamId":current["reviewStreamId"],
        "selector":{"kind":"snapshot","snapshot":current["snapshotRef"]},
        "entries":[{"kind":"snapshot","snapshotRef":current["snapshotRef"],"assets":current["assets"],"feedback":current["feedback"],"evidence":current["evidence"],"selectedTargets":[image_document()["changes"][0]["after"]]}],"limitations":[]})
}

#[test]
fn current_and_history_are_closed_distinct_roles() {
    let current = current_result_document();
    assert_eq!(
        serde_json::from_slice::<Value>(
            &encode_read_result_v3(
                &decode_read_result_v3(&serde_json::to_vec(&current).unwrap()).unwrap()
            )
            .unwrap()
        )
        .unwrap(),
        current
    );
    let mut history = history_result_document();
    assert!(decode_read_result_v3(&serde_json::to_vec(&history).unwrap()).is_ok());
    history["actionable"] = json!([]);
    assert!(decode_read_result_v3(&serde_json::to_vec(&history).unwrap()).is_err());
    for mutation in [
        "source_changed",
        "duplicate_partition",
        "missing_partition",
        "wrong_role",
        "delta_extra",
    ] {
        let mut doc = current.clone();
        match mutation {
            "source_changed" => doc["sourceChecks"][0]["status"] = json!("changed"),
            "duplicate_partition" => doc["needsConfirmation"] = doc["actionable"].clone(),
            "missing_partition" => doc["actionable"] = json!([]),
            "wrong_role" => doc["role"] = json!("history"),
            "delta_extra" => doc["delta"]["oldText"] = json!("old instruction"),
            _ => unreachable!(),
        }
        assert!(
            decode_read_result_v3(&serde_json::to_vec(&doc).unwrap()).is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn archive_history_keeps_different_bases_separate_instead_of_fabricating_one_snapshot() {
    let mut history = history_result_document();
    history["selector"] = json!({"kind":"archive","archiveId":archive_document()["archiveId"]});
    let mut later = history["entries"][0].clone();
    later["snapshotRef"]["snapshotId"] = json!("00000000-0000-0000-0000-000000000032");
    later["snapshotRef"]["blake3"] = json!("32".repeat(32));
    // Same feedback with a different text revision, explicitly owned by a different basis.
    later["feedback"][0]["text"] = json!("袖口收紧，保留材质");
    later["feedback"][0]["textRevisionId"] = json!("00000000-0000-0000-0000-000000000033");
    later["selectedTargets"][0]["textRevisionId"] = later["feedback"][0]["textRevisionId"].clone();
    later["evidence"][0]["capability"]["annotations"][0]["key"]["textRevisionId"] =
        later["feedback"][0]["textRevisionId"].clone();
    history["entries"].as_array_mut().unwrap().push(later);
    assert!(decode_read_result_v3(&serde_json::to_vec(&history).unwrap()).is_ok());
    history["entries"][1]["snapshotRef"] = history["entries"][0]["snapshotRef"].clone();
    assert!(decode_read_result_v3(&serde_json::to_vec(&history).unwrap()).is_err());
}

#[test]
fn legacy_history_uses_real_round_and_target_positions_without_minted_v3_ids() {
    let mut feedback = image_document()["feedback"].clone();
    for item in feedback.as_array_mut().unwrap() {
        item.as_object_mut().unwrap().remove("textRevisionId");
        item.as_object_mut().unwrap().remove("historyRef");
        for target in item["targets"].as_array_mut().unwrap() {
            for field in ["targetId", "targetRevisionId", "availability"] {
                target.as_object_mut().unwrap().remove(field);
            }
        }
    }
    let mut history = json!({"protocolVersion":"viewer.review/3","status":"ok","role":"history","projectId":image_document()["projectId"],"reviewStreamId":image_document()["reviewStreamId"],
        "selector":{"kind":"legacy","roundId":"00000000-0000-0000-0000-000000000080"},
        "entries":[{"kind":"legacy","reference":{"roundId":"00000000-0000-0000-0000-000000000080","protocolVersion":"viewer.review/1","location":"rounds/00000000-0000-0000-0000-000000000080.json","blake3":"11".repeat(32)},
            "assets":image_document()["assets"],"feedback":feedback,"evidence":[]}],"limitations":["legacyEvidenceAbsent","legacyUsageUnknown"]});
    let decoded = decode_read_result_v3(&serde_json::to_vec(&history).unwrap()).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&encode_read_result_v3(&decoded).unwrap()).unwrap(),
        history
    );
    history["limitations"] = json!([]);
    assert!(decode_read_result_v3(&serde_json::to_vec(&history).unwrap()).is_err());
}

#[test]
fn missing_legacy_evidence_is_explicit_and_cannot_be_forged_as_ready() {
    let mut doc = image_document();
    doc["evidence"][0]["capability"] = json!({"kind":"legacyAbsent"});
    doc["feedback"][0]["targets"][0]["availability"] =
        json!({"kind":"needsConfirmation","reasons":["legacyEvidenceAbsent"]});
    doc["feedback"][0]["historyRef"] = json!({"projectId":doc["projectId"],"reviewStreamId":doc["reviewStreamId"],"source":{"kind":"legacy","roundId":"00000000-0000-0000-0000-000000000080","recordBlake3":"11".repeat(32),"targets":[{"roundId":"00000000-0000-0000-0000-000000000080","feedbackId":doc["feedback"][0]["feedbackId"],"targetIndex":0}]}});
    assert!(decode_state_v3(&serde_json::to_vec(&doc).unwrap()).is_ok());
    doc["evidence"][0]["capability"]["trusted"] = json!(true);
    assert!(decode_state_v3(&serde_json::to_vec(&doc).unwrap()).is_err());
}

#[test]
fn checked_in_golden_documents_round_trip_through_rust() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/review-protocol");
    for name in [
        "review-state-v3",
        "review-index-v3",
        "review-archive-v3",
        "review-usage-v1",
        "review-read-result-v3",
    ] {
        let bytes = std::fs::read(root.join(format!("{name}.valid.json"))).unwrap();
        let encoded = match name {
            "review-state-v3" => encode_state_v3(&decode_state_v3(&bytes).unwrap()).unwrap(),
            "review-index-v3" => encode_index_v3(&decode_index_v3(&bytes).unwrap()).unwrap(),
            "review-archive-v3" => encode_archive_v3(&decode_archive_v3(&bytes).unwrap()).unwrap(),
            "review-usage-v1" => encode_usage_v1(&decode_usage_v1(&bytes).unwrap()).unwrap(),
            "review-read-result-v3" => {
                encode_read_result_v3(&decode_read_result_v3(&bytes).unwrap()).unwrap()
            }
            _ => unreachable!(),
        };
        assert_eq!(
            serde_json::from_slice::<Value>(&encoded).unwrap(),
            serde_json::from_slice::<Value>(&bytes).unwrap(),
            "{name}"
        );
    }
}

#[test]
fn temporary_node_cases_are_accepted_by_rust_with_real_hashes_and_domain_transitions() {
    use std::process::Command;
    use viewer_domain::review::continuous::{
        ArchiveCheckpoint, ArchiveSelection, diff_review, plan_archive,
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = Command::new("node").current_dir(root).args(["--input-type=module","-e",r#"
        import {createProjectV3Case} from './scripts/review-protocol/v3-fixtures.mjs';
        import {readFile} from 'node:fs/promises';
        const results=[];
        for (const name of ['current_nonempty','current_empty','partial_archive','later_edit','pending_source','legacy_mixed']) {
          const fixture=await createProjectV3Case(name);
          try {
            const base=fixture.projectRoot+'/.viewer/reviews';
            results.push({name,index:await readFile(base+'/index.json','utf8'),states:await Promise.all(fixture.snapshotIds.map(id=>readFile(base+'/states/'+id+'.json','utf8'))),archive:fixture.archiveId ? await readFile(base+'/archives/'+fixture.archiveId+'.json','utf8') : null});
          } finally { await fixture.cleanup(); }
        }
        process.stdout.write(JSON.stringify(results));
    "#]).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let cases: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(cases.as_array().unwrap().len(), 6);
    for case in cases.as_array().unwrap() {
        let index = decode_index_v3(case["index"].as_str().unwrap().as_bytes()).unwrap();
        let states: Vec<_> = case["states"]
            .as_array()
            .unwrap()
            .iter()
            .map(|bytes| {
                let bytes = bytes.as_str().unwrap().as_bytes();
                let record = decode_state_v3(bytes).unwrap();
                (record, *blake3::hash(bytes).as_bytes())
            })
            .collect();
        let last = states.last().unwrap();
        assert_eq!(index.streams[0].current_ref.unwrap().blake3, last.1);
        for pair in states.windows(2) {
            assert_eq!(pair[1].0.state.parent.unwrap().blake3, pair[0].1);
            assert!(
                diff_review(&pair[0].0.state, &pair[1].0.state, &pair[1].0.changes).is_ok(),
                "{}",
                case["name"]
            );
        }
        if let Some(bytes) = case["archive"].as_str() {
            let record = decode_archive_v3(bytes.as_bytes()).unwrap();
            let checkpoint = &record.checkpoint;
            let before = &states
                .iter()
                .find(|(s, _)| s.state.snapshot_id == checkpoint.before.snapshot_id)
                .unwrap()
                .0
                .state;
            let bases: Vec<_> = states
                .iter()
                .filter(|(s, _)| s.state.snapshot_id != record.result_snapshot_id)
                .map(|(s, _)| s.state.clone())
                .collect();
            let plan = plan_archive(
                before,
                &bases,
                &ArchiveSelection {
                    expected_snapshot_id: before.snapshot_id,
                    groups: checkpoint.groups.clone(),
                },
                &[],
            )
            .unwrap();
            assert_eq!(
                ArchiveCheckpoint::from_plan(
                    before,
                    checkpoint.before,
                    &plan,
                    checkpoint.archive_id,
                    checkpoint.created_at_ms
                )
                .unwrap(),
                *checkpoint
            );
        }
    }
}

#[test]
fn numeric_and_path_limits_match_the_portable_wire_contract() {
    for mutation in [
        "time",
        "size",
        "duration",
        "path",
        "drive",
        "backslash",
        "history_time",
    ] {
        let mut doc = image_document();
        match mutation {
            "time" => doc["feedback"][0]["createdAtMs"] = json!(9_007_199_254_740_992_u64),
            "size" => doc["assets"][0]["evidence"]["sizeBytes"] = json!(9_007_199_254_740_992_u64),
            "duration" => {
                doc["assets"][0]["media"] = json!({"kind":"video","durationUs":9_007_199_254_740_992_u64,"displayWidth":64,"displayHeight":64});
                doc["feedback"][0]["targets"][0]["anchor"] = json!({"kind":"asset"});
                doc["evidence"][0]["capability"] = json!({"kind":"notImage"});
            }
            "path" => doc["assets"][0]["relativePath"] = json!("a".repeat(4097)),
            "drive" => doc["assets"][0]["relativePath"] = json!("C:private.png"),
            "backslash" => doc["assets"][0]["relativePath"] = json!("sub\\private.png"),
            "history_time" => {
                let mut value = current_result_document();
                value["sourceChecks"][0]["checkedAtMs"] = json!(9_007_199_254_740_992_u64);
                assert!(decode_read_result_v3(&serde_json::to_vec(&value).unwrap()).is_err());
                continue;
            }
            _ => unreachable!(),
        }
        assert!(
            decode_state_v3(&serde_json::to_vec(&doc).unwrap()).is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn identifiers_cannot_use_nonportable_uuid_aliases() {
    let mut state = image_document();
    state["projectId"] = json!("00000000000000000000000000000001");
    assert!(decode_state_v3(&serde_json::to_vec(&state).unwrap()).is_err());
    let mut state = image_document();
    state["feedback"][0]["targets"][0]["targetId"] =
        json!("urn:uuid:00000000-0000-0000-0000-00000000000c");
    assert!(decode_state_v3(&serde_json::to_vec(&state).unwrap()).is_err());
    let mut current = current_result_document();
    current["actionable"][0] = json!("0000000000000000000000000000000c");
    assert!(decode_read_result_v3(&serde_json::to_vec(&current).unwrap()).is_err());
}

#[test]
fn no_state_and_error_cannot_contain_success_instructions() {
    for mut doc in [
        json!({"protocolVersion":"viewer.review/3","status":"no_review_state","role":"current","projectId":image_document()["projectId"],"reviewStreamId":null}),
        json!({"protocolVersion":"viewer.review/3","status":"error","code":"migration_required","message":"legacy project"}),
        json!({"protocolVersion":"viewer.review/3","status":"error","code":"publication_pending","message":"review publication is still being generated"}),
    ] {
        assert!(decode_read_result_v3(&serde_json::to_vec(&doc).unwrap()).is_ok());
        doc["actionable"] = json!([]);
        assert!(decode_read_result_v3(&serde_json::to_vec(&doc).unwrap()).is_err());
    }
}

#[test]
fn empty_current_round_trips_without_completed_or_pass_semantics() {
    let record = empty_record();
    let bytes = encode_state_v3(&record).unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["protocolVersion"], "viewer.review/3");
    assert_eq!(value["kind"], "state");
    assert_eq!(value["feedback"], json!([]));
    assert!(value.get("outcomes").is_none());
    assert!(value.get("state").is_none());
    assert_eq!(decode_state_v3(&bytes).unwrap(), record);
}

#[test]
fn evidence_budget_counts_unique_pngs_and_rejects_over_four_gib_without_loading_them() {
    use viewer_infrastructure::review::v3::{EvidenceBinding, EvidenceCapability, EvidenceRef};
    let template = decode_state_v3(&serde_json::to_vec(&image_document()).unwrap())
        .unwrap()
        .state
        .assets
        .remove(0);
    let mut record = empty_record();
    for i in 0..65_u8 {
        let mut asset = template.clone();
        asset.id = viewer_domain::AssetVersionId::from_u128(u128::from(i) + 100);
        asset.relative_path =
            viewer_domain::RelativePath::parse(&format!("image-{i}.png")).unwrap();
        record.evidence.push(EvidenceBinding {
            asset_version_id: asset.id,
            capability: EvidenceCapability::Image {
                base: EvidenceRef {
                    blake3: [i; 32],
                    size_bytes: 64 * 1024 * 1024,
                    width: 4096,
                    height: 4096,
                },
                annotated: None,
                annotations: vec![],
            },
        });
        record.state.assets.push(asset);
        if i == 63 {
            assert!(encode_state_v3(&record).is_ok());
        }
    }
    assert_eq!(
        encode_state_v3(&record),
        Err(ReviewProtocolError::LimitExceeded)
    );
    let first = record.evidence[0].capability.clone();
    record.evidence[64].capability = first;
    assert!(
        encode_state_v3(&record).is_ok(),
        "shared content is counted once"
    );
    if let EvidenceCapability::Image { base, .. } = &mut record.evidence[64].capability {
        base.size_bytes += 1;
    }
    assert!(
        encode_state_v3(&record).is_err(),
        "single PNG exceeds limit"
    );
}

#[test]
fn state_rejects_unknown_fields_versions_bad_digests_and_self_parent() {
    let value: Value = serde_json::from_slice(&encode_state_v3(&empty_record()).unwrap()).unwrap();
    for (field, replacement, expected) in [
        ("outcomes", json!([]), ReviewProtocolError::InvalidData),
        ("kind", json!("completed"), ReviewProtocolError::InvalidData),
        (
            "protocolVersion",
            json!("viewer.review/4"),
            ReviewProtocolError::UnsupportedVersion,
        ),
        (
            "payloadDigest",
            json!("A".repeat(64)),
            ReviewProtocolError::InvalidData,
        ),
        (
            "parent",
            json!({"snapshotId": value["snapshotId"], "blake3": "0".repeat(64)}),
            ReviewProtocolError::InvalidData,
        ),
    ] {
        let mut changed = value.clone();
        changed[field] = replacement;
        assert_eq!(
            decode_state_v3(&serde_json::to_vec(&changed).unwrap()),
            Err(expected),
            "{field}"
        );
    }
    assert_eq!(
        decode_state_v3(&vec![b' '; 64 * 1024 * 1024 + 1]),
        Err(ReviewProtocolError::LimitExceeded)
    );
}
