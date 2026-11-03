use std::path::{Path, PathBuf};

use viewer_domain::plan::{PlanId, PlanMarks};

const MAX_MARKS_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum MarksError {
    #[error("marks store io failed: {0}")]
    Io(String),
    #[error("marks file is not valid: {0}")]
    Invalid(String),
    #[error("marks path escapes the project root")]
    UnsafePath,
}

pub struct MarksStore {
    root: PathBuf,
}

impl MarksStore {
    pub fn open(project_root: &Path) -> Result<Self, MarksError> {
        let root = std::fs::canonicalize(project_root)
            .map_err(|error| MarksError::Io(error.to_string()))?;
        if !root.is_dir() {
            return Err(MarksError::UnsafePath);
        }
        Ok(Self { root })
    }

    pub fn read(&self, plan_id: &PlanId) -> Result<Option<PlanMarks>, MarksError> {
        let path = self.path(plan_id)?;
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(MarksError::Io(error.to_string())),
        };
        if bytes.len() as u64 > MAX_MARKS_BYTES {
            return Err(MarksError::Invalid("marks file is too large".to_owned()));
        }
        PlanMarks::parse(&bytes, plan_id)
            .map(Some)
            .map_err(|error| MarksError::Invalid(error.message))
    }

    pub fn write(&self, marks: &PlanMarks) -> Result<PathBuf, MarksError> {
        let path = self.path(&marks.plan_id)?;
        let directory = path.parent().ok_or(MarksError::UnsafePath)?.to_path_buf();
        std::fs::create_dir_all(&directory).map_err(|error| MarksError::Io(error.to_string()))?;
        let canonical =
            std::fs::canonicalize(&directory).map_err(|error| MarksError::Io(error.to_string()))?;
        if !canonical.starts_with(&self.root) {
            return Err(MarksError::UnsafePath);
        }
        let bytes =
            serde_json::to_vec_pretty(marks).map_err(|error| MarksError::Io(error.to_string()))?;
        if bytes.len() as u64 > MAX_MARKS_BYTES {
            return Err(MarksError::Invalid("marks file is too large".to_owned()));
        }
        let temporary = directory.join("current.json.tmp");
        std::fs::write(&temporary, &bytes).map_err(|error| MarksError::Io(error.to_string()))?;
        std::fs::rename(&temporary, &path).map_err(|error| MarksError::Io(error.to_string()))?;
        Ok(path)
    }

    fn path(&self, plan_id: &PlanId) -> Result<PathBuf, MarksError> {
        Ok(self
            .root
            .join(".viewer")
            .join("plan-marks")
            .join(plan_id.as_str())
            .join("current.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viewer_domain::plan::{MARKS_PROTOCOL_V1, MarkCategory, MarkTarget, PlanMark, Revision};

    fn marks(revision: u32) -> PlanMarks {
        PlanMarks {
            protocol_version: MARKS_PROTOCOL_V1.to_owned(),
            plan_id: PlanId::parse("autumn").unwrap(),
            marks: vec![PlanMark {
                id: "mk_1".to_owned(),
                revision: Revision::new(revision),
                target: MarkTarget::Shot {
                    shot_id: "s1".to_owned(),
                },
                category: MarkCategory::Wrong,
                field: None,
                note: Some("换掉这张".to_owned()),
                observed_hash: Some("abc".to_owned()),
                created_at: "2026-11-03T00:00:00Z".to_owned(),
            }],
            updated_at: "2026-11-03T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn reads_back_what_it_wrote() {
        let directory = tempfile::tempdir().unwrap();
        let store = MarksStore::open(directory.path()).unwrap();
        let written = marks(2);
        store.write(&written).unwrap();
        let read = store.read(&written.plan_id).unwrap().unwrap();
        assert_eq!(read, written);
    }

    #[test]
    fn a_project_without_marks_reads_none() {
        let directory = tempfile::tempdir().unwrap();
        let store = MarksStore::open(directory.path()).unwrap();
        assert!(
            store
                .read(&PlanId::parse("autumn").unwrap())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn writing_replaces_the_previous_snapshot_atomically() {
        let directory = tempfile::tempdir().unwrap();
        let store = MarksStore::open(directory.path()).unwrap();
        store.write(&marks(1)).unwrap();
        store.write(&marks(2)).unwrap();
        let read = store
            .read(&PlanId::parse("autumn").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(read.marks[0].revision.get(), 2);
        assert!(
            !directory
                .path()
                .join(".viewer/plan-marks/autumn/current.json.tmp")
                .exists()
        );
    }
}
