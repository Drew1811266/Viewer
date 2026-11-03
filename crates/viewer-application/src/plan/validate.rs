use std::collections::{BTreeMap, HashMap, HashSet};
use viewer_domain::plan::{AssetId, PlanDocument, PlanError, PlanErrorCode};

pub const MAX_GROUPS: usize = 50;
pub const MAX_SHOTS: usize = 500;
pub const MAX_DIMENSIONS: usize = 20;
pub const MAX_VALUES_PER_DIMENSION: usize = 50;
pub const MAX_RULES: usize = 50;
pub const MAX_REFERENCES_PER_SHOT: usize = 20;
pub const MAX_TEXT_CHARS: usize = 2000;
pub const MAX_BRIEF_CHARS: usize = 1000;

pub trait AssetIndexView {
    fn contains(&self, asset_id: &AssetId) -> bool;
}

impl AssetIndexView for HashSet<AssetId> {
    fn contains(&self, asset_id: &AssetId) -> bool {
        HashSet::contains(self, asset_id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanWarningCode {
    CountMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanWarning {
    pub path: String,
    pub code: PlanWarningCode,
    pub message: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanValidation {
    pub errors: Vec<PlanError>,
    pub warnings: Vec<PlanWarning>,
}

impl PlanValidation {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn validate_plan(plan: &PlanDocument, assets: &dyn AssetIndexView) -> PlanValidation {
    let mut validation = PlanValidation::default();

    macro_rules! fail {
        ($path:expr, $code:expr, $message:expr) => {
            validation
                .errors
                .push(PlanError::new($path, $code, $message))
        };
    }

    if plan.groups.is_empty() {
        fail!(
            "groups",
            PlanErrorCode::InvalidSchema,
            "a plan must declare at least one group"
        );
    } else if plan.groups.len() > MAX_GROUPS {
        fail!(
            "groups",
            PlanErrorCode::InvalidSchema,
            format!("a plan may declare at most {MAX_GROUPS} groups")
        );
    }
    if plan.shots.is_empty() {
        fail!(
            "shots",
            PlanErrorCode::InvalidSchema,
            "a plan must declare at least one shot"
        );
    } else if plan.shots.len() > MAX_SHOTS {
        fail!(
            "shots",
            PlanErrorCode::InvalidSchema,
            format!("a plan may declare at most {MAX_SHOTS} shots")
        );
    }
    if plan.dimensions.len() > MAX_DIMENSIONS {
        fail!(
            "dimensions",
            PlanErrorCode::InvalidSchema,
            format!("a plan may declare at most {MAX_DIMENSIONS} dimensions")
        );
    }
    if plan.rules.len() > MAX_RULES {
        fail!(
            "rules",
            PlanErrorCode::InvalidSchema,
            format!("a plan may declare at most {MAX_RULES} rules")
        );
    }

    let mut dimension_ids: HashSet<&str> = HashSet::new();
    let mut declared_values: HashMap<&str, HashSet<&str>> = HashMap::new();
    for (index, dimension) in plan.dimensions.iter().enumerate() {
        if !dimension_ids.insert(dimension.id.as_str()) {
            fail!(
                format!("dimensions[{index}].id"),
                PlanErrorCode::DuplicateId,
                "dimension id is declared more than once"
            );
        }
        if dimension.values.len() > MAX_VALUES_PER_DIMENSION {
            fail!(
                format!("dimensions[{index}].values"),
                PlanErrorCode::InvalidSchema,
                format!("a dimension may declare at most {MAX_VALUES_PER_DIMENSION} values")
            );
        }
        let mut values = HashSet::new();
        for (value_index, value) in dimension.values.iter().enumerate() {
            if !values.insert(value.id.as_str()) {
                fail!(
                    format!("dimensions[{index}].values[{value_index}].id"),
                    PlanErrorCode::DuplicateId,
                    "value id is declared more than once in this dimension"
                );
            }
            if value
                .fragment
                .as_ref()
                .is_some_and(|fragment| fragment.chars().count() > MAX_TEXT_CHARS)
            {
                fail!(
                    format!("dimensions[{index}].values[{value_index}].fragment"),
                    PlanErrorCode::InvalidSchema,
                    format!("fragment must be at most {MAX_TEXT_CHARS} characters")
                );
            }
            for (asset_index, asset) in value.references.iter().enumerate() {
                if !assets.contains(asset) {
                    fail!(
                        format!(
                            "dimensions[{index}].values[{value_index}].references[{asset_index}]"
                        ),
                        PlanErrorCode::AssetMissing,
                        "asset is not present in the reference store"
                    );
                }
            }
        }
        declared_values.insert(dimension.id.as_str(), values);
    }

    let mut rule_ids: HashSet<&str> = HashSet::new();
    for (index, rule) in plan.rules.iter().enumerate() {
        if !rule_ids.insert(rule.id.as_str()) {
            fail!(
                format!("rules[{index}].id"),
                PlanErrorCode::DuplicateId,
                "rule id is declared more than once"
            );
        }
        if rule.text.chars().count() > MAX_TEXT_CHARS {
            fail!(
                format!("rules[{index}].text"),
                PlanErrorCode::InvalidSchema,
                format!("rule text must be at most {MAX_TEXT_CHARS} characters")
            );
        }
    }

    let mut node_ids: HashSet<&str> = HashSet::new();
    for (index, group) in plan.groups.iter().enumerate() {
        if !node_ids.insert(group.id.as_str()) {
            fail!(
                format!("groups[{index}].id"),
                PlanErrorCode::DuplicateId,
                "id is already used by another group or shot"
            );
        }
        if group.brief.chars().count() > MAX_BRIEF_CHARS {
            fail!(
                format!("groups[{index}].brief"),
                PlanErrorCode::InvalidSchema,
                format!("group brief must be at most {MAX_BRIEF_CHARS} characters")
            );
        }
        validate_attributes(
            &mut validation,
            &declared_values,
            &group.attributes,
            &format!("groups[{index}].attributes"),
        );
    }

    for (index, shot) in plan.shots.iter().enumerate() {
        if !node_ids.insert(shot.id.as_str()) {
            fail!(
                format!("shots[{index}].id"),
                PlanErrorCode::DuplicateId,
                "id is already used by another group or shot"
            );
        }
        if !plan.groups.iter().any(|group| group.id == shot.group_id) {
            fail!(
                format!("shots[{index}].groupId"),
                PlanErrorCode::UnknownGroup,
                "shot references a group that the plan does not declare"
            );
        }
        validate_attributes(
            &mut validation,
            &declared_values,
            &shot.attributes,
            &format!("shots[{index}].attributes"),
        );
        if shot.references.len() > MAX_REFERENCES_PER_SHOT {
            fail!(
                format!("shots[{index}].references"),
                PlanErrorCode::InvalidSchema,
                format!("a shot may reference at most {MAX_REFERENCES_PER_SHOT} assets")
            );
        }
        for (asset_index, reference) in shot.references.iter().enumerate() {
            if !assets.contains(&reference.asset_id) {
                fail!(
                    format!("shots[{index}].references[{asset_index}]"),
                    PlanErrorCode::AssetMissing,
                    "asset is not present in the reference store"
                );
            }
        }
        if shot
            .delta
            .as_ref()
            .is_some_and(|delta| delta.chars().count() > MAX_TEXT_CHARS)
        {
            fail!(
                format!("shots[{index}].delta"),
                PlanErrorCode::InvalidSchema,
                format!("shot delta must be at most {MAX_TEXT_CHARS} characters")
            );
        }
    }

    let mut actual_counts: HashMap<&str, u32> = HashMap::new();
    for shot in &plan.shots {
        *actual_counts.entry(shot.group_id.as_str()).or_default() += 1;
    }
    for (index, group) in plan.groups.iter().enumerate() {
        let actual = actual_counts.get(group.id.as_str()).copied().unwrap_or(0);
        if actual != group.declared_count {
            validation.warnings.push(PlanWarning {
                path: format!("groups[{index}].declaredCount"),
                code: PlanWarningCode::CountMismatch,
                message: format!(
                    "group declares {} shots but the plan contains {actual}",
                    group.declared_count
                ),
            });
        }
    }

    let mut relocated_from: HashSet<&str> = HashSet::new();
    for (index, relocation) in plan.relocations.iter().enumerate() {
        if relocation.from == relocation.to {
            fail!(
                format!("relocations[{index}]"),
                PlanErrorCode::RelocationConflict,
                "a relocation must rename a node to a different id"
            );
        }
        if !relocated_from.insert(relocation.from.as_str()) {
            fail!(
                format!("relocations[{index}].from"),
                PlanErrorCode::RelocationConflict,
                "the same source id is relocated more than once"
            );
        }
    }

    validation
}

fn validate_attributes(
    validation: &mut PlanValidation,
    declared_values: &HashMap<&str, HashSet<&str>>,
    attributes: &BTreeMap<String, String>,
    prefix: &str,
) {
    for (dimension, value) in attributes {
        let path = format!("{prefix}.{dimension}");
        match declared_values.get(dimension.as_str()) {
            None => validation.errors.push(PlanError::new(
                path,
                PlanErrorCode::UnknownValue,
                format!("dimension `{dimension}` is not declared by this plan"),
            )),
            Some(allowed) => {
                if !allowed.contains(value.as_str()) {
                    validation.errors.push(PlanError::new(
                        path,
                        PlanErrorCode::UnknownValue,
                        format!("value `{value}` is not declared for dimension `{dimension}`"),
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use viewer_domain::plan::{
        Dimension, DimensionValue, PlanGroup, PlanId, PlanShot, Producer, Revision,
    };

    fn group(id: &str, declared: u32) -> PlanGroup {
        PlanGroup {
            id: id.to_owned(),
            label: id.to_owned(),
            attributes: BTreeMap::new(),
            declared_count: declared,
            brief: "brief".to_owned(),
        }
    }

    fn shot(id: &str, group_id: &str) -> PlanShot {
        PlanShot {
            id: id.to_owned(),
            group_id: group_id.to_owned(),
            attributes: BTreeMap::new(),
            delta: None,
            prompt: None,
            references: Vec::new(),
            spec: None,
            model: None,
        }
    }

    fn plan(groups: Vec<PlanGroup>, shots: Vec<PlanShot>) -> PlanDocument {
        PlanDocument {
            protocol_version: "viewer.plan/1".to_owned(),
            plan_id: PlanId::parse("autumn").unwrap(),
            revision: Revision::new(1),
            title: None,
            created_at: "2026-11-03T00:00:00Z".to_owned(),
            producer: Producer {
                name: "harness".to_owned(),
                session_id: None,
            },
            dimensions: Vec::new(),
            rules: Vec::new(),
            groups,
            shots,
            relocations: Vec::new(),
        }
    }

    #[test]
    fn declared_count_mismatch_is_a_warning_not_an_error() {
        let document = plan(
            vec![group("g1", 3)],
            vec![shot("s1", "g1"), shot("s2", "g1")],
        );
        let validation = validate_plan(&document, &HashSet::new());
        assert!(validation.errors.is_empty(), "{:?}", validation.errors);
        assert_eq!(validation.warnings.len(), 1);
        assert_eq!(validation.warnings[0].code, PlanWarningCode::CountMismatch);
        assert_eq!(validation.warnings[0].path, "groups[0].declaredCount");
    }

    #[test]
    fn declared_count_match_produces_no_warning() {
        let document = plan(vec![group("g1", 2)], vec![shot("s1", "g1"), shot("s2", "g1")]);
        assert!(validate_plan(&document, &HashSet::new()).warnings.is_empty());
    }

    #[test]
    fn missing_asset_points_at_the_exact_reference() {
        let mut target = shot("s1", "g1");
        target.references.push(viewer_domain::plan::ShotReference {
            asset_id: AssetId::from_bytes([7; 32]),
            role: None,
        });
        let document = plan(vec![group("g1", 1)], vec![target]);
        let validation = validate_plan(&document, &HashSet::new());
        assert_eq!(validation.errors.len(), 1);
        assert_eq!(validation.errors[0].code, PlanErrorCode::AssetMissing);
        assert_eq!(validation.errors[0].path, "shots[0].references[0]");
    }

    #[test]
    fn duplicate_ids_across_groups_and_shots_are_errors() {
        let document = plan(vec![group("x", 1)], vec![shot("x", "x")]);
        let validation = validate_plan(&document, &HashSet::new());
        assert!(
            validation
                .errors
                .iter()
                .any(|error| error.code == PlanErrorCode::DuplicateId)
        );
    }

    #[test]
    fn unknown_group_and_unknown_dimension_value_are_errors() {
        let mut document = plan(vec![group("g1", 1)], vec![shot("s1", "missing")]);
        document.dimensions.push(Dimension {
            id: "series".to_owned(),
            label: "套系".to_owned(),
            values: vec![DimensionValue {
                id: "summer".to_owned(),
                label: "夏".to_owned(),
                fragment: None,
                references: Vec::new(),
            }],
        });
        document.groups[0]
            .attributes
            .insert("series".to_owned(), "winter".to_owned());

        let validation = validate_plan(&document, &HashSet::new());
        assert!(
            validation
                .errors
                .iter()
                .any(|error| error.code == PlanErrorCode::UnknownGroup)
        );
        assert!(validation.errors.iter().any(|error| {
            error.code == PlanErrorCode::UnknownValue
                && error.path == "groups[0].attributes.series"
        }));
    }

    #[test]
    fn empty_plan_reports_missing_groups_and_shots() {
        let validation = validate_plan(&plan(Vec::new(), Vec::new()), &HashSet::new());
        assert_eq!(validation.errors.len(), 2);
    }
}
