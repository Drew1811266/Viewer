mod compose;
mod validate;

pub use compose::{
    ComposedShot, EffectiveReference, PromptSegment, ReferenceOrigin, SegmentSource, compose_plan,
};
pub use validate::{
    AssetIndexView, MAX_BRIEF_CHARS, MAX_DECLARED_COUNT, MAX_DIMENSIONS, MAX_GROUPS,
    MAX_MODEL_PARAMS, MAX_REFERENCES_PER_SHOT, MAX_RULES, MAX_SHOTS, MAX_TEXT_CHARS,
    MAX_VALUES_PER_DIMENSION, PlanValidation, PlanWarning, PlanWarningCode, validate_plan,
};
