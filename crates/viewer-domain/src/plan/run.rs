use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::{AssetId, ModelRef, PlanId, Producer, Revision};

pub const RUN_PROTOCOL_V1: &str = "viewer.plan.run/1";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunShot {
    pub shot_id: String,
    pub shot_hash: String,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub reference_ids: Vec<AssetId>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunRecord {
    pub protocol_version: String,
    pub run_id: String,
    pub plan_id: PlanId,
    pub plan_revision: Revision,
    pub created_at: String,
    pub producer: Producer,
    #[serde(default)]
    pub model: Option<ModelRef>,
    #[serde(default)]
    pub params: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub shots: Vec<RunShot>,
}

impl RunRecord {
    pub fn parse(bytes: &[u8]) -> Result<Self, super::PlanError> {
        let record: Self = serde_json::from_slice(bytes).map_err(|error| {
            super::PlanError::new("", super::PlanErrorCode::InvalidSchema, error.to_string())
        })?;
        if record.protocol_version != RUN_PROTOCOL_V1 {
            return Err(super::PlanError::new(
                "protocolVersion",
                super::PlanErrorCode::UnsupportedVersion,
                format!("unsupported run protocol {}", record.protocol_version),
            ));
        }
        Ok(record)
    }
}
