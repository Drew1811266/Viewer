use async_trait::async_trait;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectAccess {
    ReadWrite,
    ReadOnly,
}

#[async_trait]
pub trait ProjectProbePort: Send + Sync {
    async fn probe(&self, root: &Path) -> Result<ProjectAccess, String>;
}

pub trait ClockPort: Send + Sync {
    fn unix_millis(&self) -> i64;
}
