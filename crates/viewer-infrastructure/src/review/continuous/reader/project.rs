use super::{Failure, ReadErrorCode, heads::PinnedReviewHeads};
use crate::review::MAX_REVIEW_INDEX_BYTES;
use crate::review::continuous::owned_io::Directory;
use crate::review::{
    continuous::{migration, repository::View},
    v3,
};
use std::path::Path;
use viewer_domain::ProjectId;

pub(super) struct Project {
    pub root: Directory,
    pub directory: Option<Directory>,
    pub index: Option<Vec<u8>>,
    pub heads: Option<PinnedReviewHeads>,
}
impl Project {
    pub fn open(path: &str, pin_heads: bool) -> Result<Self, Failure> {
        let root = Directory::open_anchored(Path::new(path))?;
        let heads = pin_heads
            .then(|| PinnedReviewHeads::open(&root))
            .transpose()?
            .flatten();
        let directory = match root.child(".viewer", false)? {
            Some(viewer) => viewer.child("reviews", false)?,
            None => None,
        };
        let index = match &directory {
            Some(directory) => directory
                .pin_index()?
                .map(|index| index.read(MAX_REVIEW_INDEX_BYTES))
                .transpose()?,
            None => None,
        };
        Ok(Self {
            root,
            directory,
            index,
            heads,
        })
    }
    pub fn required(&self) -> Result<(&Directory, &[u8]), Failure> {
        self.root.verify()?;
        self.directory
            .as_ref()
            .zip(self.index.as_deref())
            .ok_or_else(|| Failure::new(ReadErrorCode::Io, "review index is missing"))
    }
    pub fn into_current(self) -> Result<CurrentProject, Failure> {
        if self.index.is_none()
            && let Some(directory) = &self.directory
        {
            crate::review::continuous::migration_inspect::verify_without_index(directory)?;
        }
        if let (Some(directory), Some(bytes)) = (self.directory, self.index) {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Version {
                protocol_version: String,
            }
            let version: Version = serde_json::from_slice(&bytes)
                .map_err(|_| Failure::integrity("review index is not valid JSON"))?;
            if matches!(
                version.protocol_version.as_str(),
                "viewer.review/1" | "viewer.review/2"
            ) {
                return Err(Failure::new(
                    ReadErrorCode::MigrationRequired,
                    "legacy review requires explicit Viewer migration; no current instructions returned",
                ));
            }
            let index = v3::decode_index_v3(&bytes)?;
            if let Some(backup) = &index.legacy_index {
                migration::verify_backup(&directory, backup, index.project_id)?;
            }
            if let Some(heads) = &self.heads {
                heads.require_project(index.project_id)?;
            }
            return Ok(CurrentProject {
                root: self.root,
                project_id: index.project_id,
                heads: self.heads,
                view: Some(View {
                    directory,
                    index,
                    index_bytes: Some(bytes),
                    ancestry: Default::default(),
                }),
            });
        }
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Identity {
            schema_version: u16,
            project_id: String,
            created_at_ms: i64,
        }
        let bytes = self
            .root
            .child(".viewer", false)?
            .ok_or_else(|| Failure::new(ReadErrorCode::Io, "project identity is missing"))?
            .read("project.json", 64 * 1024)?
            .ok_or_else(|| Failure::new(ReadErrorCode::Io, "project identity is missing"))?;
        let identity: Identity = serde_json::from_slice(&bytes)
            .map_err(|_| Failure::integrity("project identity is invalid"))?;
        if identity.schema_version != 1
            || !(0..=9_007_199_254_740_991).contains(&identity.created_at_ms)
        {
            return Err(Failure::integrity(
                "project identity version or timestamp is invalid",
            ));
        }
        let project_id = super::request::parse_id(&identity.project_id)?;
        if let Some(heads) = &self.heads {
            heads.require_project(project_id)?;
        }
        Ok(CurrentProject {
            root: self.root,
            project_id,
            heads: self.heads,
            view: None,
        })
    }
}

pub(super) struct CurrentProject {
    pub root: Directory,
    pub project_id: ProjectId,
    pub heads: Option<PinnedReviewHeads>,
    pub view: Option<View>,
}
