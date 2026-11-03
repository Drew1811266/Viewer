use std::path::{Path, PathBuf};

use viewer_domain::plan::{PlanId, RunRecord};

const MAX_RUN_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("run id or plan id is not a safe file name component")]
    UnsafeId,
    #[error("run {0} already exists and runs are append-only")]
    Conflict(String),
    #[error("run record is not valid: {0}")]
    Invalid(String),
    #[error("run store io failed: {0}")]
    Io(String),
}

#[derive(Clone, Debug)]
pub struct RunRepository {
    root: PathBuf,
}

impl RunRepository {
    pub fn open(project_root: &Path) -> Result<Self, RunError> {
        let root = std::fs::canonicalize(project_root).map_err(|_| RunError::UnsafeId)?;
        if !root.is_dir() {
            return Err(RunError::UnsafeId);
        }
        Ok(Self { root })
    }

    pub fn record(&self, record: &RunRecord) -> Result<PathBuf, RunError> {
        let directory = self.directory(&record.plan_id)?;
        std::fs::create_dir_all(&directory).map_err(|error| RunError::Io(error.to_string()))?;
        let path = directory.join(format!("{}.json", safe_component(&record.run_id)?));
        let bytes =
            serde_json::to_vec_pretty(record).map_err(|error| RunError::Io(error.to_string()))?;
        if bytes.len() as u64 > MAX_RUN_BYTES {
            return Err(RunError::Invalid("run record is too large".to_owned()));
        }
        let temporary = directory.join(format!(".{}.json.tmp", safe_component(&record.run_id)?));
        std::fs::write(&temporary, &bytes).map_err(|error| RunError::Io(error.to_string()))?;
        match std::fs::hard_link(&temporary, &path) {
            Ok(()) => {
                let _ = std::fs::remove_file(&temporary);
                Ok(path)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = std::fs::remove_file(&temporary);
                Err(RunError::Conflict(record.run_id.clone()))
            }
            Err(error) => {
                let _ = std::fs::remove_file(&temporary);
                Err(RunError::Io(error.to_string()))
            }
        }
    }

    pub fn list(&self, plan_id: &PlanId) -> Result<Vec<RunRecord>, RunError> {
        let directory = self.directory(plan_id)?;
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(RunError::Io(error.to_string())),
        };

        let mut records = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| RunError::Io(error.to_string()))?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path).map_err(|error| RunError::Io(error.to_string()))?;
            if bytes.len() as u64 > MAX_RUN_BYTES {
                continue;
            }
            if let Ok(record) = RunRecord::parse(&bytes) {
                records.push(record);
            }
        }
        records.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(records)
    }

    fn directory(&self, plan_id: &PlanId) -> Result<PathBuf, RunError> {
        let viewer = self.root.join(".viewer");
        std::fs::create_dir_all(&viewer).map_err(|error| RunError::Io(error.to_string()))?;
        let viewer =
            std::fs::canonicalize(&viewer).map_err(|error| RunError::Io(error.to_string()))?;
        if !viewer.starts_with(&self.root) {
            return Err(RunError::UnsafeId);
        }
        Ok(viewer.join("plans").join("runs").join(plan_id.as_str()))
    }
}

fn safe_component(value: &str) -> Result<&str, RunError> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && value != "."
        && value != ".."
        && !value.contains('/');
    if valid {
        Ok(value)
    } else {
        Err(RunError::UnsafeId)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viewer_domain::plan::{Producer, RUN_PROTOCOL_V1, Revision};

    fn record(run_id: &str, revision: u32, model: &str) -> RunRecord {
        RunRecord {
            protocol_version: RUN_PROTOCOL_V1.to_owned(),
            run_id: run_id.to_owned(),
            plan_id: PlanId::parse("autumn").unwrap(),
            plan_revision: Revision::new(revision),
            created_at: "2026-11-03T00:00:00Z".to_owned(),
            producer: Producer {
                name: "harness".to_owned(),
                session_id: None,
            },
            model: Some(viewer_domain::plan::ModelRef {
                id: model.to_owned(),
                version: None,
                params: Default::default(),
            }),
            params: Default::default(),
            shots: Vec::new(),
        }
    }

    #[test]
    fn records_and_lists_runs_for_a_plan() {
        let directory = tempfile::tempdir().unwrap();
        let repository = RunRepository::open(directory.path()).unwrap();
        repository.record(&record("run-1", 3, "seedream")).unwrap();
        repository
            .record(&record("run-2", 3, "nano-banana"))
            .unwrap();

        let runs = repository.list(&PlanId::parse("autumn").unwrap()).unwrap();
        assert_eq!(runs.len(), 2);
        assert!(runs.iter().any(|run| run.run_id == "run-2"));
    }

    #[test]
    fn a_model_swap_adds_a_run_without_touching_the_plan_revision() {
        let directory = tempfile::tempdir().unwrap();
        let repository = RunRepository::open(directory.path()).unwrap();
        let first = repository.record(&record("run-1", 3, "seedream")).unwrap();
        let second = repository
            .record(&record("run-2", 3, "nano-banana"))
            .unwrap();

        assert_ne!(first, second);
        let runs = repository.list(&PlanId::parse("autumn").unwrap()).unwrap();
        assert!(runs.iter().all(|run| run.plan_revision.get() == 3));
    }

    #[test]
    fn a_repeated_run_id_is_a_conflict() {
        let directory = tempfile::tempdir().unwrap();
        let repository = RunRepository::open(directory.path()).unwrap();
        repository.record(&record("run-1", 3, "seedream")).unwrap();
        assert!(matches!(
            repository.record(&record("run-1", 3, "other")),
            Err(RunError::Conflict(_))
        ));
    }

    #[test]
    fn unsafe_ids_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let repository = RunRepository::open(directory.path()).unwrap();
        for unsafe_id in ["../escape", "a/b", "", "."] {
            assert!(matches!(
                repository.record(&record(unsafe_id, 1, "m")),
                Err(RunError::UnsafeId)
            ));
        }
    }

    #[test]
    fn an_unknown_plan_lists_no_runs() {
        let directory = tempfile::tempdir().unwrap();
        let repository = RunRepository::open(directory.path()).unwrap();
        assert!(
            repository
                .list(&PlanId::parse("nothing").unwrap())
                .unwrap()
                .is_empty()
        );
    }
}
