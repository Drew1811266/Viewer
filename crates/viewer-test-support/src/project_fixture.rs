use std::{fs, path::Path};
use viewer_domain::RelativePath;

pub struct ProjectFixture {
    directory: tempfile::TempDir,
}

impl ProjectFixture {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().expect("create disposable Viewer project");
        fs::create_dir(directory.path().join(".viewer"))
            .expect("create disposable Viewer metadata directory");
        Self { directory }
    }

    pub fn root(&self) -> &Path {
        self.directory.path()
    }

    pub fn metadata_path(&self) -> std::path::PathBuf {
        self.root().join(".viewer/metadata.sqlite")
    }

    pub fn create_file(&self, relative: &str, contents: &[u8]) -> RelativePath {
        let relative = RelativePath::parse(relative).expect("valid fixture-relative path");
        let path = self.root().join(relative.as_str());
        fs::create_dir_all(path.parent().expect("fixture file parent"))
            .expect("create fixture file parent");
        fs::write(path, contents).expect("write fixture file");
        relative
    }

    pub fn create_directory(&self, relative: &str) -> RelativePath {
        let relative = RelativePath::parse(relative).expect("valid fixture-relative path");
        fs::create_dir_all(self.root().join(relative.as_str())).expect("create fixture directory");
        relative
    }
}

impl Default for ProjectFixture {
    fn default() -> Self {
        Self::new()
    }
}
