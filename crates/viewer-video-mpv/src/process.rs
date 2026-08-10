use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    sync::Arc,
};

use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::Semaphore,
};

use crate::runtime_manifest::RuntimeLayout;

const MAX_TOOL_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum MediaToolError {
    #[error("bundled media tool paths must be absolute")]
    ExecutablePathMustBeAbsolute,
    #[error("bundled media tool paths do not match the runtime layout")]
    UnexpectedExecutablePath,
    #[error("the bundled media tool concurrency gate is closed")]
    GateClosed,
    #[error("the bundled media tool could not be started or awaited")]
    Io(#[from] std::io::Error),
    #[error("bundled media tool output exceeded the safety limit")]
    OutputTooLarge,
}

#[derive(Debug)]
pub struct MediaToolOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct BundledMediaTools {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
    gate: Arc<Semaphore>,
}

impl BundledMediaTools {
    pub fn from_layout(layout: &RuntimeLayout) -> Result<Self, MediaToolError> {
        if !layout.ffmpeg.is_absolute() || !layout.ffprobe.is_absolute() {
            return Err(MediaToolError::ExecutablePathMustBeAbsolute);
        }
        if layout.ffmpeg != layout.root.join("bin/ffmpeg")
            || layout.ffprobe != layout.root.join("bin/ffprobe")
        {
            return Err(MediaToolError::UnexpectedExecutablePath);
        }
        Ok(Self {
            ffmpeg: layout.ffmpeg.clone(),
            ffprobe: layout.ffprobe.clone(),
            gate: Arc::new(Semaphore::new(1)),
        })
    }

    pub async fn ffprobe(&self, arguments: &[OsString]) -> Result<MediaToolOutput, MediaToolError> {
        self.run(&self.ffprobe, arguments).await
    }

    pub async fn ffmpeg(&self, arguments: &[OsString]) -> Result<MediaToolOutput, MediaToolError> {
        self.run(&self.ffmpeg, arguments).await
    }

    async fn run(
        &self,
        executable: &Path,
        arguments: &[OsString],
    ) -> Result<MediaToolOutput, MediaToolError> {
        let _permit = self
            .gate
            .acquire()
            .await
            .map_err(|_| MediaToolError::GateClosed)?;
        let mut child = Command::new(executable)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let stdout = child.stdout.take().ok_or_else(|| {
            MediaToolError::Io(std::io::Error::other("stdout pipe was not created"))
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            MediaToolError::Io(std::io::Error::other("stderr pipe was not created"))
        })?;

        let (status, stdout, stderr) = tokio::try_join!(
            async { child.wait().await.map_err(MediaToolError::Io) },
            read_limited(stdout, MAX_TOOL_OUTPUT_BYTES),
            read_limited(stderr, MAX_TOOL_OUTPUT_BYTES),
        )?;
        Ok(MediaToolOutput {
            status,
            stdout,
            stderr,
        })
    }
}

async fn read_limited(
    reader: impl AsyncRead + Unpin,
    limit: usize,
) -> Result<Vec<u8>, MediaToolError> {
    let mut output = Vec::with_capacity(limit.min(64 * 1024));
    reader
        .take((limit + 1) as u64)
        .read_to_end(&mut output)
        .await?;
    if output.len() > limit {
        Err(MediaToolError::OutputTooLarge)
    } else {
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{BundledMediaTools, MediaToolError, read_limited};
    use crate::runtime_manifest::RuntimeLayout;

    #[test]
    fn media_tools_reject_relative_executable_paths() {
        let layout = RuntimeLayout {
            root: PathBuf::from("runtime"),
            libmpv: PathBuf::from("runtime/lib/libmpv.2.dylib"),
            ffmpeg: PathBuf::from("runtime/bin/ffmpeg"),
            ffprobe: PathBuf::from("runtime/bin/ffprobe"),
            manifest: PathBuf::from("runtime/runtime.lock.json"),
            licenses: PathBuf::from("runtime/licenses"),
        };

        assert!(matches!(
            BundledMediaTools::from_layout(&layout),
            Err(MediaToolError::ExecutablePathMustBeAbsolute)
        ));
    }

    #[tokio::test]
    async fn subprocess_output_is_rejected_at_the_configured_bound() {
        let error = read_limited(&b"12345"[..], 4).await.unwrap_err();
        assert!(matches!(error, MediaToolError::OutputTooLarge));
    }
}
