use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    sync::Arc,
    time::Duration,
};

use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::Semaphore,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::runtime_manifest::RuntimeLayout;

const MAX_TOOL_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_FFPROBE_STDOUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_FFPROBE_STDERR_BYTES: usize = 64 * 1024;
const FFPROBE_TIMEOUT: Duration = Duration::from_secs(15);
const FFPROBE_ENTRIES: &str = "stream=codec_type,codec_name,width,height,avg_frame_rate,r_frame_rate:stream_tags=rotate:stream_side_data=rotation:format=duration";

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
    #[error("bundled ffprobe stdout exceeded the safety limit")]
    StdoutTooLarge,
    #[error("bundled ffprobe stderr exceeded the safety limit")]
    StderrTooLarge,
    #[error("bundled ffprobe was cancelled")]
    Cancelled,
    #[error("bundled ffprobe timed out")]
    TimedOut,
    #[error("the media input disappeared")]
    InputMissing,
    #[error("the media input is unreadable")]
    InputUnreadable,
    #[error("the media input path is not canonical")]
    InputPathNotCanonical,
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

    pub async fn ffprobe_json(
        &self,
        canonical_path: &Path,
        cancellation: CancellationToken,
    ) -> Result<MediaToolOutput, MediaToolError> {
        self.ffprobe_json_with_timeout(canonical_path, cancellation, FFPROBE_TIMEOUT)
            .await
    }

    async fn ffprobe_json_with_timeout(
        &self,
        canonical_path: &Path,
        cancellation: CancellationToken,
        timeout: Duration,
    ) -> Result<MediaToolOutput, MediaToolError> {
        validate_media_path(canonical_path)?;
        let arguments = [
            OsString::from("-v"),
            OsString::from("error"),
            OsString::from("-print_format"),
            OsString::from("json"),
            OsString::from("-show_streams"),
            OsString::from("-show_format"),
            OsString::from("-show_entries"),
            OsString::from(FFPROBE_ENTRIES),
            OsString::from("--"),
            canonical_path.as_os_str().to_owned(),
        ];
        self.run_ffprobe_bounded(&arguments, cancellation, timeout)
            .await
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

    async fn run_ffprobe_bounded(
        &self,
        arguments: &[OsString],
        cancellation: CancellationToken,
        timeout: Duration,
    ) -> Result<MediaToolOutput, MediaToolError> {
        let deadline = tokio::time::Instant::now() + timeout;
        let permit = tokio::select! {
            _ = cancellation.cancelled() => return Err(MediaToolError::Cancelled),
            _ = tokio::time::sleep_until(deadline) => return Err(MediaToolError::TimedOut),
            permit = self.gate.acquire() => permit.map_err(|_| MediaToolError::GateClosed)?,
        };
        if cancellation.is_cancelled() {
            return Err(MediaToolError::Cancelled);
        }

        let mut child = Command::new(&self.ffprobe)
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
        let mut stdout_task = tokio::spawn(read_limited(stdout, MAX_FFPROBE_STDOUT_BYTES));
        let mut stderr_task = tokio::spawn(read_limited(stderr, MAX_FFPROBE_STDERR_BYTES));

        let mut status = None;
        let mut stdout = None;
        let mut stderr = None;
        let mut stdout_settled = false;
        let mut stderr_settled = false;
        let completion = {
            let wait = child.wait();
            tokio::pin!(wait);
            loop {
                tokio::select! {
                    _ = cancellation.cancelled() => break Err(MediaToolError::Cancelled),
                    _ = tokio::time::sleep_until(deadline) => break Err(MediaToolError::TimedOut),
                    result = &mut wait, if status.is_none() => {
                        match result {
                            Ok(exit_status) => status = Some(exit_status),
                            Err(error) => break Err(MediaToolError::Io(error)),
                        }
                    }
                    result = &mut stdout_task, if stdout.is_none() => {
                        stdout_settled = true;
                        match pipe_result(result) {
                            Ok(bytes) => stdout = Some(bytes),
                            Err(MediaToolError::OutputTooLarge) => {
                                break Err(MediaToolError::StdoutTooLarge);
                            }
                            Err(error) => break Err(error),
                        }
                    }
                    result = &mut stderr_task, if stderr.is_none() => {
                        stderr_settled = true;
                        match pipe_result(result) {
                            Ok(bytes) => stderr = Some(bytes),
                            Err(MediaToolError::OutputTooLarge) => {
                                break Err(MediaToolError::StderrTooLarge);
                            }
                            Err(error) => break Err(error),
                        }
                    }
                }
                if status.is_some() && stdout.is_some() && stderr.is_some() {
                    let (Some(exit_status), Some(stdout), Some(stderr)) =
                        (status.take(), stdout.take(), stderr.take())
                    else {
                        unreachable!("all ffprobe outputs were checked")
                    };
                    break Ok(MediaToolOutput {
                        status: exit_status,
                        stdout,
                        stderr,
                    });
                }
            }
        };

        if completion.is_err() {
            let _ = child.start_kill();
            if status.is_none() {
                child.wait().await.map_err(MediaToolError::Io)?;
            }
            settle_reader(&mut stdout_task, stdout_settled).await;
            settle_reader(&mut stderr_task, stderr_settled).await;
        }
        drop(permit);
        completion
    }
}

fn validate_media_path(path: &Path) -> Result<(), MediaToolError> {
    if !path.is_absolute() {
        return Err(MediaToolError::InputPathNotCanonical);
    }
    let canonical = path.canonicalize().map_err(map_input_error)?;
    if canonical != path {
        return Err(MediaToolError::InputPathNotCanonical);
    }
    std::fs::File::open(path).map_err(map_input_error)?;
    Ok(())
}

fn map_input_error(error: std::io::Error) -> MediaToolError {
    match error.kind() {
        std::io::ErrorKind::NotFound => MediaToolError::InputMissing,
        std::io::ErrorKind::PermissionDenied => MediaToolError::InputUnreadable,
        _ => MediaToolError::InputUnreadable,
    }
}

fn pipe_result(
    result: Result<Result<Vec<u8>, MediaToolError>, tokio::task::JoinError>,
) -> Result<Vec<u8>, MediaToolError> {
    result.map_err(|_| MediaToolError::Io(std::io::Error::other("pipe reader task failed")))?
}

async fn settle_reader(reader: &mut JoinHandle<Result<Vec<u8>, MediaToolError>>, complete: bool) {
    if !complete {
        let _ = reader.await;
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
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        sync::LazyLock,
        time::Duration,
    };
    use tokio_util::sync::CancellationToken;

    use super::{BundledMediaTools, MediaToolError, read_limited};
    use crate::runtime_manifest::RuntimeLayout;

    static FAKE_PROCESS_GATE: LazyLock<tokio::sync::Semaphore> =
        LazyLock::new(|| tokio::sync::Semaphore::new(1));

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

    #[tokio::test]
    async fn ffprobe_json_uses_only_the_fixed_reviewed_arguments() {
        let _gate = FAKE_PROCESS_GATE.acquire().await.unwrap();
        let fixture = fake_tools("printf '%s\\n' \"$@\"");
        let media = canonical_media(fixture.path(), "sample.mp4");
        let output = fixture
            .tools()
            .ffprobe_json(&media, CancellationToken::new())
            .await
            .unwrap();
        let arguments = String::from_utf8(output.stdout).unwrap();

        assert_eq!(
            arguments.lines().collect::<Vec<_>>(),
            [
                "-v",
                "error",
                "-print_format",
                "json",
                "-show_streams",
                "-show_format",
                "-show_entries",
                "stream=codec_type,codec_name,width,height,avg_frame_rate,r_frame_rate:stream_tags=rotate:stream_side_data=rotation:format=duration",
                "--",
                media.to_str().unwrap(),
            ]
        );
    }

    #[tokio::test]
    async fn ffprobe_json_enforces_independent_stdout_and_stderr_bounds() {
        let _gate = FAKE_PROCESS_GATE.acquire().await.unwrap();
        let stdout_fixture = fake_tools("echo $$ > \"$0.pid\"; exec /usr/bin/yes stdout-overflow");
        let media = canonical_media(stdout_fixture.path(), "stdout.mp4");
        assert!(matches!(
            stdout_fixture
                .tools()
                .ffprobe_json(&media, CancellationToken::new())
                .await,
            Err(MediaToolError::StdoutTooLarge)
        ));
        let pid = wait_for_pid(&stdout_fixture.ffprobe_path().with_extension("pid")).await;
        assert_process_is_gone(pid);

        let stderr_fixture =
            fake_tools("echo $$ > \"$0.pid\"; exec /usr/bin/yes stderr-overflow >&2");
        let media = canonical_media(stderr_fixture.path(), "stderr.mp4");
        assert!(matches!(
            stderr_fixture
                .tools()
                .ffprobe_json(&media, CancellationToken::new())
                .await,
            Err(MediaToolError::StderrTooLarge)
        ));
        let pid = wait_for_pid(&stderr_fixture.ffprobe_path().with_extension("pid")).await;
        assert_process_is_gone(pid);
    }

    #[tokio::test]
    async fn cancellation_kills_and_awaits_the_ffprobe_child() {
        let _gate = FAKE_PROCESS_GATE.acquire().await.unwrap();
        let fixture = fake_tools("echo $$ > \"$0.pid\"; exec /usr/bin/tail -f /dev/null");
        let media = canonical_media(fixture.path(), "cancel.mp4");
        let cancellation = CancellationToken::new();
        let task = {
            let tools = fixture.tools();
            let cancellation = cancellation.clone();
            tokio::spawn(async move { tools.ffprobe_json(&media, cancellation).await })
        };
        let pid = wait_for_pid(&fixture.ffprobe_path().with_extension("pid")).await;

        cancellation.cancel();
        let result = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("cancelled ffprobe must be killed and awaited")
            .unwrap();

        assert!(matches!(result, Err(MediaToolError::Cancelled)));
        assert_process_is_gone(pid);
    }

    #[tokio::test]
    async fn timeout_kills_and_awaits_the_ffprobe_child() {
        let _gate = FAKE_PROCESS_GATE.acquire().await.unwrap();
        let fixture = fake_tools("echo $$ > \"$0.pid\"; exec /usr/bin/tail -f /dev/null");
        let media = canonical_media(fixture.path(), "timeout.mp4");

        let result = fixture
            .tools()
            .ffprobe_json_with_timeout(&media, CancellationToken::new(), Duration::from_secs(1))
            .await;
        let pid = wait_for_pid(&fixture.ffprobe_path().with_extension("pid")).await;

        assert!(matches!(result, Err(MediaToolError::TimedOut)));
        assert_process_is_gone(pid);
    }

    struct FakeTools {
        directory: tempfile::TempDir,
        layout: RuntimeLayout,
    }

    impl FakeTools {
        fn path(&self) -> &Path {
            self.directory.path()
        }

        fn ffprobe_path(&self) -> &Path {
            &self.layout.ffprobe
        }

        fn tools(&self) -> BundledMediaTools {
            BundledMediaTools::from_layout(&self.layout).unwrap()
        }
    }

    fn fake_tools(body: &str) -> FakeTools {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("ViewerVideoRuntime");
        fs::create_dir_all(root.join("bin")).unwrap();
        let ffprobe = root.join("bin/ffprobe");
        fs::write(&ffprobe, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&ffprobe, fs::Permissions::from_mode(0o755)).unwrap();
        let layout = RuntimeLayout {
            libmpv: root.join("lib/libmpv.2.dylib"),
            ffmpeg: root.join("bin/ffmpeg"),
            ffprobe,
            manifest: root.join("runtime.lock.json"),
            licenses: root.join("licenses"),
            root,
        };
        FakeTools { directory, layout }
    }

    fn canonical_media(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        fs::write(&path, b"fixture").unwrap();
        path.canonicalize().unwrap()
    }

    async fn wait_for_pid(path: &Path) -> libc::pid_t {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Ok(pid) = fs::read_to_string(path)
                    && let Ok(pid) = pid.trim().parse()
                {
                    break pid;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("fake ffprobe must publish its pid")
    }

    fn assert_process_is_gone(pid: libc::pid_t) {
        let result = unsafe { libc::kill(pid, 0) };
        assert_eq!(result, -1, "process {pid} is still alive or unreaped");
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }
}
