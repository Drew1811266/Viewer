use crate::{EntityId, RelativePath};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Directory,
    Jpeg,
    Png,
    Markdown,
    Text,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Keep,
    Pending,
    Reject,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileNode {
    pub entity_id: EntityId,
    pub relative_path: RelativePath,
    pub kind: FileKind,
    pub size: u64,
    pub modified_ns: i128,
}
