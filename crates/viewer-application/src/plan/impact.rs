use std::collections::{BTreeMap, BTreeSet};
use viewer_domain::plan::PlanDocument;

use super::compose::compose_plan;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeLayer {
    Requirement,
    Material,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanChange {
    pub layer: ChangeLayer,
    pub kind: ChangeKind,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDiff {
    pub requirement: Vec<PlanChange>,
    pub material: Vec<PlanChange>,
    pub affected_shots: Vec<String>,
}

impl PlanDiff {
    pub fn is_empty(&self) -> bool {
        self.requirement.is_empty() && self.material.is_empty() && self.affected_shots.is_empty()
    }
}

pub fn diff_plans(previous: &PlanDocument, current: &PlanDocument) -> PlanDiff {
    let mut diff = PlanDiff::default();
    let mut affected: BTreeSet<String> = BTreeSet::new();

    let previous_groups: BTreeMap<&str, &viewer_domain::plan::PlanGroup> = previous
        .groups
        .iter()
        .map(|group| (group.id.as_str(), group))
        .collect();
    for group in &current.groups {
        match previous_groups.get(group.id.as_str()) {
            None => diff.requirement.push(PlanChange {
                layer: ChangeLayer::Requirement,
                kind: ChangeKind::Added,
                path: format!("groups.{}", group.id),
                message: format!("新增组 {}", group.id),
            }),
            Some(old) => {
                for (field, changed) in [
                    ("brief", old.brief != group.brief),
                    ("label", old.label != group.label),
                    ("declaredCount", old.declared_count != group.declared_count),
                ] {
                    if changed {
                        diff.requirement.push(PlanChange {
                            layer: ChangeLayer::Requirement,
                            kind: ChangeKind::Modified,
                            path: format!("groups.{}.{field}", group.id),
                            message: format!("组 {} 的 {field} 变了", group.id),
                        });
                    }
                }
            }
        }
    }
    for group in &previous.groups {
        if !current
            .groups
            .iter()
            .any(|candidate| candidate.id == group.id)
        {
            diff.requirement.push(PlanChange {
                layer: ChangeLayer::Requirement,
                kind: ChangeKind::Removed,
                path: format!("groups.{}", group.id),
                message: format!("删除组 {}", group.id),
            });
        }
    }

    let previous_values: BTreeMap<(&str, &str), &viewer_domain::plan::DimensionValue> = previous
        .dimensions
        .iter()
        .flat_map(|dimension| {
            dimension
                .values
                .iter()
                .map(move |value| ((dimension.id.as_str(), value.id.as_str()), value))
        })
        .collect();
    for dimension in &current.dimensions {
        for value in &dimension.values {
            let key = (dimension.id.as_str(), value.id.as_str());
            match previous_values.get(&key) {
                None => diff.requirement.push(PlanChange {
                    layer: ChangeLayer::Requirement,
                    kind: ChangeKind::Added,
                    path: format!("dimensions.{}.values.{}", dimension.id, value.id),
                    message: format!("新增取值 {}/{}", dimension.id, value.id),
                }),
                Some(old) => {
                    if old.fragment != value.fragment || old.references != value.references {
                        diff.requirement.push(PlanChange {
                            layer: ChangeLayer::Requirement,
                            kind: ChangeKind::Modified,
                            path: format!("dimensions.{}.values.{}", dimension.id, value.id),
                            message: format!("共享取值 {}/{} 变了", dimension.id, value.id),
                        });
                    }
                }
            }
        }
    }

    let previous_rules: BTreeMap<&str, &str> = previous
        .rules
        .iter()
        .map(|rule| (rule.id.as_str(), rule.text.as_str()))
        .collect();
    for rule in &current.rules {
        if previous_rules.get(rule.id.as_str()) != Some(&rule.text.as_str()) {
            diff.requirement.push(PlanChange {
                layer: ChangeLayer::Requirement,
                kind: if previous_rules.contains_key(rule.id.as_str()) {
                    ChangeKind::Modified
                } else {
                    ChangeKind::Added
                },
                path: format!("rules.{}", rule.id),
                message: format!("规则 {} 变了", rule.id),
            });
        }
    }

    let previous_hashes: BTreeMap<String, String> = compose_plan(previous)
        .into_iter()
        .map(|shot| (shot.shot_id, shot.hash))
        .collect();
    let current_hashes: BTreeMap<String, String> = compose_plan(current)
        .into_iter()
        .map(|shot| (shot.shot_id, shot.hash))
        .collect();
    let previous_shots: BTreeMap<&str, &viewer_domain::plan::PlanShot> = previous
        .shots
        .iter()
        .map(|shot| (shot.id.as_str(), shot))
        .collect();

    for shot in &current.shots {
        match previous_shots.get(shot.id.as_str()) {
            None => {
                affected.insert(shot.id.clone());
                diff.material.push(PlanChange {
                    layer: ChangeLayer::Material,
                    kind: ChangeKind::Added,
                    path: format!("shots.{}", shot.id),
                    message: format!("新增分镜 {}", shot.id),
                });
            }
            Some(old) => {
                let hash_changed = previous_hashes.get(&shot.id) != current_hashes.get(&shot.id);
                if hash_changed || *old != shot {
                    affected.insert(shot.id.clone());
                }
                if *old != shot {
                    diff.material.push(PlanChange {
                        layer: ChangeLayer::Material,
                        kind: ChangeKind::Modified,
                        path: format!("shots.{}", shot.id),
                        message: format!("分镜 {} 的素材变了", shot.id),
                    });
                }
            }
        }
    }
    for shot in &previous.shots {
        if !current
            .shots
            .iter()
            .any(|candidate| candidate.id == shot.id)
        {
            affected.insert(shot.id.clone());
            diff.material.push(PlanChange {
                layer: ChangeLayer::Material,
                kind: ChangeKind::Removed,
                path: format!("shots.{}", shot.id),
                message: format!("删除分镜 {}", shot.id),
            });
        }
    }

    diff.affected_shots = affected.into_iter().collect();
    diff
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use viewer_domain::plan::{
        Dimension, DimensionValue, ModelRef, PlanGroup, PlanId, PlanShot, Producer, Revision,
        ShotSpec,
    };

    fn shot(id: &str, delta: &str) -> PlanShot {
        PlanShot {
            id: id.to_owned(),
            group_id: "g1".to_owned(),
            attributes: BTreeMap::new(),
            delta: Some(delta.to_owned()),
            prompt: None,
            references: Vec::new(),
            spec: None,
            model: None,
        }
    }

    fn plan(revision: u32, fragment: &str, shots: Vec<PlanShot>) -> PlanDocument {
        PlanDocument {
            protocol_version: "viewer.plan/1".to_owned(),
            plan_id: PlanId::parse("autumn").unwrap(),
            revision: Revision::new(revision),
            title: None,
            created_at: "2026-11-03T00:00:00Z".to_owned(),
            producer: Producer {
                name: "harness".to_owned(),
                session_id: None,
            },
            dimensions: vec![Dimension {
                id: "series".to_owned(),
                label: "套系".to_owned(),
                values: vec![DimensionValue {
                    id: "summer".to_owned(),
                    label: "夏".to_owned(),
                    fragment: Some(fragment.to_owned()),
                    references: Vec::new(),
                }],
            }],
            rules: Vec::new(),
            groups: vec![PlanGroup {
                id: "g1".to_owned(),
                label: "夏".to_owned(),
                attributes: BTreeMap::new(),
                declared_count: shots.len() as u32,
                brief: "轻薄夏装".to_owned(),
                references: Vec::new(),
            }],
            shots,
            relocations: Vec::new(),
        }
    }

    fn with_attribute(mut shot: PlanShot, key: &str, value: &str) -> PlanShot {
        shot.attributes.insert(key.to_owned(), value.to_owned());
        shot
    }

    #[test]
    fn an_unchanged_plan_has_no_diff() {
        let shots = vec![with_attribute(shot("s1", "a"), "series", "summer")];
        let diff = diff_plans(&plan(1, "明亮", shots.clone()), &plan(2, "明亮", shots));
        assert!(diff.is_empty(), "{diff:?}");
    }

    #[test]
    fn changing_a_shared_fragment_flags_the_requirement_layer_and_affected_shots() {
        let before = plan(
            1,
            "明亮",
            vec![
                with_attribute(shot("s1", "a"), "series", "summer"),
                with_attribute(shot("s2", "b"), "series", "summer"),
            ],
        );
        let after = plan(
            2,
            "明亮清爽",
            vec![
                with_attribute(shot("s1", "a"), "series", "summer"),
                with_attribute(shot("s2", "b"), "series", "summer"),
            ],
        );
        let diff = diff_plans(&before, &after);
        assert_eq!(diff.affected_shots, vec!["s1".to_owned(), "s2".to_owned()]);
        assert_eq!(diff.requirement[0].layer, ChangeLayer::Requirement);
        assert!(diff.material.is_empty());
    }

    #[test]
    fn swapping_one_model_only_touches_the_material_layer() {
        let mut before_shots = vec![shot("s1", "a"), shot("s2", "b")];
        let mut after_shots = before_shots.clone();
        after_shots[1].model = Some(ModelRef {
            id: "nano-banana".to_owned(),
            version: None,
            params: BTreeMap::new(),
        });
        before_shots[1].model = Some(ModelRef {
            id: "seedream".to_owned(),
            version: None,
            params: BTreeMap::new(),
        });

        let diff = diff_plans(
            &plan(1, "明亮", before_shots),
            &plan(1, "明亮", after_shots),
        );
        assert_eq!(diff.affected_shots, vec!["s2".to_owned()]);
        assert!(diff.requirement.is_empty());
        assert_eq!(diff.material[0].layer, ChangeLayer::Material);
    }

    #[test]
    fn an_added_shot_is_reported_as_affected() {
        let diff = diff_plans(
            &plan(1, "明亮", vec![shot("s1", "a")]),
            &plan(2, "明亮", vec![shot("s1", "a"), shot("s2", "b")]),
        );
        assert!(diff.affected_shots.contains(&"s2".to_owned()));
        assert!(
            diff.material
                .iter()
                .any(|change| change.kind == ChangeKind::Added)
        );
    }

    #[test]
    fn a_spec_change_is_material() {
        let mut after = shot("s1", "a");
        after.spec = Some(ShotSpec {
            aspect: Some("4:5".to_owned()),
            size: None,
            seed: None,
        });
        let diff = diff_plans(
            &plan(1, "明亮", vec![shot("s1", "a")]),
            &plan(2, "明亮", vec![after]),
        );
        assert_eq!(diff.affected_shots, vec!["s1".to_owned()]);
        assert_eq!(diff.material.len(), 1);
    }
}
