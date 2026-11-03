use blake3;
use std::collections::BTreeMap;
use viewer_domain::plan::{AssetId, AssetRole, PlanDocument, PlanShot};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SegmentSource {
    Group,
    Value { dimension: String, value: String },
    Shot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptSegment {
    pub source: SegmentSource,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReferenceOrigin {
    Group,
    Value { dimension: String, value: String },
    Shot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectiveReference {
    pub asset_id: AssetId,
    pub role: Option<AssetRole>,
    pub origin: ReferenceOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComposedShot {
    pub shot_id: String,
    pub prompt: String,
    pub segments: Vec<PromptSegment>,
    pub references: Vec<EffectiveReference>,
    pub hash: String,
}

pub fn compose_plan(plan: &PlanDocument) -> Vec<ComposedShot> {
    plan.shots
        .iter()
        .map(|shot| compose_shot(plan, shot))
        .collect()
}

fn compose_shot(plan: &PlanDocument, shot: &PlanShot) -> ComposedShot {
    let mut segments = Vec::new();
    let mut references: Vec<EffectiveReference> = Vec::new();

    if let Some(prompt) = shot
        .prompt
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        segments.push(PromptSegment {
            source: SegmentSource::Shot,
            text: prompt.clone(),
        });
    } else {
        if let Some(group) = plan.groups.iter().find(|group| group.id == shot.group_id) {
            push_text(&mut segments, SegmentSource::Group, Some(&group.brief));
        }

        let mut attributes: BTreeMap<&str, &str> = BTreeMap::new();
        for (key, value) in &shot.attributes {
            attributes.insert(key.as_str(), value.as_str());
        }
        for dimension in &plan.dimensions {
            let Some(value_id) = attributes.get(dimension.id.as_str()) else {
                continue;
            };
            let Some(value) = dimension.values.iter().find(|value| value.id == *value_id) else {
                continue;
            };
            push_text(
                &mut segments,
                value_source(dimension, value),
                value.fragment.as_deref(),
            );
            for asset_id in &value.references {
                push_reference(
                    &mut references,
                    *asset_id,
                    None,
                    value_origin(dimension, value),
                );
            }
        }
        push_text(&mut segments, SegmentSource::Shot, shot.delta.as_deref());
    }

    for reference in &shot.references {
        push_reference(
            &mut references,
            reference.asset_id,
            reference.role,
            ReferenceOrigin::Shot,
        );
    }

    let prompt = segments
        .iter()
        .map(|segment| segment.text.as_str())
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    ComposedShot {
        shot_id: shot.id.clone(),
        hash: shot_hash(shot, &segments, &references),
        prompt,
        segments,
        references,
    }
}

fn value_source(
    dimension: &viewer_domain::plan::Dimension,
    value: &viewer_domain::plan::DimensionValue,
) -> SegmentSource {
    SegmentSource::Value {
        dimension: dimension.id.clone(),
        value: value.id.clone(),
    }
}

fn value_origin(
    dimension: &viewer_domain::plan::Dimension,
    value: &viewer_domain::plan::DimensionValue,
) -> ReferenceOrigin {
    ReferenceOrigin::Value {
        dimension: dimension.id.clone(),
        value: value.id.clone(),
    }
}

fn push_text(segments: &mut Vec<PromptSegment>, source: SegmentSource, text: Option<&str>) {
    let Some(text) = text.filter(|value| !value.trim().is_empty()) else {
        return;
    };
    segments.push(PromptSegment {
        source,
        text: text.to_owned(),
    });
}

fn push_reference(
    references: &mut Vec<EffectiveReference>,
    asset_id: AssetId,
    role: Option<AssetRole>,
    origin: ReferenceOrigin,
) {
    if references
        .iter()
        .any(|existing| existing.asset_id == asset_id)
    {
        return;
    }
    references.push(EffectiveReference {
        asset_id,
        role,
        origin,
    });
}

fn shot_hash(
    shot: &PlanShot,
    segments: &[PromptSegment],
    references: &[EffectiveReference],
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(shot.id.as_bytes());
    hasher.update(b"\0");
    hasher.update(shot.group_id.as_bytes());
    for (key, value) in &shot.attributes {
        hasher.update(key.as_bytes());
        hasher.update(b"=");
        hasher.update(value.as_bytes());
        hasher.update(b"\0");
    }
    for segment in segments {
        hasher.update(segment.text.as_bytes());
        hasher.update(b"\0");
    }
    let mut asset_ids: Vec<String> = references
        .iter()
        .map(|reference| reference.asset_id.as_hex())
        .collect();
    asset_ids.sort();
    for asset_id in asset_ids {
        hasher.update(asset_id.as_bytes());
        hasher.update(b"\0");
    }
    hasher.finalize().to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use viewer_domain::plan::{Dimension, DimensionValue, PlanGroup, PlanId, Producer, Revision};

    fn shot(id: &str, group_id: &str, attributes: &[(&str, &str)], delta: &str) -> PlanShot {
        PlanShot {
            id: id.to_owned(),
            group_id: group_id.to_owned(),
            attributes: attributes
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
            delta: Some(delta.to_owned()),
            prompt: None,
            references: Vec::new(),
            spec: None,
            model: None,
        }
    }

    fn plan(shots: Vec<PlanShot>, summer_fragment: &str) -> PlanDocument {
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
            dimensions: vec![
                Dimension {
                    id: "series".to_owned(),
                    label: "套系".to_owned(),
                    values: vec![
                        DimensionValue {
                            id: "summer".to_owned(),
                            label: "夏".to_owned(),
                            fragment: Some(summer_fragment.to_owned()),
                            references: Vec::new(),
                        },
                        DimensionValue {
                            id: "winter".to_owned(),
                            label: "冬".to_owned(),
                            fragment: Some("厚重冬装".to_owned()),
                            references: Vec::new(),
                        },
                    ],
                },
                Dimension {
                    id: "subject".to_owned(),
                    label: "人物".to_owned(),
                    values: vec![DimensionValue {
                        id: "boy".to_owned(),
                        label: "男孩".to_owned(),
                        fragment: Some("10 岁男孩".to_owned()),
                        references: Vec::new(),
                    }],
                },
            ],
            rules: Vec::new(),
            groups: vec![
                PlanGroup {
                    id: "g_summer".to_owned(),
                    label: "夏".to_owned(),
                    attributes: BTreeMap::new(),
                    declared_count: 1,
                    brief: "轻薄夏装".to_owned(),
                },
                PlanGroup {
                    id: "g_winter".to_owned(),
                    label: "冬".to_owned(),
                    attributes: BTreeMap::new(),
                    declared_count: 1,
                    brief: "厚重冬装".to_owned(),
                },
            ],
            shots,
            relocations: Vec::new(),
        }
    }

    #[test]
    fn composes_group_then_values_in_declaration_order_then_delta() {
        let document = plan(
            vec![shot(
                "s1",
                "g_summer",
                &[("series", "summer"), ("subject", "boy")],
                "海边逆光",
            )],
            "明亮清爽",
        );
        let composed = compose_plan(&document);
        let sources: Vec<_> = composed[0]
            .segments
            .iter()
            .map(|segment| segment.source.clone())
            .collect();
        assert_eq!(
            sources,
            vec![
                SegmentSource::Group,
                SegmentSource::Value {
                    dimension: "series".to_owned(),
                    value: "summer".to_owned()
                },
                SegmentSource::Value {
                    dimension: "subject".to_owned(),
                    value: "boy".to_owned()
                },
                SegmentSource::Shot,
            ]
        );
        assert_eq!(
            composed[0].prompt,
            "轻薄夏装\n明亮清爽\n10 岁男孩\n海边逆光"
        );
    }

    #[test]
    fn explicit_prompt_overrides_composition() {
        let mut target = shot("s1", "g_summer", &[("series", "summer")], "海边逆光");
        target.prompt = Some("完全手写的提示词".to_owned());
        let composed = compose_plan(&plan(vec![target], "明亮清爽"));
        assert_eq!(composed[0].segments.len(), 1);
        assert_eq!(composed[0].segments[0].source, SegmentSource::Shot);
        assert_eq!(composed[0].prompt, "完全手写的提示词");
    }

    #[test]
    fn value_references_record_their_owning_value() {
        let asset = AssetId::from_bytes([3; 32]);
        let mut document = plan(
            vec![shot("s1", "g_summer", &[("series", "summer")], "x")],
            "明亮",
        );
        document.dimensions[0].values[0].references = vec![asset];
        document.dimensions[0].values[0].fragment = None;

        let composed = compose_plan(&document);
        assert_eq!(composed[0].references.len(), 1);
        assert_eq!(composed[0].references[0].asset_id, asset);
        assert_eq!(
            composed[0].references[0].origin,
            ReferenceOrigin::Value {
                dimension: "series".to_owned(),
                value: "summer".to_owned()
            }
        );
    }

    #[test]
    fn hash_changes_only_for_shots_that_use_the_changed_fragment() {
        let shots = vec![
            shot("s1", "g_summer", &[("series", "summer")], "a"),
            shot("s2", "g_winter", &[("series", "winter")], "b"),
        ];
        let before = compose_plan(&plan(shots.clone(), "明亮清爽"));
        let after = compose_plan(&plan(shots, "明亮清爽，夏天光线"));
        assert_ne!(before[0].hash, after[0].hash);
        assert_eq!(before[1].hash, after[1].hash);
    }
}
