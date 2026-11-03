use std::path::Path;

use viewer_application::plan::{compose_plan, diff_plans, validate_plan};
use viewer_domain::plan::{PlanId, PlanMarks};
use viewer_infrastructure::plan::{AssetStore, MarksStore, PlanRepository};

use crate::dto::plan::{PlanGroupDto, PlanMarksDto, PlanShotDto, PlanSnapshotDto, PlanSummaryDto};

#[derive(Debug)]
pub enum PlanViewError {
    Invalid(String),
    NotFound(String),
    Io(String),
}

impl std::fmt::Display for PlanViewError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) | Self::NotFound(message) | Self::Io(message) => {
                formatter.write_str(message)
            }
        }
    }
}

pub fn list_plans(root: &Path) -> Result<Vec<PlanSummaryDto>, PlanViewError> {
    let repository =
        PlanRepository::open(root).map_err(|error| PlanViewError::Io(error.to_string()))?;
    Ok(repository
        .list()
        .into_iter()
        .map(|summary| PlanSummaryDto {
            plan_id: summary.plan_id,
            revision: summary.revision,
            ignored: summary.ignored,
        })
        .collect())
}

pub fn plan_snapshot(root: &Path, plan_id: &str) -> Result<PlanSnapshotDto, PlanViewError> {
    let plan_id = parse_plan_id(plan_id)?;
    let repository =
        PlanRepository::open(root).map_err(|error| PlanViewError::Io(error.to_string()))?;
    let published = repository
        .current(&plan_id)
        .map_err(|error| PlanViewError::NotFound(error.to_string()))?;
    let previous = repository.previous(&plan_id).ok().flatten();

    let composed = compose_plan(&published.plan);
    let mut groups: Vec<PlanGroupDto> = published
        .plan
        .groups
        .iter()
        .map(|group| PlanGroupDto {
            id: group.id.clone(),
            label: group.label.clone(),
            declared_count: group.declared_count,
            actual_count: published
                .plan
                .shots
                .iter()
                .filter(|shot| shot.group_id == group.id)
                .count() as u32,
            brief: group.brief.clone(),
        })
        .collect();
    groups.sort_by(|left, right| left.id.cmp(&right.id));

    let shots = composed
        .iter()
        .map(|shot| PlanShotDto {
            shot_id: shot.shot_id.clone(),
            group_id: published
                .plan
                .shots
                .iter()
                .find(|candidate| candidate.id == shot.shot_id)
                .map(|candidate| candidate.group_id.clone())
                .unwrap_or_default(),
            prompt: shot.prompt.clone(),
            hash: shot.hash.clone(),
            references: shot
                .references
                .iter()
                .map(|reference| reference.asset_id.as_hex())
                .collect(),
        })
        .collect();

    let assets = AssetStore::open(root).map_err(|error| PlanViewError::Io(error.to_string()))?;
    let validation = validate_plan(&published.plan, &assets);

    Ok(PlanSnapshotDto {
        plan_id: published.plan.plan_id.as_str().to_owned(),
        revision: published.plan.revision.get(),
        blake3: published.blake3,
        groups,
        shots,
        diff: previous.map(|previous| diff_plans(&previous.plan, &published.plan)),
        warnings: validation.warnings,
    })
}

pub fn read_marks(root: &Path, plan_id: &str) -> Result<PlanMarksDto, PlanViewError> {
    let plan_id = parse_plan_id(plan_id)?;
    let store = MarksStore::open(root).map_err(|error| PlanViewError::Io(error.to_string()))?;
    let Some(marks) = store
        .read(&plan_id)
        .map_err(|error| PlanViewError::Invalid(error.to_string()))?
    else {
        return Ok(PlanMarksDto {
            plan_id: plan_id.as_str().to_owned(),
            plan_available: PlanRepository::open(root)
                .ok()
                .and_then(|repository| repository.current(&plan_id).ok())
                .is_some(),
            plan_revision: None,
            marks: Vec::new(),
        });
    };

    let published = PlanRepository::open(root)
        .ok()
        .and_then(|repository| repository.current(&plan_id).ok());
    let (plan_available, plan_revision, resolved) = match published {
        Some(published) => (
            true,
            Some(published.plan.revision.get()),
            viewer_domain::plan::resolve_marks(&marks, &published.plan),
        ),
        None => (false, None, Vec::new()),
    };

    Ok(PlanMarksDto {
        plan_id: plan_id.as_str().to_owned(),
        plan_available,
        plan_revision,
        marks: resolved,
    })
}

pub fn write_marks(root: &Path, marks: PlanMarks) -> Result<(), PlanViewError> {
    let store = MarksStore::open(root).map_err(|error| PlanViewError::Io(error.to_string()))?;
    store
        .write(&marks)
        .map(|_| ())
        .map_err(|error| PlanViewError::Invalid(error.to_string()))
}

fn parse_plan_id(raw: &str) -> Result<PlanId, PlanViewError> {
    PlanId::parse(raw).map_err(|error| PlanViewError::Invalid(error.message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use viewer_domain::plan::{
        MARKS_PROTOCOL_V1, MarkCategory, MarkTarget, PlanMark, PlanMarks, Revision,
    };

    fn project() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        let plan = r#"{
          "protocolVersion": "viewer.plan/1",
          "planId": "autumn",
          "revision": 2,
          "createdAt": "2026-11-03T00:00:00Z",
          "producer": { "name": "harness" },
          "groups": [{ "id": "g1", "label": "夏", "declaredCount": 3, "brief": "轻薄夏装" }],
          "shots": [
            { "id": "s1", "groupId": "g1", "delta": "海边" },
            { "id": "s2", "groupId": "g1", "delta": "室内" }
          ]
        }"#;
        let path = directory
            .path()
            .join(".viewer/plans/incoming/autumn/2.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, plan).unwrap();
        directory
    }

    #[test]
    fn snapshot_reports_declared_and_actual_counts() {
        let directory = project();
        let snapshot = plan_snapshot(directory.path(), "autumn").unwrap();
        assert_eq!(snapshot.revision, 2);
        assert_eq!(snapshot.groups[0].declared_count, 3);
        assert_eq!(snapshot.groups[0].actual_count, 2);
        assert_eq!(snapshot.shots[0].prompt, "轻薄夏装\n海边");
        assert_eq!(
            snapshot.warnings[0].code,
            viewer_application::plan::PlanWarningCode::CountMismatch
        );
    }

    #[test]
    fn marks_round_trip_through_the_view_service() {
        let directory = project();
        let marks = PlanMarks {
            protocol_version: MARKS_PROTOCOL_V1.to_owned(),
            plan_id: PlanId::parse("autumn").unwrap(),
            marks: vec![PlanMark {
                id: "mk_1".to_owned(),
                revision: Revision::new(2),
                target: MarkTarget::Shot {
                    shot_id: "s1".to_owned(),
                },
                category: MarkCategory::Wrong,
                field: None,
                note: None,
                observed_hash: None,
                created_at: "2026-11-03T00:00:00Z".to_owned(),
            }],
            updated_at: "2026-11-03T00:00:00Z".to_owned(),
        };
        write_marks(directory.path(), marks).unwrap();
        let read = read_marks(directory.path(), "autumn").unwrap();
        assert!(read.plan_available);
        assert_eq!(read.marks.len(), 1);
    }

    #[test]
    fn an_unknown_plan_is_reported_as_not_found() {
        let directory = project();
        assert!(matches!(
            plan_snapshot(directory.path(), "nothing"),
            Err(PlanViewError::NotFound(_))
        ));
    }
}
