use serde::Serialize;
use viewer_application::plan::{PlanDiff, PlanWarning};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanGroupDto {
    pub id: String,
    pub label: String,
    pub declared_count: u32,
    pub actual_count: u32,
    pub brief: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanShotDto {
    pub shot_id: String,
    pub group_id: String,
    pub prompt: String,
    pub hash: String,
    pub references: Vec<String>,
    pub generation: viewer_application::plan::ShotGeneration,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanSnapshotDto {
    pub plan_id: String,
    pub revision: u32,
    pub blake3: String,
    pub groups: Vec<PlanGroupDto>,
    pub shots: Vec<PlanShotDto>,
    pub diff: Option<PlanDiff>,
    pub warnings: Vec<PlanWarning>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanMarksDto {
    pub plan_id: String,
    pub plan_available: bool,
    pub plan_revision: Option<u32>,
    pub marks: Vec<viewer_domain::plan::ResolvedMark>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanSummaryDto {
    pub plan_id: String,
    pub revision: u32,
    pub ignored: usize,
}
