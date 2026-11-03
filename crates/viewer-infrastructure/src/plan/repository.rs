use blake3;
use std::path::{Path, PathBuf};
use viewer_application::plan::{AssetIndexView, validate_plan};
use viewer_domain::plan::{MAX_PLAN_BYTES, PlanDocument, PlanId, parse_plan};

use super::assets::AssetStore;

const RETRY_ATTEMPTS: usize = 3;
const RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(50);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IgnoredRevision {
    pub revision: u32,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct PublishedPlan {
    pub plan: PlanDocument,
    pub blake3: String,
    pub ignored: Vec<IgnoredRevision>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanSummary {
    pub plan_id: String,
    pub revision: u32,
    pub ignored: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("no plan exists in this project")]
    NoPlan,
    #[error("unknown plan {0}")]
    UnknownPlan(String),
    #[error("plan {0} has no valid revision")]
    InvalidPlan(String),
    #[error("plan repository io failed: {0}")]
    Io(String),
}

pub struct PlanRepository {
    incoming_root: PathBuf,
    assets: AssetStore,
}

impl PlanRepository {
    pub fn open(project_root: &Path) -> Result<Self, RepoError> {
        let incoming_root = project_root.join(".viewer").join("plans").join("incoming");
        let assets = AssetStore::open(project_root)
            .map_err(|error| RepoError::Io(error.to_string()))?;
        Ok(Self {
            incoming_root,
            assets,
        })
    }

    pub fn current(&self, plan_id: &PlanId) -> Result<PublishedPlan, RepoError> {
        let directory = self.incoming_root.join(plan_id.as_str());
        if !directory.is_dir() {
            return Err(RepoError::UnknownPlan(plan_id.to_string()));
        }

        let mut revisions = Vec::new();
        let entries =
            std::fs::read_dir(&directory).map_err(|error| RepoError::Io(error.to_string()))?;
        for entry in entries {
            let entry = entry.map_err(|error| RepoError::Io(error.to_string()))?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            if stem.is_empty() || stem.len() > 9 || !stem.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            if let Ok(revision) = stem.parse::<u32>() {
                revisions.push(revision);
            }
        }
        revisions.sort_unstable_by(|left, right| right.cmp(left));

        let mut ignored = Vec::new();
        for revision in &revisions {
            let path = directory.join(format!("{revision}.json"));
            match self.read_and_validate(&path) {
                Ok((plan, hash)) => {
                    return Ok(PublishedPlan {
                        plan,
                        blake3: hash,
                        ignored,
                    });
                }
                Err((code, message)) => ignored.push(IgnoredRevision {
                    revision: *revision,
                    code,
                    message,
                }),
            }
        }

        Err(RepoError::InvalidPlan(plan_id.to_string()))
    }

    pub fn list(&self) -> Vec<PlanSummary> {
        let Ok(entries) = std::fs::read_dir(&self.incoming_root) else {
            return Vec::new();
        };
        let mut summaries = Vec::new();
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(plan_id) = PlanId::parse(&name) else {
                continue;
            };
            if let Ok(published) = self.current(&plan_id) {
                summaries.push(PlanSummary {
                    plan_id: name,
                    revision: published.plan.revision.get(),
                    ignored: published.ignored.len(),
                });
            }
        }
        summaries.sort_by(|left, right| left.plan_id.cmp(&right.plan_id));
        summaries
    }

    fn read_and_validate(
        &self,
        path: &Path,
    ) -> Result<(PlanDocument, String), (String, String)> {
        let mut last_failure: Option<(String, String)> = None;
        for attempt in 0..RETRY_ATTEMPTS {
            let bytes = std::fs::read(path)
                .map_err(|error| ("io".to_owned(), error.to_string()))?;
            let hash = blake3::hash(&bytes).to_hex().to_string();
            match parse_plan(&bytes, MAX_PLAN_BYTES) {
                Ok(plan) => {
                    let validation = validate_plan(&plan, &self.assets);
                    if let Some(error) = validation.errors.first() {
                        return Err((error.code.as_str().to_owned(), error.message.clone()));
                    }
                    return Ok((plan, hash));
                }
                Err(error) => {
                    last_failure = Some((error.code.as_str().to_owned(), error.message));
                    if attempt + 1 < RETRY_ATTEMPTS {
                        std::thread::sleep(RETRY_DELAY);
                    }
                }
            }
        }
        Err(last_failure.unwrap_or_else(|| ("io".to_owned(), "plan read failed".to_owned())))
    }
}

impl AssetIndexView for PlanRepository {
    fn contains(&self, asset_id: &viewer_domain::plan::AssetId) -> bool {
        self.assets.contains(asset_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use viewer_domain::plan::{PlanGroup, PlanShot, Producer, Revision};

    fn document(revision: u32) -> PlanDocument {
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
                brief: "brief".to_owned(),
            }],
            shots: vec![PlanShot {
                id: "s1".to_owned(),
                group_id: "g1".to_owned(),
                attributes: BTreeMap::new(),
                delta: Some(format!("revision {revision}")),
                prompt: None,
                references: Vec::new(),
                spec: None,
                model: None,
            }],
            relocations: Vec::new(),
        }
    }

    fn write_revision(root: &Path, revision: u32, bytes: &[u8]) {
        let directory = root.join(".viewer/plans/incoming/autumn");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("{revision}.json")), bytes).unwrap();
    }

    fn setup() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        PlanRepository::open(directory.path()).unwrap();
        directory
    }

    #[test]
    fn picks_the_highest_valid_revision() {
        let directory = setup();
        for revision in 1..=3 {
            write_revision(
                directory.path(),
                revision,
                serde_json::to_string(&document(revision)).unwrap().as_bytes(),
            );
        }
        let repository = PlanRepository::open(directory.path()).unwrap();
        let published = repository
            .current(&PlanId::parse("autumn").unwrap())
            .unwrap();
        assert_eq!(published.plan.revision.get(), 3);
        assert!(published.ignored.is_empty());
    }

    #[test]
    fn truncated_highest_revision_falls_back_to_previous() {
        let directory = setup();
        write_revision(
            directory.path(),
            2,
            serde_json::to_string(&document(2)).unwrap().as_bytes(),
        );
        write_revision(directory.path(), 3, b"{\"protocolVersion\":\"viewer.plan/1\"");

        let repository = PlanRepository::open(directory.path()).unwrap();
        let published = repository
            .current(&PlanId::parse("autumn").unwrap())
            .unwrap();
        assert_eq!(published.plan.revision.get(), 2);
        assert_eq!(published.ignored.len(), 1);
        assert_eq!(published.ignored[0].revision, 3);
    }

    #[test]
    fn richer_semantic_errors_also_ignore_the_higher_revision() {
        let directory = setup();
        write_revision(
            directory.path(),
            1,
            serde_json::to_string(&document(1)).unwrap().as_bytes(),
        );
        let mut broken = document(2);
        broken.shots[0].group_id = "missing".to_owned();
        write_revision(
            directory.path(),
            2,
            serde_json::to_string(&broken).unwrap().as_bytes(),
        );

        let published = PlanRepository::open(directory.path())
            .unwrap()
            .current(&PlanId::parse("autumn").unwrap())
            .unwrap();
        assert_eq!(published.plan.revision.get(), 1);
        assert_eq!(published.ignored[0].code, "unknown_group");
    }

    #[test]
    fn unknown_plan_is_reported() {
        let directory = setup();
        let repository = PlanRepository::open(directory.path()).unwrap();
        assert!(matches!(
            repository.current(&PlanId::parse("nope").unwrap()),
            Err(RepoError::UnknownPlan(_))
        ));
    }

    #[test]
    fn list_reports_one_summary_per_plan_directory() {
        let directory = setup();
        write_revision(
            directory.path(),
            1,
            serde_json::to_string(&document(1)).unwrap().as_bytes(),
        );
        let summaries = PlanRepository::open(directory.path()).unwrap().list();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].plan_id, "autumn");
        assert_eq!(summaries[0].revision, 1);
    }
}
