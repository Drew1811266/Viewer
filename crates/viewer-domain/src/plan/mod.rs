mod document;
mod error;
mod ids;
mod run;

pub use document::{
    AssetRole, Dimension, DimensionValue, ModelRef, PlanDocument, PlanGroup, PlanRule, PlanShot,
    Producer, Relocation, RelocationKind, ShotReference, ShotSpec, parse_plan,
};
pub use error::{PlanError, PlanErrorCode};
pub use ids::{AssetId, PlanId, Revision};
pub use run::{RUN_PROTOCOL_V1, RunRecord, RunShot};

pub const PLAN_PROTOCOL_V1: &str = "viewer.plan/1";
pub const MAX_PLAN_BYTES: u64 = 8 * 1024 * 1024;
