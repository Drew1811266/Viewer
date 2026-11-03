use serde::{Deserialize, Serialize};

use super::{PlanDocument, PlanError, PlanErrorCode, PlanId, Revision};

pub const MARKS_PROTOCOL_V1: &str = "viewer.plan-marks/1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MarkTarget {
    #[serde(rename_all = "camelCase")]
    Shot {
        shot_id: String,
    },
    #[serde(rename_all = "camelCase")]
    Group {
        group_id: String,
    },
    Plan,
}

impl MarkTarget {
    pub fn node_id(&self) -> Option<&str> {
        match self {
            Self::Shot { shot_id } => Some(shot_id),
            Self::Group { group_id } => Some(group_id),
            Self::Plan => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkCategory {
    Wrong,
    Missing,
    Extra,
    Style,
    Order,
    Other,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanMark {
    pub id: String,
    pub revision: Revision,
    pub target: MarkTarget,
    pub category: MarkCategory,
    #[serde(default)]
    pub field: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub observed_hash: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanMarks {
    pub protocol_version: String,
    pub plan_id: PlanId,
    #[serde(default)]
    pub marks: Vec<PlanMark>,
    pub updated_at: String,
}

impl PlanMarks {
    pub fn parse(bytes: &[u8], plan_id: &PlanId) -> Result<Self, PlanError> {
        let marks: Self = serde_json::from_slice(bytes)
            .map_err(|error| PlanError::new("", PlanErrorCode::InvalidSchema, error.to_string()))?;
        if marks.protocol_version != MARKS_PROTOCOL_V1 {
            return Err(PlanError::new(
                "protocolVersion",
                PlanErrorCode::UnsupportedVersion,
                format!("unsupported marks protocol {}", marks.protocol_version),
            ));
        }
        if marks.plan_id != *plan_id {
            return Err(PlanError::new(
                "planId",
                PlanErrorCode::InvalidSchema,
                "the marks file belongs to a different plan",
            ));
        }
        Ok(marks)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkStatus {
    /// 计划没动过，标记还没被处理。
    Pending,
    /// 同一个节点还在，但内容已经变了。
    Changed,
    /// 节点被 relocations 改名了，标记跟着走。
    Replaced,
    /// 节点消失了，也没有 relocation 能解释。
    Orphaned,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedMark {
    pub mark: PlanMark,
    pub status: MarkStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_target: Option<String>,
}

/// 比较标记与当前计划，得出每条标记的派生状态。纯函数，不读写磁盘。
pub fn resolve_marks(marks: &PlanMarks, plan: &PlanDocument) -> Vec<ResolvedMark> {
    marks
        .marks
        .iter()
        .map(|mark| resolve_mark(mark, plan))
        .collect()
}

fn resolve_mark(mark: &PlanMark, plan: &PlanDocument) -> ResolvedMark {
    let Some(target) = mark.target.node_id() else {
        // 计划级标记：只要还是同一份计划就算待处理。
        return ResolvedMark {
            mark: mark.clone(),
            status: if mark.revision.get() <= plan.revision.get() {
                MarkStatus::Pending
            } else {
                MarkStatus::Changed
            },
            resolved_target: None,
        };
    };

    let exists = match &mark.target {
        MarkTarget::Shot { .. } => plan.shots.iter().any(|shot| shot.id == target),
        MarkTarget::Group { .. } => plan.groups.iter().any(|group| group.id == target),
        MarkTarget::Plan => false,
    };

    if exists {
        let status = if mark.revision.get() >= plan.revision.get() {
            MarkStatus::Pending
        } else {
            MarkStatus::Changed
        };
        return ResolvedMark {
            mark: mark.clone(),
            status,
            resolved_target: Some(target.to_owned()),
        };
    }

    match relocation_target(plan, target) {
        Some(new_id) => ResolvedMark {
            mark: mark.clone(),
            status: MarkStatus::Replaced,
            resolved_target: Some(new_id.to_owned()),
        },
        None => ResolvedMark {
            mark: mark.clone(),
            status: MarkStatus::Orphaned,
            resolved_target: None,
        },
    }
}

fn relocation_target<'a>(plan: &'a PlanDocument, target: &str) -> Option<&'a str> {
    plan.relocations
        .iter()
        .find(|relocation| relocation.from == target)
        .map(|relocation| relocation.to.as_str())
}

#[cfg(test)]
mod tests {
    use super::super::{PlanGroup, PlanShot, Producer, Relocation, RelocationKind};
    use super::*;
    use std::collections::BTreeMap;

    fn shot(id: &str) -> PlanShot {
        PlanShot {
            id: id.to_owned(),
            group_id: "g1".to_owned(),
            attributes: BTreeMap::new(),
            delta: None,
            prompt: None,
            references: Vec::new(),
            spec: None,
            model: None,
        }
    }

    fn plan(revision: u32, relocations: Vec<Relocation>) -> PlanDocument {
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
            dimensions: Vec::new(),
            rules: Vec::new(),
            groups: vec![PlanGroup {
                id: "g1".to_owned(),
                label: "夏".to_owned(),
                attributes: BTreeMap::new(),
                declared_count: 1,
                brief: "b".to_owned(),
                references: Vec::new(),
            }],
            shots: vec![shot("s1")],
            relocations,
        }
    }

    fn mark(target: MarkTarget, revision: u32) -> PlanMark {
        PlanMark {
            id: "mk_1".to_owned(),
            revision: Revision::new(revision),
            target,
            category: MarkCategory::Wrong,
            field: None,
            note: None,
            observed_hash: None,
            created_at: "2026-11-03T00:00:00Z".to_owned(),
        }
    }

    fn marks(entries: Vec<PlanMark>) -> PlanMarks {
        PlanMarks {
            protocol_version: MARKS_PROTOCOL_V1.to_owned(),
            plan_id: PlanId::parse("autumn").unwrap(),
            marks: entries,
            updated_at: "2026-11-03T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn a_mark_on_the_current_revision_is_pending() {
        let resolved = resolve_marks(
            &marks(vec![mark(
                MarkTarget::Shot {
                    shot_id: "s1".to_owned(),
                },
                2,
            )]),
            &plan(2, Vec::new()),
        );
        assert_eq!(resolved[0].status, MarkStatus::Pending);
        assert_eq!(resolved[0].resolved_target.as_deref(), Some("s1"));
    }

    #[test]
    fn a_mark_on_an_older_revision_is_changed() {
        let resolved = resolve_marks(
            &marks(vec![mark(
                MarkTarget::Shot {
                    shot_id: "s1".to_owned(),
                },
                1,
            )]),
            &plan(2, Vec::new()),
        );
        assert_eq!(resolved[0].status, MarkStatus::Changed);
    }

    #[test]
    fn a_renamed_target_is_replaced_rather_than_orphaned() {
        let resolved = resolve_marks(
            &marks(vec![mark(
                MarkTarget::Shot {
                    shot_id: "s0".to_owned(),
                },
                1,
            )]),
            &plan(
                2,
                vec![Relocation {
                    kind: RelocationKind::Shot,
                    from: "s0".to_owned(),
                    to: "s1".to_owned(),
                }],
            ),
        );
        assert_eq!(resolved[0].status, MarkStatus::Replaced);
        assert_eq!(resolved[0].resolved_target.as_deref(), Some("s1"));
    }

    #[test]
    fn a_vanished_target_without_relocation_is_orphaned() {
        let resolved = resolve_marks(
            &marks(vec![mark(
                MarkTarget::Shot {
                    shot_id: "gone".to_owned(),
                },
                1,
            )]),
            &plan(2, Vec::new()),
        );
        assert_eq!(resolved[0].status, MarkStatus::Orphaned);
        assert!(resolved[0].resolved_target.is_none());
    }

    #[test]
    fn a_marks_file_for_another_plan_is_rejected() {
        let bytes = serde_json::to_vec(&PlanMarks {
            protocol_version: MARKS_PROTOCOL_V1.to_owned(),
            plan_id: PlanId::parse("winter").unwrap(),
            marks: Vec::new(),
            updated_at: "2026-11-03T00:00:00Z".to_owned(),
        })
        .unwrap();
        assert!(PlanMarks::parse(&bytes, &PlanId::parse("autumn").unwrap()).is_err());
    }
}
