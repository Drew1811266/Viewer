use serde_json::{Value, json};
use viewer_infrastructure::review::v3::decode_state_v3;
use viewer_infrastructure::review::v4::decode_state_v4;
use viewer_infrastructure::review::v5::{decode_state_v5, encode_state_v5};

fn id(value: u128) -> String {
    format!("00000000-0000-0000-0000-{value:012x}")
}

fn key(feedback: u128, text: u128, target: u128, target_revision: u128) -> Value {
    json!({
        "feedbackId": id(feedback),
        "textRevisionId": id(text),
        "targetId": id(target),
        "targetRevisionId": id(target_revision)
    })
}

fn styled_document() -> Value {
    let text_key = key(10, 11, 12, 13);
    let line_key = key(10, 11, 14, 15);
    let polygon_key = key(10, 11, 16, 17);
    json!({
        "protocolVersion": "viewer.review/5",
        "kind": "state",
        "projectId": id(1),
        "reviewStreamId": id(2),
        "snapshotId": id(3),
        "parent": null,
        "commandId": id(4),
        "payloadDigest": "05".repeat(32),
        "assets": [{
            "assetVersionId": id(20),
            "sourceEntityId": null,
            "relativePath": "image.png",
            "evidence": {"sizeBytes": 100, "modifiedNs": "1", "blake3": "01".repeat(32)},
            "media": {"kind": "image", "width": 64, "height": 64},
            "producerAssetId": null,
            "parentAssetVersionId": null
        }],
        "feedback": [{
            "feedbackId": id(10),
            "textRevisionId": id(11),
            "text": "这里需要再亮一点",
            "createdAtMs": 1,
            "historyRef": null,
            "targets": [
                {
                    "targetId": id(12),
                    "targetRevisionId": id(13),
                    "assetVersionId": id(20),
                    "anchor": {"kind": "imageText", "x": 0.2, "y": 0.3},
                    "style": {
                        "tool": "text",
                        "color": "yellow",
                        "width": "medium",
                        "opacityPercent": 100,
                        "dashed": false,
                        "textSize": "medium"
                    },
                    "availability": {"kind": "ready"}
                },
                {
                    "targetId": id(14),
                    "targetRevisionId": id(15),
                    "assetVersionId": id(20),
                    "anchor": {
                        "kind": "imageLine",
                        "tail": {"x": 0.1, "y": 0.1},
                        "head": {"x": 0.9, "y": 0.1}
                    },
                    "style": {
                        "tool": "ruler",
                        "color": "red",
                        "width": "thin",
                        "opacityPercent": 100,
                        "dashed": false
                    },
                    "availability": {"kind": "ready"}
                },
                {
                    "targetId": id(16),
                    "targetRevisionId": id(17),
                    "assetVersionId": id(20),
                    "anchor": {
                        "kind": "imagePolygon",
                        "points": [
                            {"x": 0.2, "y": 0.3},
                            {"x": 0.6, "y": 0.3},
                            {"x": 0.4, "y": 0.7}
                        ]
                    },
                    "style": {
                        "tool": "polygon",
                        "color": "blue",
                        "width": "medium",
                        "opacityPercent": 60,
                        "dashed": true
                    },
                    "availability": {"kind": "ready"}
                }
            ]
        }],
        "changes": [
            {
                "targetId": id(12),
                "before": null,
                "after": text_key,
                "kind": "added",
                "archiveId": null,
                "historicalKey": null
            },
            {
                "targetId": id(14),
                "before": null,
                "after": line_key,
                "kind": "added",
                "archiveId": null,
                "historicalKey": null
            },
            {
                "targetId": id(16),
                "before": null,
                "after": polygon_key,
                "kind": "added",
                "archiveId": null,
                "historicalKey": null
            }
        ],
        "evidence": [{
            "assetVersionId": id(20),
            "capability": {
                "kind": "image",
                "base": {
                    "blake3": "02".repeat(32),
                    "sizeBytes": 100,
                    "width": 64,
                    "height": 64
                },
                "annotated": {
                    "blake3": "03".repeat(32),
                    "sizeBytes": 150,
                    "width": 64,
                    "height": 64
                },
                "annotations": [
                    {"ordinal": 1, "key": text_key},
                    {"ordinal": 2, "key": line_key},
                    {"ordinal": 3, "key": polygon_key}
                ]
            }
        }]
    })
}

#[test]
fn v5_round_trips_styled_text_line_and_polygon_targets() {
    let document = styled_document();
    let decoded = decode_state_v5(&serde_json::to_vec(&document).unwrap()).unwrap();
    let targets = &decoded.state.feedback[0].targets;
    assert_eq!(targets.len(), 3);
    assert_eq!(targets[0].anchor.kind_name(), "imageText");
    assert_eq!(targets[1].anchor.kind_name(), "imageLine");
    assert_eq!(targets[2].anchor.kind_name(), "imagePolygon");
    assert!(targets.iter().all(|target| target.style.is_some()));
    assert_eq!(
        serde_json::from_slice::<Value>(&encode_state_v5(&decoded).unwrap()).unwrap(),
        document
    );
}

#[test]
fn v3_rejects_styled_targets_and_new_anchor_kinds() {
    let mut document = styled_document();
    document["protocolVersion"] = json!("viewer.review/3");
    assert!(decode_state_v3(&serde_json::to_vec(&document).unwrap()).is_err());

    document["protocolVersion"] = json!("viewer.review/4");
    assert!(decode_state_v4(&serde_json::to_vec(&document).unwrap()).is_err());

    let mut unstyled = styled_document();
    for target in unstyled["feedback"][0]["targets"].as_array_mut().unwrap() {
        target.as_object_mut().unwrap().remove("style");
    }
    assert!(decode_state_v3(&serde_json::to_vec(&unstyled).unwrap()).is_err());
}
