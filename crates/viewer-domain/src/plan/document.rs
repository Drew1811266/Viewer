use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::{
    PLAN_PROTOCOL_V1,
    error::{PlanError, PlanErrorCode},
    ids::{AssetId, PlanId, Revision},
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetRole {
    Style,
    Subject,
    Composition,
    Other,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RelocationKind {
    Group,
    Shot,
    Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Producer {
    pub name: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DimensionValue {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub fragment: Option<String>,
    #[serde(default)]
    pub references: Vec<AssetId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Dimension {
    pub id: String,
    pub label: String,
    pub values: Vec<DimensionValue>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanRule {
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanGroup {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
    pub declared_count: u32,
    pub brief: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShotReference {
    pub asset_id: AssetId,
    #[serde(default)]
    pub role: Option<AssetRole>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShotSpec {
    #[serde(default)]
    pub aspect: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub seed: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelRef {
    pub id: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub params: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanShot {
    pub id: String,
    pub group_id: String,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
    #[serde(default)]
    pub delta: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub references: Vec<ShotReference>,
    #[serde(default)]
    pub spec: Option<ShotSpec>,
    #[serde(default)]
    pub model: Option<ModelRef>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Relocation {
    pub kind: RelocationKind,
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanDocument {
    pub protocol_version: String,
    pub plan_id: PlanId,
    pub revision: Revision,
    #[serde(default)]
    pub title: Option<String>,
    pub created_at: String,
    pub producer: Producer,
    #[serde(default)]
    pub dimensions: Vec<Dimension>,
    #[serde(default)]
    pub rules: Vec<PlanRule>,
    #[serde(default)]
    pub groups: Vec<PlanGroup>,
    #[serde(default)]
    pub shots: Vec<PlanShot>,
    #[serde(default)]
    pub relocations: Vec<Relocation>,
}

#[derive(Deserialize)]
struct ProtocolProbe {
    #[serde(rename = "protocolVersion")]
    protocol_version: String,
}

pub fn parse_plan(bytes: &[u8], max_bytes: u64) -> Result<PlanDocument, PlanError> {
    if bytes.len() as u64 > max_bytes {
        return Err(PlanError::new(
            "",
            PlanErrorCode::TooLarge,
            format!("plan document exceeds {max_bytes} bytes"),
        ));
    }
    let probe: ProtocolProbe = serde_json::from_slice(bytes)
        .map_err(|error| PlanError::new("", PlanErrorCode::InvalidSchema, error.to_string()))?;
    if probe.protocol_version != PLAN_PROTOCOL_V1 {
        return Err(PlanError::new(
            "protocolVersion",
            PlanErrorCode::UnsupportedVersion,
            format!(
                "viewer does not support plan protocol {}",
                probe.protocol_version
            ),
        ));
    }
    serde_json::from_slice(bytes)
        .map_err(|error| PlanError::new("", PlanErrorCode::InvalidSchema, error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::MAX_PLAN_BYTES;

    const MINIMAL: &str = r#"{
      "protocolVersion": "viewer.plan/1",
      "planId": "autumn-hero",
      "revision": 1,
      "createdAt": "2026-11-03T00:00:00Z",
      "producer": { "name": "deepseek-harness" },
      "groups": [{ "id": "g1", "label": "夏", "declaredCount": 1, "brief": "轻薄夏装" }],
      "shots": [{ "id": "s1", "groupId": "g1", "delta": "海边逆光" }]
    }"#;

    #[test]
    fn parses_a_minimal_plan() {
        let plan = parse_plan(MINIMAL.as_bytes(), MAX_PLAN_BYTES).unwrap();
        assert_eq!(plan.plan_id.as_str(), "autumn-hero");
        assert_eq!(plan.revision.get(), 1);
        assert_eq!(plan.groups.len(), 1);
        assert_eq!(plan.shots[0].group_id, "g1");
        assert_eq!(plan.shots[0].delta.as_deref(), Some("海边逆光"));
    }

    #[test]
    fn rejects_unknown_fields() {
        let json = MINIMAL.replace("\"shots\"", "\"surprise\": true,\n      \"shots\"");
        let error = parse_plan(json.as_bytes(), MAX_PLAN_BYTES).unwrap_err();
        assert_eq!(error.code, PlanErrorCode::InvalidSchema);
    }

    #[test]
    fn rejects_newer_protocol_version() {
        let json = MINIMAL.replace("viewer.plan/1", "viewer.plan/2");
        let error = parse_plan(json.as_bytes(), MAX_PLAN_BYTES).unwrap_err();
        assert_eq!(error.code, PlanErrorCode::UnsupportedVersion);
    }

    #[test]
    fn rejects_oversized_documents() {
        let error = parse_plan(MINIMAL.as_bytes(), 16).unwrap_err();
        assert_eq!(error.code, PlanErrorCode::TooLarge);
    }

    #[test]
    fn round_trips_asset_ids_on_shots_and_values() {
        let asset = "9f2c".to_owned() + &"ab".repeat(30);
        let json = MINIMAL.replace(
            "\"delta\": \"海边逆光\"",
            &format!(
                "\"delta\": \"海边逆光\", \"references\": [{{ \"assetId\": \"{asset}\", \"role\": \"style\" }}]"
            ),
        );
        let plan = parse_plan(json.as_bytes(), MAX_PLAN_BYTES).unwrap();
        assert_eq!(plan.shots[0].references[0].asset_id.as_hex(), asset);
        assert_eq!(plan.shots[0].references[0].role, Some(AssetRole::Style));

        let encoded = serde_json::to_string(&plan).unwrap();
        let reparsed = parse_plan(encoded.as_bytes(), MAX_PLAN_BYTES).unwrap();
        assert_eq!(reparsed, plan);
    }
}
