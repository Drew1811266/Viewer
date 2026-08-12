use std::{
    ffi::{CString, OsString},
    fs::{File, OpenOptions},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{
            ffi::{OsStrExt, OsStringExt},
            fs::OpenOptionsExt,
            process::{CommandExt, ExitStatusExt},
        },
    },
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    sync::Arc,
    time::Duration,
};

use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::Semaphore,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::runtime_manifest::{RuntimeLayout, RuntimeManifest};

const MAX_TOOL_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_FFPROBE_STDOUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_FFPROBE_STDERR_BYTES: usize = 64 * 1024;
const FFPROBE_TIMEOUT: Duration = Duration::from_secs(15);
const FRAME_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_FRAME_STDOUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_FRAME_STDERR_BYTES: usize = 64 * 1024;
const FFPROBE_ENTRIES: &str = "stream=codec_type,codec_name,width,height,avg_frame_rate,r_frame_rate:stream_tags=rotate:stream_side_data=rotation:format=duration";

fn noop_path(_: &Path) {}

#[derive(Debug, Error)]
pub enum MediaToolError {
    #[error("bundled media tool paths must be absolute")]
    ExecutablePathMustBeAbsolute,
    #[error("bundled media tool paths do not match the runtime layout")]
    UnexpectedExecutablePath,
    #[error("the bundled media executable is not a safe regular bundle entry")]
    UnsafeExecutable,
    #[error("the bundled media executable changed after identity binding")]
    ExecutableChanged,
    #[error("the bundled video runtime integrity contract is unavailable")]
    RuntimeIntegrity,
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
    #[error("bundled media tool was cancelled")]
    Cancelled,
    #[error("bundled media tool timed out")]
    TimedOut,
    #[error("the media input disappeared")]
    InputMissing,
    #[error("the media input is unreadable")]
    InputUnreadable,
    #[error("the media input path is not canonical")]
    InputPathNotCanonical,
    #[error("the media input changed after active-entity validation")]
    InputChanged,
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
    ffmpeg_file: Arc<File>,
    ffmpeg_launch: LaunchExecutable,
    ffmpeg_identity: FileIdentity,
    ffmpeg_sha256: String,
    ffprobe: PathBuf,
    ffprobe_file: Arc<File>,
    ffprobe_launch: LaunchExecutable,
    ffprobe_identity: FileIdentity,
    ffprobe_sha256: String,
    gate: Arc<Semaphore>,
    _launch_directory: Arc<tempfile::TempDir>,
}

#[derive(Clone, Debug)]
struct LaunchExecutable {
    path: PathBuf,
    file: Arc<File>,
    identity: FileIdentity,
    mach_o: bool,
    #[cfg(target_os = "macos")]
    code_directory_hashes: Arc<[[u8; 20]]>,
}

struct FrameLaunchHooks<BeforeSpawn, AfterValidation, AfterSpawn, AfterPidpath> {
    before_spawn: BeforeSpawn,
    after_final_validation: AfterValidation,
    after_spawn: AfterSpawn,
    after_pidpath: AfterPidpath,
}

#[cfg(target_os = "macos")]
struct SuspendedProcess {
    pid: libc::pid_t,
    stdout: tokio::fs::File,
    stderr: tokio::fs::File,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileIdentity {
    len: u64,
    modified_ns: i128,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    changed_ns: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaFileIdentity(FileIdentity);

impl MediaFileIdentity {
    pub fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        Self(file_identity(metadata))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaFrameOutput {
    Gray160,
    Png320,
    Png640,
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
        let launch_directory = Arc::new(
            tempfile::Builder::new()
                .prefix("viewer-media-runtime-")
                .tempdir()
                .map_err(MediaToolError::Io)?,
        );
        let (ffmpeg, ffmpeg_file, ffmpeg_launch, ffmpeg_identity, ffmpeg_sha256) = bind_media_tool(
            layout,
            &layout.ffmpeg,
            "bin/ffmpeg",
            launch_directory.path(),
            "ffmpeg",
        )?;
        let (ffprobe, ffprobe_file, ffprobe_launch, ffprobe_identity, ffprobe_sha256) =
            bind_media_tool(
                layout,
                &layout.ffprobe,
                "bin/ffprobe",
                launch_directory.path(),
                "ffprobe",
            )?;
        Ok(Self {
            ffmpeg,
            ffmpeg_file,
            ffmpeg_launch,
            ffmpeg_identity,
            ffmpeg_sha256,
            ffprobe,
            ffprobe_file,
            ffprobe_launch,
            ffprobe_identity,
            ffprobe_sha256,
            gate: Arc::new(Semaphore::new(1)),
            _launch_directory: launch_directory,
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
        self.ffprobe_json_with_timeout(canonical_path, None, cancellation, FFPROBE_TIMEOUT)
            .await
    }

    pub async fn ffprobe_json_identity_bound(
        &self,
        canonical_path: &Path,
        expected_identity: &MediaFileIdentity,
        cancellation: CancellationToken,
    ) -> Result<MediaToolOutput, MediaToolError> {
        self.ffprobe_json_with_timeout(
            canonical_path,
            Some(expected_identity),
            cancellation,
            FFPROBE_TIMEOUT,
        )
        .await
    }

    async fn ffprobe_json_with_timeout(
        &self,
        canonical_path: &Path,
        expected_identity: Option<&MediaFileIdentity>,
        cancellation: CancellationToken,
        timeout: Duration,
    ) -> Result<MediaToolOutput, MediaToolError> {
        let input = validate_media_path(canonical_path, expected_identity)?;
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
            OsString::from("/dev/fd/0"),
        ];
        self.run_ffprobe_bounded(&arguments, input, cancellation, timeout)
            .await
    }

    pub async fn ffmpeg(&self, arguments: &[OsString]) -> Result<MediaToolOutput, MediaToolError> {
        self.run(&self.ffmpeg, arguments).await
    }

    pub async fn video_frame_identity_bound(
        &self,
        canonical_path: &Path,
        expected_identity: &MediaFileIdentity,
        time_us: u64,
        output: MediaFrameOutput,
        cancellation: CancellationToken,
    ) -> Result<MediaToolOutput, MediaToolError> {
        let input = validate_media_path(canonical_path, Some(expected_identity))?;
        let arguments = frame_arguments(time_us, output);
        self.run_ffmpeg_frame(
            &arguments,
            input,
            cancellation,
            FRAME_TIMEOUT,
            FrameLaunchHooks {
                before_spawn: || {},
                after_final_validation: || {},
                after_spawn: |_| {},
                after_pidpath: noop_path,
            },
        )
        .await
    }

    #[cfg(test)]
    async fn video_frame_identity_bound_with_before_spawn_for_test(
        &self,
        canonical_path: &Path,
        expected_identity: &MediaFileIdentity,
        time_us: u64,
        output: MediaFrameOutput,
        cancellation: CancellationToken,
        before_spawn: impl FnOnce(),
    ) -> Result<MediaToolOutput, MediaToolError> {
        let input = validate_media_path(canonical_path, Some(expected_identity))?;
        let arguments = frame_arguments(time_us, output);
        self.run_ffmpeg_frame(
            &arguments,
            input,
            cancellation,
            FRAME_TIMEOUT,
            FrameLaunchHooks {
                before_spawn,
                after_final_validation: || {},
                after_spawn: |_| {},
                after_pidpath: noop_path,
            },
        )
        .await
    }

    #[cfg(test)]
    async fn video_frame_identity_bound_with_after_final_validation_for_test(
        &self,
        canonical_path: &Path,
        expected_identity: &MediaFileIdentity,
        time_us: u64,
        output: MediaFrameOutput,
        cancellation: CancellationToken,
        after_final_validation: impl FnOnce(),
    ) -> Result<MediaToolOutput, MediaToolError> {
        let input = validate_media_path(canonical_path, Some(expected_identity))?;
        let arguments = frame_arguments(time_us, output);
        self.run_ffmpeg_frame(
            &arguments,
            input,
            cancellation,
            FRAME_TIMEOUT,
            FrameLaunchHooks {
                before_spawn: || {},
                after_final_validation,
                after_spawn: |_| {},
                after_pidpath: noop_path,
            },
        )
        .await
    }

    #[cfg(test)]
    async fn video_frame_identity_bound_with_spawn_aba_for_test(
        &self,
        canonical_path: &Path,
        expected_identity: &MediaFileIdentity,
        cancellation: CancellationToken,
        after_final_validation: impl FnOnce(),
        after_spawn: impl FnOnce(libc::pid_t),
        after_pidpath: impl FnOnce(&Path),
    ) -> Result<MediaToolOutput, MediaToolError> {
        let input = validate_media_path(canonical_path, Some(expected_identity))?;
        let arguments = frame_arguments(0, MediaFrameOutput::Png320);
        self.run_ffmpeg_frame(
            &arguments,
            input,
            cancellation,
            FRAME_TIMEOUT,
            FrameLaunchHooks {
                before_spawn: || {},
                after_final_validation,
                after_spawn,
                after_pidpath,
            },
        )
        .await
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
        if executable == self.ffprobe {
            self.revalidate_ffprobe()?;
        } else if executable == self.ffmpeg {
            self.revalidate_ffmpeg()?;
        }
        let launch = if executable == self.ffprobe {
            &self.ffprobe_launch
        } else if executable == self.ffmpeg {
            &self.ffmpeg_launch
        } else {
            return Err(MediaToolError::UnexpectedExecutablePath);
        };
        let mut command = command_from_verified_snapshot(launch, executable)?;
        let mut child = command
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        collect_raw_tool_output(&mut child).await
    }

    async fn run_ffprobe_bounded(
        &self,
        arguments: &[OsString],
        input: File,
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
        self.revalidate_ffprobe()?;

        let mut command = command_from_verified_snapshot(&self.ffprobe_launch, &self.ffprobe)?;
        let mut child = command
            .args(arguments)
            .stdin(Stdio::from(input))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let completion = collect_ffprobe_output(&mut child, cancellation, deadline).await;
        drop(permit);
        completion
    }

    async fn run_ffmpeg_frame<BeforeSpawn, AfterValidation, AfterSpawn, AfterPidpath>(
        &self,
        arguments: &[OsString],
        input: File,
        cancellation: CancellationToken,
        timeout: Duration,
        hooks: FrameLaunchHooks<BeforeSpawn, AfterValidation, AfterSpawn, AfterPidpath>,
    ) -> Result<MediaToolOutput, MediaToolError>
    where
        BeforeSpawn: FnOnce(),
        AfterValidation: FnOnce(),
        AfterSpawn: FnOnce(libc::pid_t),
        AfterPidpath: FnOnce(&Path),
    {
        let FrameLaunchHooks {
            before_spawn,
            after_final_validation,
            after_spawn,
            after_pidpath,
        } = hooks;
        let deadline = tokio::time::Instant::now() + timeout;
        let permit = tokio::select! {
            _ = cancellation.cancelled() => return Err(MediaToolError::Cancelled),
            _ = tokio::time::sleep_until(deadline) => return Err(MediaToolError::TimedOut),
            permit = self.gate.acquire() => permit.map_err(|_| MediaToolError::GateClosed)?,
        };
        if cancellation.is_cancelled() {
            return Err(MediaToolError::Cancelled);
        }
        self.revalidate_ffmpeg()?;
        before_spawn();
        if self.ffmpeg_launch.mach_o {
            let process = spawn_suspended_verified(
                &self.ffmpeg_launch,
                &self.ffmpeg,
                arguments,
                input,
                after_final_validation,
                after_spawn,
                after_pidpath,
            )?;
            let completion = collect_suspended_process_output(
                process,
                cancellation,
                deadline,
                MAX_FRAME_STDOUT_BYTES,
                MAX_FRAME_STDERR_BYTES,
            )
            .await;
            drop(permit);
            return completion;
        }
        let mut command = command_from_verified_snapshot(&self.ffmpeg_launch, &self.ffmpeg)?;
        after_final_validation();
        let mut child = command
            .args(arguments)
            .stdin(Stdio::from(input))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let completion = collect_process_output(
            &mut child,
            Some(cancellation),
            Some(deadline),
            MAX_FRAME_STDOUT_BYTES,
            MAX_FRAME_STDERR_BYTES,
        )
        .await;
        drop(permit);
        completion
    }

    fn revalidate_ffprobe(&self) -> Result<(), MediaToolError> {
        revalidate_media_tool(
            &self.ffprobe,
            &self.ffprobe_file,
            &self.ffprobe_launch,
            &self.ffprobe_identity,
            &self.ffprobe_sha256,
        )
    }

    fn revalidate_ffmpeg(&self) -> Result<(), MediaToolError> {
        revalidate_media_tool(
            &self.ffmpeg,
            &self.ffmpeg_file,
            &self.ffmpeg_launch,
            &self.ffmpeg_identity,
            &self.ffmpeg_sha256,
        )
    }
}

fn frame_arguments(time_us: u64, output: MediaFrameOutput) -> Vec<OsString> {
    let (filter, format) = match output {
        MediaFrameOutput::Gray160 => (
            "scale=160:-2:flags=lanczos,format=gray",
            ["-f", "rawvideo", "-pix_fmt", "gray", "pipe:1"].as_slice(),
        ),
        MediaFrameOutput::Png320 => (
            "scale=320:-2:flags=lanczos",
            ["-f", "image2pipe", "-c:v", "png", "pipe:1"].as_slice(),
        ),
        MediaFrameOutput::Png640 => (
            "scale=640:-2:flags=lanczos",
            ["-f", "image2pipe", "-c:v", "png", "pipe:1"].as_slice(),
        ),
    };
    let timestamp = format!("{}.{:06}", time_us / 1_000_000, time_us % 1_000_000);
    let mut arguments = [
        "-v",
        "error",
        "-ss",
        &timestamp,
        "-i",
        "/dev/fd/0",
        "-map",
        "0:v:0",
        "-frames:v",
        "1",
        "-an",
        "-sn",
        "-dn",
        "-vf",
        filter,
    ]
    .into_iter()
    .map(OsString::from)
    .collect::<Vec<_>>();
    arguments.extend(format.iter().map(OsString::from));
    arguments
}

async fn collect_ffprobe_output(
    child: &mut tokio::process::Child,
    cancellation: CancellationToken,
    deadline: tokio::time::Instant,
) -> Result<MediaToolOutput, MediaToolError> {
    collect_process_output(
        child,
        Some(cancellation),
        Some(deadline),
        MAX_FFPROBE_STDOUT_BYTES,
        MAX_FFPROBE_STDERR_BYTES,
    )
    .await
}

async fn collect_raw_tool_output(
    child: &mut tokio::process::Child,
) -> Result<MediaToolOutput, MediaToolError> {
    match collect_process_output(
        child,
        None,
        None,
        MAX_TOOL_OUTPUT_BYTES,
        MAX_TOOL_OUTPUT_BYTES,
    )
    .await
    {
        Err(MediaToolError::StdoutTooLarge | MediaToolError::StderrTooLarge) => {
            Err(MediaToolError::OutputTooLarge)
        }
        result => result,
    }
}

async fn collect_process_output(
    child: &mut tokio::process::Child,
    cancellation: Option<CancellationToken>,
    deadline: Option<tokio::time::Instant>,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<MediaToolOutput, MediaToolError> {
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            terminate_and_reap(child).await;
            return Err(MediaToolError::Io(std::io::Error::other(
                "stdout pipe was not created",
            )));
        }
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            drop(stdout);
            terminate_and_reap(child).await;
            return Err(MediaToolError::Io(std::io::Error::other(
                "stderr pipe was not created",
            )));
        }
    };
    let mut stdout_task = tokio::spawn(read_limited(stdout, stdout_limit));
    let mut stderr_task = tokio::spawn(read_limited(stderr, stderr_limit));
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
                _ = wait_for_cancellation(cancellation.as_ref()) => break Err(MediaToolError::Cancelled),
                _ = wait_for_deadline(deadline) => break Err(MediaToolError::TimedOut),
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
        if status.is_none() {
            terminate_and_reap(child).await;
        }
        settle_reader(&mut stdout_task, stdout_settled).await;
        settle_reader(&mut stderr_task, stderr_settled).await;
    }
    completion
}

#[cfg(target_os = "macos")]
async fn collect_suspended_process_output(
    process: SuspendedProcess,
    cancellation: CancellationToken,
    deadline: tokio::time::Instant,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<MediaToolOutput, MediaToolError> {
    let pid = process.pid;
    let mut stdout_task = tokio::spawn(read_limited(process.stdout, stdout_limit));
    let mut stderr_task = tokio::spawn(read_limited(process.stderr, stderr_limit));
    let mut wait_task = tokio::task::spawn_blocking(move || wait_for_pid(pid));
    let mut status = None;
    let mut stdout = None;
    let mut stderr = None;
    let mut stdout_settled = false;
    let mut stderr_settled = false;
    let mut wait_settled = false;
    let completion = loop {
        tokio::select! {
            _ = cancellation.cancelled() => break Err(MediaToolError::Cancelled),
            _ = tokio::time::sleep_until(deadline) => break Err(MediaToolError::TimedOut),
            result = &mut wait_task, if status.is_none() => {
                wait_settled = true;
                match result {
                    Ok(Ok(exit_status)) => status = Some(exit_status),
                    Ok(Err(error)) => break Err(MediaToolError::Io(error)),
                    Err(_) => break Err(MediaToolError::Io(std::io::Error::other(
                        "media tool waiter task failed",
                    ))),
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
                unreachable!("all media tool outputs were checked")
            };
            break Ok(MediaToolOutput {
                status: exit_status,
                stdout,
                stderr,
            });
        }
    };
    if completion.is_err() {
        if status.is_none() {
            let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
        }
        if !wait_settled {
            let _ = wait_task.await;
        }
        settle_reader(&mut stdout_task, stdout_settled).await;
        settle_reader(&mut stderr_task, stderr_settled).await;
    }
    completion
}

#[cfg(not(target_os = "macos"))]
async fn collect_suspended_process_output(
    process: !,
    _cancellation: CancellationToken,
    _deadline: tokio::time::Instant,
    _stdout_limit: usize,
    _stderr_limit: usize,
) -> Result<MediaToolOutput, MediaToolError> {
    process
}

async fn wait_for_cancellation(cancellation: Option<&CancellationToken>) {
    match cancellation {
        Some(cancellation) => cancellation.cancelled().await,
        None => std::future::pending().await,
    }
}

async fn wait_for_deadline(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

async fn terminate_and_reap(child: &mut tokio::process::Child) {
    let _ = child.start_kill();
    let _ = child.wait().await;
}

fn validate_media_path(
    path: &Path,
    expected_identity: Option<&MediaFileIdentity>,
) -> Result<File, MediaToolError> {
    validate_media_path_after_open(path, expected_identity, || {})
}

fn validate_media_path_after_open(
    path: &Path,
    expected_identity: Option<&MediaFileIdentity>,
    after_open: impl FnOnce(),
) -> Result<File, MediaToolError> {
    if !path.is_absolute() {
        return Err(MediaToolError::InputPathNotCanonical);
    }
    let path_metadata = std::fs::symlink_metadata(path).map_err(map_input_error)?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(MediaToolError::InputUnreadable);
    }
    let canonical = path.canonicalize().map_err(map_input_error)?;
    if canonical != path {
        return Err(MediaToolError::InputPathNotCanonical);
    }
    let file = File::open(path).map_err(map_input_error)?;
    after_open();
    let open_metadata = file.metadata().map_err(map_input_error)?;
    if !open_metadata.is_file() {
        return Err(MediaToolError::InputUnreadable);
    }
    if expected_identity.is_some_and(|expected| file_identity(&open_metadata) != expected.0) {
        return Err(MediaToolError::InputChanged);
    }
    if path.canonicalize().map_err(map_input_error)? != path {
        return Err(MediaToolError::InputPathNotCanonical);
    }
    Ok(file)
}

fn bind_media_tool(
    layout: &RuntimeLayout,
    executable: &Path,
    inventory_name: &str,
    launch_directory: &Path,
    launch_name: &str,
) -> Result<(PathBuf, Arc<File>, LaunchExecutable, FileIdentity, String), MediaToolError> {
    let root_metadata =
        std::fs::symlink_metadata(&layout.root).map_err(|_| MediaToolError::UnsafeExecutable)?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(MediaToolError::UnsafeExecutable);
    }
    let canonical_root = layout
        .root
        .canonicalize()
        .map_err(|_| MediaToolError::UnsafeExecutable)?;
    let executable_metadata =
        std::fs::symlink_metadata(executable).map_err(|_| MediaToolError::UnsafeExecutable)?;
    if executable_metadata.file_type().is_symlink() || !executable_metadata.is_file() {
        return Err(MediaToolError::UnsafeExecutable);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if executable_metadata.permissions().mode() & 0o111 == 0 {
            return Err(MediaToolError::UnsafeExecutable);
        }
    }
    let canonical_executable = executable
        .canonicalize()
        .map_err(|_| MediaToolError::UnsafeExecutable)?;
    if canonical_executable != canonical_root.join(inventory_name) {
        return Err(MediaToolError::UnsafeExecutable);
    }
    let executable_file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(executable)
        .map_err(|_| MediaToolError::UnsafeExecutable)?;
    let open_metadata = executable_file
        .metadata()
        .map_err(|_| MediaToolError::UnsafeExecutable)?;
    if !open_metadata.is_file()
        || file_identity(&open_metadata) != file_identity(&executable_metadata)
    {
        return Err(MediaToolError::UnsafeExecutable);
    }
    let executable_sha256 =
        validate_runtime_integrity(layout, &canonical_root, inventory_name, &executable_file)?;
    let launch = create_launch_snapshot(
        &executable_file,
        &executable_sha256,
        launch_directory,
        launch_name,
    )?;
    Ok((
        canonical_executable,
        Arc::new(executable_file),
        launch,
        file_identity(&executable_metadata),
        executable_sha256,
    ))
}

fn validate_runtime_integrity(
    layout: &RuntimeLayout,
    canonical_root: &Path,
    inventory_name: &str,
    executable: &File,
) -> Result<String, MediaToolError> {
    if layout.manifest != layout.root.join("runtime.lock.json") {
        return Err(MediaToolError::RuntimeIntegrity);
    }
    for (path, expected) in [
        (&layout.manifest, canonical_root.join("runtime.lock.json")),
        (
            &layout.root.join("runtime.inventory.sha256"),
            canonical_root.join("runtime.inventory.sha256"),
        ),
    ] {
        let metadata =
            std::fs::symlink_metadata(path).map_err(|_| MediaToolError::RuntimeIntegrity)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(MediaToolError::RuntimeIntegrity);
        }
        if path
            .canonicalize()
            .map_err(|_| MediaToolError::RuntimeIntegrity)?
            != expected
        {
            return Err(MediaToolError::RuntimeIntegrity);
        }
    }
    let manifest =
        RuntimeManifest::load(&layout.manifest).map_err(|_| MediaToolError::RuntimeIntegrity)?;
    if manifest.ffmpeg.tag != "n8.0"
        || ![
            "--disable-gpl",
            "--disable-nonfree",
            "--disable-network",
            "--disable-ffplay",
            "--enable-zlib",
            "--enable-encoder=png",
        ]
        .into_iter()
        .all(|required| {
            manifest
                .ffmpeg
                .configure_options
                .iter()
                .any(|option| option == required)
        })
    {
        return Err(MediaToolError::RuntimeIntegrity);
    }
    let inventory = std::fs::read_to_string(layout.root.join("runtime.inventory.sha256"))
        .map_err(|_| MediaToolError::RuntimeIntegrity)?;
    let entries = inventory
        .lines()
        .filter(|line| line.ends_with(&format!("  {inventory_name}")))
        .collect::<Vec<_>>();
    if entries.len() != 1 || entries[0].len() != 64 + 2 + inventory_name.len() {
        return Err(MediaToolError::RuntimeIntegrity);
    }
    let expected = &entries[0][..64];
    if !expected
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || sha256_file_handle(executable).map_err(|_| MediaToolError::RuntimeIntegrity)? != expected
    {
        return Err(MediaToolError::RuntimeIntegrity);
    }
    Ok(expected.to_owned())
}

fn revalidate_media_tool(
    executable: &Path,
    executable_file: &File,
    launch: &LaunchExecutable,
    expected_identity: &FileIdentity,
    expected_sha256: &str,
) -> Result<(), MediaToolError> {
    let metadata =
        std::fs::symlink_metadata(executable).map_err(|_| MediaToolError::ExecutableChanged)?;
    let valid = !metadata.file_type().is_symlink()
        && metadata.is_file()
        && file_identity(&metadata) == *expected_identity
        && executable_file
            .metadata()
            .is_ok_and(|metadata| file_identity(&metadata) == *expected_identity)
        && executable
            .canonicalize()
            .is_ok_and(|canonical| canonical == executable);
    if !valid
        || sha256_file_handle(executable_file).map_err(|_| MediaToolError::ExecutableChanged)?
            != expected_sha256
        || !launch_path_matches(launch)
        || sha256_file_handle(&launch.file).map_err(|_| MediaToolError::ExecutableChanged)?
            != expected_sha256
    {
        return Err(MediaToolError::ExecutableChanged);
    }
    Ok(())
}

fn sha256_file_handle(file: &File) -> Result<String, std::io::Error> {
    use std::os::unix::fs::FileExt;

    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut offset = 0_u64;
    loop {
        let read = file.read_at(&mut buffer, offset)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        offset += read as u64;
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn command_from_verified_snapshot(
    launch: &LaunchExecutable,
    display_path: &Path,
) -> Result<Command, MediaToolError> {
    if !launch_path_matches(launch) {
        return Err(MediaToolError::ExecutableChanged);
    }
    let mut command = Command::new(&launch.path);
    command.as_std_mut().arg0(display_path);
    Ok(command)
}

#[cfg(target_os = "macos")]
fn spawn_suspended_verified(
    launch: &LaunchExecutable,
    display_path: &Path,
    arguments: &[OsString],
    input: File,
    after_final_validation: impl FnOnce(),
    after_spawn: impl FnOnce(libc::pid_t),
    after_pidpath: impl FnOnce(&Path),
) -> Result<SuspendedProcess, MediaToolError> {
    if !launch_path_matches(launch) {
        return Err(MediaToolError::ExecutableChanged);
    }
    let executable = cstring(launch.path.as_os_str().as_bytes())?;
    let mut argument_values = Vec::with_capacity(arguments.len() + 1);
    argument_values.push(cstring(display_path.as_os_str().as_bytes())?);
    for argument in arguments {
        argument_values.push(cstring(argument.as_os_str().as_bytes())?);
    }
    let mut argument_pointers = argument_values
        .iter()
        .map(|argument| argument.as_ptr().cast_mut())
        .collect::<Vec<_>>();
    argument_pointers.push(std::ptr::null_mut());

    let environment_values = std::env::vars_os()
        .map(|(key, value)| {
            let mut entry = key.into_vec();
            entry.push(b'=');
            entry.extend(value.into_vec());
            cstring(&entry)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut environment_pointers = environment_values
        .iter()
        .map(|entry| entry.as_ptr().cast_mut())
        .collect::<Vec<_>>();
    environment_pointers.push(std::ptr::null_mut());

    let (stdout_read, stdout_write) = cloexec_pipe()?;
    let (stderr_read, stderr_write) = cloexec_pipe()?;
    let mut actions: libc::posix_spawn_file_actions_t = std::ptr::null_mut();
    posix_spawn_result(unsafe { libc::posix_spawn_file_actions_init(&mut actions) })?;
    let actions_result = [
        (input.as_raw_fd(), libc::STDIN_FILENO),
        (stdout_write.as_raw_fd(), libc::STDOUT_FILENO),
        (stderr_write.as_raw_fd(), libc::STDERR_FILENO),
    ]
    .into_iter()
    .try_for_each(|(source, target)| {
        posix_spawn_result(unsafe {
            libc::posix_spawn_file_actions_adddup2(&mut actions, source, target)
        })
    });
    if let Err(error) = actions_result {
        unsafe { libc::posix_spawn_file_actions_destroy(&mut actions) };
        return Err(error);
    }

    let mut attributes: libc::posix_spawnattr_t = std::ptr::null_mut();
    if let Err(error) = posix_spawn_result(unsafe { libc::posix_spawnattr_init(&mut attributes) }) {
        unsafe { libc::posix_spawn_file_actions_destroy(&mut actions) };
        return Err(error);
    }
    let flags = (libc::POSIX_SPAWN_START_SUSPENDED | libc::POSIX_SPAWN_CLOEXEC_DEFAULT) as i16;
    if let Err(error) =
        posix_spawn_result(unsafe { libc::posix_spawnattr_setflags(&mut attributes, flags) })
    {
        unsafe {
            libc::posix_spawnattr_destroy(&mut attributes);
            libc::posix_spawn_file_actions_destroy(&mut actions);
        }
        return Err(error);
    }

    after_final_validation();
    let mut pid = 0;
    let spawn_result = posix_spawn_result(unsafe {
        libc::posix_spawn(
            &mut pid,
            executable.as_ptr(),
            &actions,
            &attributes,
            argument_pointers.as_ptr(),
            environment_pointers.as_ptr(),
        )
    });
    unsafe {
        libc::posix_spawnattr_destroy(&mut attributes);
        libc::posix_spawn_file_actions_destroy(&mut actions);
    }
    spawn_result?;
    after_spawn(pid);
    drop(input);
    drop(stdout_write);
    drop(stderr_write);

    if !suspended_child_matches_launch_after_pidpath(pid, launch, after_pidpath) {
        kill_and_reap_pid(pid);
        return Err(MediaToolError::ExecutableChanged);
    }
    if unsafe { libc::kill(pid, libc::SIGCONT) } != 0 {
        let error = std::io::Error::last_os_error();
        kill_and_reap_pid(pid);
        return Err(MediaToolError::Io(error));
    }
    Ok(SuspendedProcess {
        pid,
        stdout: tokio::fs::File::from_std(File::from(stdout_read)),
        stderr: tokio::fs::File::from_std(File::from(stderr_read)),
    })
}

#[cfg(not(target_os = "macos"))]
fn spawn_suspended_verified(
    _launch: &LaunchExecutable,
    _display_path: &Path,
    _arguments: &[OsString],
    _input: File,
    after_final_validation: impl FnOnce(),
    _after_spawn: impl FnOnce(libc::pid_t),
    _after_pidpath: impl FnOnce(&Path),
) -> Result<!, MediaToolError> {
    after_final_validation();
    Err(MediaToolError::ExecutableChanged)
}

#[cfg(target_os = "macos")]
fn suspended_child_matches_launch_after_pidpath(
    pid: libc::pid_t,
    launch: &LaunchExecutable,
    after_pidpath: impl FnOnce(&Path),
) -> bool {
    let mut buffer = [0_u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let length = unsafe {
        libc::proc_pidpath(
            pid,
            buffer.as_mut_ptr().cast(),
            libc::PROC_PIDPATHINFO_MAXSIZE as u32,
        )
    };
    if length <= 0 {
        return false;
    }
    let path = Path::new(std::ffi::OsStr::from_bytes(&buffer[..length as usize]));
    after_pidpath(path);
    if path != launch.path {
        return false;
    }
    if !suspended_child_code_matches_launch(pid, launch) {
        return false;
    }
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
    {
        Ok(file) => file,
        Err(_) => return false,
    };
    file.metadata()
        .is_ok_and(|metadata| file_identity(&metadata) == launch.identity)
        && launch
            .file
            .metadata()
            .is_ok_and(|metadata| file_identity(&metadata) == launch.identity)
}

#[cfg(target_os = "macos")]
fn suspended_child_code_matches_launch(pid: libc::pid_t, launch: &LaunchExecutable) -> bool {
    const CS_OPS_STATUS: libc::c_uint = 0;
    const CS_OPS_CDHASH: libc::c_uint = 5;
    const CS_VALID: u32 = 0x0000_0001;

    unsafe extern "C" {
        fn csops(
            pid: libc::pid_t,
            ops: libc::c_uint,
            user_address: *mut libc::c_void,
            user_size: libc::size_t,
        ) -> libc::c_int;
    }

    let mut status = 0_u32;
    if unsafe {
        csops(
            pid,
            CS_OPS_STATUS,
            std::ptr::from_mut(&mut status).cast(),
            std::mem::size_of_val(&status),
        )
    } != 0
        || status & CS_VALID == 0
    {
        return false;
    }
    let mut child_hash = [0_u8; 20];
    if unsafe {
        csops(
            pid,
            CS_OPS_CDHASH,
            child_hash.as_mut_ptr().cast(),
            child_hash.len(),
        )
    } != 0
    {
        return false;
    }
    launch.code_directory_hashes.contains(&child_hash)
}

#[cfg(target_os = "macos")]
fn cloexec_pipe() -> Result<(OwnedFd, OwnedFd), MediaToolError> {
    let mut descriptors = [-1; 2];
    if unsafe { libc::pipe(descriptors.as_mut_ptr()) } != 0 {
        return Err(MediaToolError::Io(std::io::Error::last_os_error()));
    }
    let read = unsafe { OwnedFd::from_raw_fd(descriptors[0]) };
    let write = unsafe { OwnedFd::from_raw_fd(descriptors[1]) };
    for descriptor in [&read, &write] {
        if unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
            return Err(MediaToolError::Io(std::io::Error::last_os_error()));
        }
    }
    Ok((read, write))
}

#[cfg(target_os = "macos")]
fn posix_spawn_result(code: libc::c_int) -> Result<(), MediaToolError> {
    if code == 0 {
        Ok(())
    } else {
        Err(MediaToolError::Io(std::io::Error::from_raw_os_error(code)))
    }
}

fn cstring(bytes: &[u8]) -> Result<CString, MediaToolError> {
    CString::new(bytes).map_err(|_| {
        MediaToolError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "media tool argument contained a NUL byte",
        ))
    })
}

#[cfg(target_os = "macos")]
fn kill_and_reap_pid(pid: libc::pid_t) {
    let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
    let _ = wait_for_pid(pid);
}

#[cfg(target_os = "macos")]
fn wait_for_pid(pid: libc::pid_t) -> Result<ExitStatus, std::io::Error> {
    let mut status = 0;
    loop {
        let result = unsafe { libc::waitpid(pid, &mut status, 0) };
        if result == pid {
            return Ok(ExitStatus::from_raw(status));
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

fn create_launch_snapshot(
    bundle_file: &File,
    expected_sha256: &str,
    launch_directory: &Path,
    launch_name: &str,
) -> Result<LaunchExecutable, MediaToolError> {
    use std::{
        io::Write,
        os::unix::fs::{FileExt, PermissionsExt},
    };

    let path = launch_directory.join(launch_name);
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path)
        .map_err(|_| MediaToolError::RuntimeIntegrity)?;
    let mut offset = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = bundle_file
            .read_at(&mut buffer, offset)
            .map_err(|_| MediaToolError::RuntimeIntegrity)?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(|_| MediaToolError::RuntimeIntegrity)?;
        offset += read as u64;
    }
    output
        .sync_all()
        .map_err(|_| MediaToolError::RuntimeIntegrity)?;
    output
        .set_permissions(std::fs::Permissions::from_mode(0o500))
        .map_err(|_| MediaToolError::RuntimeIntegrity)?;
    drop(output);
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path)
        .map_err(|_| MediaToolError::RuntimeIntegrity)?;
    let metadata = file
        .metadata()
        .map_err(|_| MediaToolError::RuntimeIntegrity)?;
    if !metadata.is_file()
        || sha256_file_handle(&file).map_err(|_| MediaToolError::RuntimeIntegrity)?
            != expected_sha256
    {
        return Err(MediaToolError::RuntimeIntegrity);
    }
    let path = path
        .canonicalize()
        .map_err(|_| MediaToolError::RuntimeIntegrity)?;
    let mach_o = is_mach_o(&file).map_err(|_| MediaToolError::RuntimeIntegrity)?;
    #[cfg(target_os = "macos")]
    let code_directory_hashes = if mach_o {
        Arc::from(
            macho_code_directory_hashes(&file)
                .map_err(|_| MediaToolError::RuntimeIntegrity)?
                .into_boxed_slice(),
        )
    } else {
        Arc::from([])
    };
    Ok(LaunchExecutable {
        path,
        identity: file_identity(&metadata),
        file: Arc::new(file),
        mach_o,
        #[cfg(target_os = "macos")]
        code_directory_hashes,
    })
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
enum ByteOrder {
    Big,
    Little,
}

#[cfg(target_os = "macos")]
fn macho_code_directory_hashes(file: &File) -> Result<Vec<[u8; 20]>, std::io::Error> {
    use std::os::unix::fs::FileExt;

    const MAX_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;
    let length = file.metadata()?.len();
    if length == 0 || length > MAX_EXECUTABLE_BYTES || length > usize::MAX as u64 {
        return Err(std::io::Error::other(
            "media executable has an invalid size",
        ));
    }
    let mut bytes = vec![0_u8; length as usize];
    let mut offset = 0;
    while offset < bytes.len() {
        let read = file.read_at(&mut bytes[offset..], offset as u64)?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "media executable ended while reading its code signature",
            ));
        }
        offset += read;
    }

    let mut hashes = Vec::new();
    match bytes.get(..4) {
        Some([0xca, 0xfe, 0xba, 0xbe]) => {
            parse_fat_macho(&bytes, ByteOrder::Big, false, &mut hashes)?
        }
        Some([0xbe, 0xba, 0xfe, 0xca]) => {
            parse_fat_macho(&bytes, ByteOrder::Little, false, &mut hashes)?
        }
        Some([0xca, 0xfe, 0xba, 0xbf]) => {
            parse_fat_macho(&bytes, ByteOrder::Big, true, &mut hashes)?
        }
        Some([0xbf, 0xba, 0xfe, 0xca]) => {
            parse_fat_macho(&bytes, ByteOrder::Little, true, &mut hashes)?
        }
        _ => parse_thin_macho(&bytes, &mut hashes)?,
    }
    hashes.sort_unstable();
    hashes.dedup();
    if hashes.is_empty() {
        return Err(std::io::Error::other(
            "media executable has no supported code directory",
        ));
    }
    Ok(hashes)
}

#[cfg(target_os = "macos")]
fn parse_fat_macho(
    bytes: &[u8],
    byte_order: ByteOrder,
    is_64_bit: bool,
    hashes: &mut Vec<[u8; 20]>,
) -> Result<(), std::io::Error> {
    let architectures = read_u32(bytes, 4, byte_order)? as usize;
    let entry_size = if is_64_bit { 32 } else { 20 };
    let table_size = architectures
        .checked_mul(entry_size)
        .and_then(|size| size.checked_add(8))
        .ok_or_else(invalid_macho)?;
    if table_size > bytes.len() {
        return Err(invalid_macho());
    }
    for index in 0..architectures {
        let entry = 8 + index * entry_size;
        let (offset, length) = if is_64_bit {
            (
                read_u64(bytes, entry + 8, byte_order)?,
                read_u64(bytes, entry + 16, byte_order)?,
            )
        } else {
            (
                u64::from(read_u32(bytes, entry + 8, byte_order)?),
                u64::from(read_u32(bytes, entry + 12, byte_order)?),
            )
        };
        let start = usize::try_from(offset).map_err(|_| invalid_macho())?;
        let length = usize::try_from(length).map_err(|_| invalid_macho())?;
        let end = start.checked_add(length).ok_or_else(invalid_macho)?;
        let slice = bytes.get(start..end).ok_or_else(invalid_macho)?;
        parse_thin_macho(slice, hashes)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn parse_thin_macho(bytes: &[u8], hashes: &mut Vec<[u8; 20]>) -> Result<(), std::io::Error> {
    const LC_CODE_SIGNATURE: u32 = 0x1d;
    let (byte_order, header_size) = match bytes.get(..4) {
        Some([0xfe, 0xed, 0xfa, 0xce]) => (ByteOrder::Big, 28_usize),
        Some([0xce, 0xfa, 0xed, 0xfe]) => (ByteOrder::Little, 28_usize),
        Some([0xfe, 0xed, 0xfa, 0xcf]) => (ByteOrder::Big, 32_usize),
        Some([0xcf, 0xfa, 0xed, 0xfe]) => (ByteOrder::Little, 32_usize),
        _ => return Err(invalid_macho()),
    };
    let commands = read_u32(bytes, 16, byte_order)? as usize;
    let commands_size = read_u32(bytes, 20, byte_order)? as usize;
    let commands_end = header_size
        .checked_add(commands_size)
        .ok_or_else(invalid_macho)?;
    if commands_end > bytes.len() {
        return Err(invalid_macho());
    }
    let mut offset = header_size;
    for _ in 0..commands {
        let command = read_u32(bytes, offset, byte_order)?;
        let command_size = read_u32(bytes, offset + 4, byte_order)? as usize;
        if command_size < 8
            || offset
                .checked_add(command_size)
                .is_none_or(|end| end > commands_end)
        {
            return Err(invalid_macho());
        }
        if command == LC_CODE_SIGNATURE {
            if command_size < 16 {
                return Err(invalid_macho());
            }
            let signature_offset = read_u32(bytes, offset + 8, byte_order)? as usize;
            let signature_size = read_u32(bytes, offset + 12, byte_order)? as usize;
            let signature_end = signature_offset
                .checked_add(signature_size)
                .ok_or_else(invalid_macho)?;
            parse_embedded_signature(
                bytes
                    .get(signature_offset..signature_end)
                    .ok_or_else(invalid_macho)?,
                hashes,
            )?;
        }
        offset += command_size;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn parse_embedded_signature(
    bytes: &[u8],
    hashes: &mut Vec<[u8; 20]>,
) -> Result<(), std::io::Error> {
    const CSMAGIC_EMBEDDED_SIGNATURE: u32 = 0xfade_0cc0;
    const CSMAGIC_CODEDIRECTORY: u32 = 0xfade_0c02;
    if read_u32(bytes, 0, ByteOrder::Big)? != CSMAGIC_EMBEDDED_SIGNATURE {
        return Err(invalid_macho());
    }
    let length = read_u32(bytes, 4, ByteOrder::Big)? as usize;
    let count = read_u32(bytes, 8, ByteOrder::Big)? as usize;
    if length > bytes.len()
        || count
            .checked_mul(8)
            .and_then(|size| size.checked_add(12))
            .is_none_or(|end| end > length)
    {
        return Err(invalid_macho());
    }
    for index in 0..count {
        let blob_offset = read_u32(bytes, 12 + index * 8 + 4, ByteOrder::Big)? as usize;
        if read_u32(bytes, blob_offset, ByteOrder::Big)? != CSMAGIC_CODEDIRECTORY {
            continue;
        }
        let blob_length = read_u32(bytes, blob_offset + 4, ByteOrder::Big)? as usize;
        let blob_end = blob_offset
            .checked_add(blob_length)
            .ok_or_else(invalid_macho)?;
        let code_directory = bytes.get(blob_offset..blob_end).ok_or_else(invalid_macho)?;
        let hash_type = *code_directory.get(37).ok_or_else(invalid_macho)?;
        if matches!(hash_type, 2 | 3) {
            let digest = Sha256::digest(code_directory);
            let mut cdhash = [0_u8; 20];
            cdhash.copy_from_slice(&digest[..20]);
            hashes.push(cdhash);
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn read_u32(bytes: &[u8], offset: usize, byte_order: ByteOrder) -> Result<u32, std::io::Error> {
    let raw: [u8; 4] = bytes
        .get(offset..offset.checked_add(4).ok_or_else(invalid_macho)?)
        .ok_or_else(invalid_macho)?
        .try_into()
        .map_err(|_| invalid_macho())?;
    Ok(match byte_order {
        ByteOrder::Big => u32::from_be_bytes(raw),
        ByteOrder::Little => u32::from_le_bytes(raw),
    })
}

#[cfg(target_os = "macos")]
fn read_u64(bytes: &[u8], offset: usize, byte_order: ByteOrder) -> Result<u64, std::io::Error> {
    let raw: [u8; 8] = bytes
        .get(offset..offset.checked_add(8).ok_or_else(invalid_macho)?)
        .ok_or_else(invalid_macho)?
        .try_into()
        .map_err(|_| invalid_macho())?;
    Ok(match byte_order {
        ByteOrder::Big => u64::from_be_bytes(raw),
        ByteOrder::Little => u64::from_le_bytes(raw),
    })
}

#[cfg(target_os = "macos")]
fn invalid_macho() -> std::io::Error {
    std::io::Error::other("media executable has an invalid Mach-O code signature")
}

fn is_mach_o(file: &File) -> Result<bool, std::io::Error> {
    use std::os::unix::fs::FileExt;

    let mut magic = [0_u8; 4];
    let read = file.read_at(&mut magic, 0)?;
    Ok(read == magic.len()
        && matches!(
            magic,
            [0xfe, 0xed, 0xfa, 0xce]
                | [0xce, 0xfa, 0xed, 0xfe]
                | [0xfe, 0xed, 0xfa, 0xcf]
                | [0xcf, 0xfa, 0xed, 0xfe]
                | [0xca, 0xfe, 0xba, 0xbe]
                | [0xbe, 0xba, 0xfe, 0xca]
                | [0xca, 0xfe, 0xba, 0xbf]
                | [0xbf, 0xba, 0xfe, 0xca]
        ))
}

fn launch_path_matches(launch: &LaunchExecutable) -> bool {
    let metadata = match std::fs::symlink_metadata(&launch.path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };
    !metadata.file_type().is_symlink()
        && metadata.is_file()
        && file_identity(&metadata) == launch.identity
        && launch
            .file
            .metadata()
            .is_ok_and(|metadata| file_identity(&metadata) == launch.identity)
        && launch
            .path
            .canonicalize()
            .is_ok_and(|canonical| canonical == launch.path)
}

fn file_identity(metadata: &std::fs::Metadata) -> FileIdentity {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        FileIdentity {
            len: metadata.len(),
            modified_ns: i128::from(metadata.mtime()) * 1_000_000_000
                + i128::from(metadata.mtime_nsec()),
            device: metadata.dev(),
            inode: metadata.ino(),
            changed_ns: i128::from(metadata.ctime()) * 1_000_000_000
                + i128::from(metadata.ctime_nsec()),
        }
    }
    #[cfg(not(unix))]
    {
        FileIdentity {
            len: metadata.len(),
            modified_ns: metadata
                .modified()
                .ok()
                .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |duration| duration.as_nanos() as i128),
        }
    }
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
    use sha2::{Digest, Sha256};
    use std::{
        ffi::OsString,
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        process::Stdio,
        sync::LazyLock,
        time::Duration,
    };
    use tokio_util::sync::CancellationToken;

    use super::{
        BundledMediaTools, MediaFileIdentity, MediaFrameOutput, MediaToolError,
        collect_ffprobe_output, collect_suspended_process_output, read_limited,
        spawn_suspended_verified, validate_media_path_after_open,
    };
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
                "/dev/fd/0",
            ]
        );
    }

    #[tokio::test]
    async fn ffmpeg_frame_uses_only_fixed_arguments_and_the_identity_bound_input() {
        let _gate = FAKE_PROCESS_GATE.acquire().await.unwrap();
        let fixture = fake_tools("printf '%s\\n' \"$@\"");
        let media = canonical_media(fixture.path(), "frame.mp4");
        let identity = MediaFileIdentity::from_metadata(&fs::metadata(&media).unwrap());
        let output = fixture
            .tools()
            .video_frame_identity_bound(
                &media,
                &identity,
                1_250_000,
                MediaFrameOutput::Png320,
                CancellationToken::new(),
            )
            .await
            .unwrap();

        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            [
                "-v",
                "error",
                "-ss",
                "1.250000",
                "-i",
                "/dev/fd/0",
                "-map",
                "0:v:0",
                "-frames:v",
                "1",
                "-an",
                "-sn",
                "-dn",
                "-vf",
                "scale=320:-2:flags=lanczos",
                "-f",
                "image2pipe",
                "-c:v",
                "png",
                "pipe:1",
            ]
        );
    }

    #[tokio::test]
    async fn ffmpeg_frame_rejects_source_replacement_before_launch() {
        let fixture = fake_tools("exit 0");
        let media = canonical_media(fixture.path(), "source-change.mp4");
        let identity = MediaFileIdentity::from_metadata(&fs::metadata(&media).unwrap());
        fs::rename(&media, media.with_extension("old")).unwrap();
        fs::write(&media, b"replacement").unwrap();

        assert!(matches!(
            fixture
                .tools()
                .video_frame_identity_bound(
                    &media,
                    &identity,
                    0,
                    MediaFrameOutput::Gray160,
                    CancellationToken::new(),
                )
                .await,
            Err(MediaToolError::InputChanged)
        ));
    }

    #[tokio::test]
    async fn ffmpeg_frame_executes_the_verified_handle_after_path_replacement() {
        let _gate = FAKE_PROCESS_GATE.acquire().await.unwrap();
        let fixture = fake_tools("printf verified-executable");
        let tools = fixture.tools();
        let media = canonical_media(fixture.path(), "verified-exec.mp4");
        let identity = MediaFileIdentity::from_metadata(&fs::metadata(&media).unwrap());
        let ffmpeg = fixture.ffmpeg_path().to_path_buf();

        let output = tools
            .video_frame_identity_bound_with_before_spawn_for_test(
                &media,
                &identity,
                0,
                MediaFrameOutput::Png320,
                CancellationToken::new(),
                || {
                    fs::rename(&ffmpeg, ffmpeg.with_extension("verified")).unwrap();
                    fs::write(&ffmpeg, "#!/bin/sh\nprintf path-replacement\n").unwrap();
                    fs::set_permissions(&ffmpeg, fs::Permissions::from_mode(0o755)).unwrap();
                },
            )
            .await
            .unwrap();

        assert_eq!(output.stdout, b"verified-executable");
    }

    #[tokio::test]
    async fn ffmpeg_frame_rejects_snapshot_replacement_after_final_validation() {
        let _gate = FAKE_PROCESS_GATE.acquire().await.unwrap();
        let fixture = fake_macho_tools();
        let tools = fixture.tools();
        let media = canonical_media(fixture.path(), "snapshot-swap.mp4");
        let identity = MediaFileIdentity::from_metadata(&fs::metadata(&media).unwrap());
        let snapshot = tools.ffmpeg_launch.path.clone();

        let result = tools
            .video_frame_identity_bound_with_after_final_validation_for_test(
                &media,
                &identity,
                0,
                MediaFrameOutput::Png320,
                CancellationToken::new(),
                || {
                    fs::rename(&snapshot, snapshot.with_extension("verified")).unwrap();
                    fs::copy("/usr/bin/false", &snapshot).unwrap();
                },
            )
            .await;

        assert!(
            matches!(result, Err(MediaToolError::ExecutableChanged)),
            "unexpected result: {result:?}"
        );
    }

    #[tokio::test]
    async fn ffmpeg_frame_rejects_snapshot_replacement_restored_after_pidpath() {
        let _gate = FAKE_PROCESS_GATE.acquire().await.unwrap();
        let fixture = fake_macho_tools();
        let tools = fixture.tools();
        let media = canonical_media(fixture.path(), "snapshot-aba.mp4");
        let identity = MediaFileIdentity::from_metadata(&fs::metadata(&media).unwrap());
        let snapshot = tools.ffmpeg_launch.path.clone();
        let launch_directory = snapshot.parent().unwrap().to_path_buf();
        let reviewed_directory = launch_directory.with_extension("reviewed");
        let replacement_directory = launch_directory.with_extension("replacement");
        let swap_directory = launch_directory.clone();
        let swap_reviewed_directory = reviewed_directory.clone();
        let restore_directory = launch_directory.clone();
        let selected_snapshot = snapshot.clone();

        let result = tools
            .video_frame_identity_bound_with_spawn_aba_for_test(
                &media,
                &identity,
                CancellationToken::new(),
                move || {
                    fs::rename(&swap_directory, &swap_reviewed_directory).unwrap();
                    fs::create_dir(&swap_directory).unwrap();
                    fs::copy("/usr/bin/false", swap_directory.join("ffmpeg")).unwrap();
                },
                |_| {},
                move |selected_path| {
                    assert_eq!(selected_path, selected_snapshot);
                    fs::rename(&restore_directory, replacement_directory).unwrap();
                    fs::rename(reviewed_directory, restore_directory).unwrap();
                },
            )
            .await;

        assert!(
            matches!(result, Err(MediaToolError::ExecutableChanged)),
            "unexpected ABA result: {result:?}"
        );
    }

    #[tokio::test]
    async fn suspended_macho_cancellation_kills_and_reaps_the_child() {
        let _gate = FAKE_PROCESS_GATE.acquire().await.unwrap();
        let fixture = fake_macho_helper_tools();
        let tools = fixture.tools();
        let media = canonical_media(fixture.path(), "suspended-cancel.mp4");
        let process = spawn_suspended_verified(
            &tools.ffmpeg_launch,
            &tools.ffmpeg,
            &[OsString::from("100")],
            fs::File::open(media).unwrap(),
            || {},
            |_| {},
            |_| {},
        )
        .unwrap();
        let pid = process.pid;
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let result = collect_suspended_process_output(
            process,
            cancellation,
            tokio::time::Instant::now() + Duration::from_secs(1),
            128,
            128,
        )
        .await;

        assert!(matches!(result, Err(MediaToolError::Cancelled)));
        assert_process_is_gone(pid);
    }

    #[tokio::test]
    async fn suspended_macho_timeout_kills_and_reaps_the_child() {
        let _gate = FAKE_PROCESS_GATE.acquire().await.unwrap();
        let fixture = fake_macho_helper_tools();
        let tools = fixture.tools();
        let media = canonical_media(fixture.path(), "suspended-timeout.mp4");
        let process = spawn_suspended_verified(
            &tools.ffmpeg_launch,
            &tools.ffmpeg,
            &[OsString::from("100")],
            fs::File::open(media).unwrap(),
            || {},
            |_| {},
            |_| {},
        )
        .unwrap();
        let pid = process.pid;

        let result = collect_suspended_process_output(
            process,
            CancellationToken::new(),
            tokio::time::Instant::now(),
            128,
            128,
        )
        .await;

        assert!(matches!(result, Err(MediaToolError::TimedOut)));
        assert_process_is_gone(pid);
    }

    #[tokio::test]
    async fn suspended_macho_pipe_overflow_kills_and_reaps_the_child() {
        let _gate = FAKE_PROCESS_GATE.acquire().await.unwrap();
        let fixture = fake_macho_helper_tools();
        let tools = fixture.tools();
        let media = canonical_media(fixture.path(), "suspended-overflow.mp4");
        let process = spawn_suspended_verified(
            &tools.ffmpeg_launch,
            &tools.ffmpeg,
            &[],
            fs::File::open(media).unwrap(),
            || {},
            |_| {},
            |_| {},
        )
        .unwrap();
        let pid = process.pid;

        let result = collect_suspended_process_output(
            process,
            CancellationToken::new(),
            tokio::time::Instant::now() + Duration::from_secs(1),
            128,
            128,
        )
        .await;

        assert!(
            matches!(result, Err(MediaToolError::StdoutTooLarge)),
            "unexpected result: {result:?}"
        );
        assert_process_is_gone(pid);
    }

    #[test]
    fn cancellation_and_timeout_diagnostics_are_media_tool_neutral() {
        assert_eq!(
            MediaToolError::Cancelled.to_string(),
            "bundled media tool was cancelled"
        );
        assert_eq!(
            MediaToolError::TimedOut.to_string(),
            "bundled media tool timed out"
        );
    }

    #[tokio::test]
    async fn cancellation_kills_and_awaits_the_ffmpeg_frame_child() {
        let _gate = FAKE_PROCESS_GATE.acquire().await.unwrap();
        let fixture = fake_tools("echo $$ > \"$0.pid\"; exec /usr/bin/tail -f /dev/null");
        let media = canonical_media(fixture.path(), "cancel-frame.mp4");
        let identity = MediaFileIdentity::from_metadata(&fs::metadata(&media).unwrap());
        let cancellation = CancellationToken::new();
        let tools = fixture.tools();
        let pid_path = tools.ffmpeg_launch.path.with_extension("pid");
        let task = {
            let cancellation = cancellation.clone();
            tokio::spawn(async move {
                tools
                    .video_frame_identity_bound(
                        &media,
                        &identity,
                        0,
                        MediaFrameOutput::Png640,
                        cancellation,
                    )
                    .await
            })
        };
        let pid = wait_for_pid(&pid_path).await;

        cancellation.cancel();
        let result = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("cancelled ffmpeg must be killed and awaited")
            .unwrap();

        assert!(matches!(result, Err(MediaToolError::Cancelled)));
        assert_process_is_gone(pid);
    }

    #[test]
    fn bundled_ffprobe_rejects_a_symlink_entry() {
        use std::os::unix::fs::symlink;

        let fixture = fake_tools("exit 0");
        fs::remove_file(fixture.ffprobe_path()).unwrap();
        symlink("/bin/sh", fixture.ffprobe_path()).unwrap();

        assert!(matches!(
            BundledMediaTools::from_layout(&fixture.layout),
            Err(MediaToolError::UnsafeExecutable)
        ));
    }

    #[tokio::test]
    async fn bundled_ffprobe_rejects_replacement_after_identity_binding() {
        let fixture = fake_tools("exit 0");
        let tools = fixture.tools();
        fs::rename(
            fixture.ffprobe_path(),
            fixture.ffprobe_path().with_extension("old"),
        )
        .unwrap();
        fs::write(fixture.ffprobe_path(), "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(fixture.ffprobe_path(), fs::Permissions::from_mode(0o755)).unwrap();
        let media = canonical_media(fixture.path(), "replacement.mp4");

        assert!(matches!(
            tools.ffprobe_json(&media, CancellationToken::new()).await,
            Err(MediaToolError::ExecutableChanged)
        ));
    }

    #[tokio::test]
    async fn ffprobe_reads_the_verified_open_file_after_path_replacement() {
        let _gate = FAKE_PROCESS_GATE.acquire().await.unwrap();
        let fixture = fake_tools("echo $$ > \"$0.pid\"; /bin/sleep 0.1; /bin/cat /dev/fd/0");
        let media = canonical_media(fixture.path(), "identity.mp4");
        fs::write(&media, b"original-open-file").unwrap();
        let tools = fixture.tools();
        let pid_path = tools.ffprobe_launch.path.with_extension("pid");
        let task = {
            let media = media.clone();
            tokio::spawn(async move { tools.ffprobe_json(&media, CancellationToken::new()).await })
        };
        let _pid = wait_for_pid(&pid_path).await;
        fs::rename(&media, media.with_extension("old")).unwrap();
        fs::write(&media, b"replacement-path-file").unwrap();

        let output = task.await.unwrap().unwrap();
        assert_eq!(output.stdout, b"original-open-file");
    }

    #[test]
    fn bundled_ffprobe_requires_the_packaged_integrity_contract() {
        let fixture = fake_tools("exit 0");
        fs::remove_file(&fixture.layout.manifest).ok();
        fs::remove_file(fixture.layout.root.join("runtime.inventory.sha256")).ok();

        assert!(matches!(
            BundledMediaTools::from_layout(&fixture.layout),
            Err(MediaToolError::RuntimeIntegrity)
        ));
    }

    #[test]
    fn bundled_ffprobe_rejects_inventory_digest_that_does_not_match_entry() {
        let fixture = fake_tools("exit 0");
        fs::write(
            fixture.layout.root.join("runtime.inventory.sha256"),
            format!("{}  bin/ffprobe\n", "0".repeat(64)),
        )
        .unwrap();

        assert!(matches!(
            BundledMediaTools::from_layout(&fixture.layout),
            Err(MediaToolError::RuntimeIntegrity)
        ));
    }

    #[test]
    fn opened_media_fd_rejects_temporary_path_replacement_aba() {
        let fixture = fake_tools("exit 0");
        let media = canonical_media(fixture.path(), "aba.mp4");
        fs::write(&media, b"task-4-original-identity").unwrap();
        let expected = MediaFileIdentity::from_metadata(&fs::metadata(&media).unwrap());
        let original = media.with_extension("original");
        fs::rename(&media, &original).unwrap();
        fs::write(&media, b"replacement-probe-content").unwrap();

        let result = validate_media_path_after_open(&media, Some(&expected), || {
            fs::remove_file(&media).unwrap();
            fs::rename(&original, &media).unwrap();
        });

        assert!(matches!(result, Err(MediaToolError::InputChanged)));
        assert_eq!(fs::read(&media).unwrap(), b"task-4-original-identity");
    }

    #[tokio::test]
    async fn ffprobe_json_enforces_independent_stdout_and_stderr_bounds() {
        let _gate = FAKE_PROCESS_GATE.acquire().await.unwrap();
        let stdout_fixture = fake_tools("echo $$ > \"$0.pid\"; exec /usr/bin/yes stdout-overflow");
        let media = canonical_media(stdout_fixture.path(), "stdout.mp4");
        let stdout_tools = stdout_fixture.tools();
        let stdout_pid_path = stdout_tools.ffprobe_launch.path.with_extension("pid");
        assert!(matches!(
            stdout_tools
                .ffprobe_json(&media, CancellationToken::new())
                .await,
            Err(MediaToolError::StdoutTooLarge)
        ));
        let pid = wait_for_pid(&stdout_pid_path).await;
        assert_process_is_gone(pid);

        let stderr_fixture =
            fake_tools("echo $$ > \"$0.pid\"; exec /usr/bin/yes stderr-overflow >&2");
        let media = canonical_media(stderr_fixture.path(), "stderr.mp4");
        let stderr_tools = stderr_fixture.tools();
        let stderr_pid_path = stderr_tools.ffprobe_launch.path.with_extension("pid");
        assert!(matches!(
            stderr_tools
                .ffprobe_json(&media, CancellationToken::new())
                .await,
            Err(MediaToolError::StderrTooLarge)
        ));
        let pid = wait_for_pid(&stderr_pid_path).await;
        assert_process_is_gone(pid);
    }

    #[tokio::test]
    async fn cancellation_kills_and_awaits_the_ffprobe_child() {
        let _gate = FAKE_PROCESS_GATE.acquire().await.unwrap();
        let fixture = fake_tools("echo $$ > \"$0.pid\"; exec /usr/bin/tail -f /dev/null");
        let media = canonical_media(fixture.path(), "cancel.mp4");
        let cancellation = CancellationToken::new();
        let tools = fixture.tools();
        let pid_path = tools.ffprobe_launch.path.with_extension("pid");
        let task = {
            let cancellation = cancellation.clone();
            tokio::spawn(async move { tools.ffprobe_json(&media, cancellation).await })
        };
        let pid = wait_for_pid(&pid_path).await;

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
        let tools = fixture.tools();
        let pid_path = tools.ffprobe_launch.path.with_extension("pid");

        let result = tools
            .ffprobe_json_with_timeout(
                &media,
                None,
                CancellationToken::new(),
                Duration::from_secs(1),
            )
            .await;
        let pid = wait_for_pid(&pid_path).await;

        assert!(matches!(result, Err(MediaToolError::TimedOut)));
        assert_process_is_gone(pid);
    }

    #[tokio::test]
    async fn missing_pipe_after_spawn_still_kills_and_awaits_child() {
        let _gate = FAKE_PROCESS_GATE.acquire().await.unwrap();
        let directory = tempfile::tempdir().unwrap();
        let pid_path = directory.path().join("missing-pipe.pid");
        let mut child = tokio::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("echo $$ > \"$1\"; exec /usr/bin/tail -f /dev/null")
            .arg("viewer-missing-pipe")
            .arg(&pid_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let pid = wait_for_pid(&pid_path).await;

        assert!(matches!(
            collect_ffprobe_output(
                &mut child,
                CancellationToken::new(),
                tokio::time::Instant::now() + Duration::from_secs(1),
            )
            .await,
            Err(MediaToolError::Io(_))
        ));
        assert_process_is_gone(pid);
    }

    #[tokio::test]
    async fn raw_ffprobe_overflow_still_kills_and_awaits_child() {
        let _gate = FAKE_PROCESS_GATE.acquire().await.unwrap();
        let fixture = fake_tools("echo $$ > \"$0.pid\"; exec /usr/bin/yes raw-overflow");
        let tools = fixture.tools();
        let pid_path = tools.ffprobe_launch.path.with_extension("pid");
        let result = tools.ffprobe(&[]).await;
        let pid = wait_for_pid(&pid_path).await;

        assert!(matches!(result, Err(MediaToolError::OutputTooLarge)));
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

        fn ffmpeg_path(&self) -> &Path {
            &self.layout.ffmpeg
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
        let ffmpeg = root.join("bin/ffmpeg");
        fs::write(&ffprobe, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::write(&ffmpeg, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&ffprobe, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&ffmpeg, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(
            root.join("runtime.lock.json"),
            r#"{"schemaVersion":1,"target":"universal-apple-darwin","mpv":{"tag":"v0.41.0","commit":"41f6a64","mesonOptions":{}},"ffmpeg":{"tag":"n8.0","configureOptions":["--disable-gpl","--disable-nonfree","--disable-network","--disable-ffplay","--enable-zlib","--enable-encoder=png"]},"components":[]}"#,
        )
        .unwrap();
        let digest = format!("{:x}", Sha256::digest(fs::read(&ffprobe).unwrap()));
        let ffmpeg_digest = format!("{:x}", Sha256::digest(fs::read(&ffmpeg).unwrap()));
        fs::write(
            root.join("runtime.inventory.sha256"),
            format!("{ffmpeg_digest}  bin/ffmpeg\n{digest}  bin/ffprobe\n"),
        )
        .unwrap();
        let layout = RuntimeLayout {
            libmpv: root.join("lib/libmpv.2.dylib"),
            ffmpeg,
            ffprobe,
            manifest: root.join("runtime.lock.json"),
            licenses: root.join("licenses"),
            root,
        };
        FakeTools { directory, layout }
    }

    fn fake_macho_tools() -> FakeTools {
        fake_macho_tools_with_executable("/bin/echo")
    }

    fn fake_macho_helper_tools() -> FakeTools {
        let build_directory = tempfile::tempdir().unwrap();
        let source = build_directory.path().join("helper.c");
        let executable = build_directory.path().join("helper");
        fs::write(
            &source,
            r#"#include <string.h>
#include <unistd.h>
int main(int argc, char **argv) {
  if (argc > 1) { for (;;) pause(); }
  char bytes[512]; memset(bytes, 'x', sizeof(bytes));
  for (;;) { if (write(1, bytes, sizeof(bytes)) < 0) return 1; }
}"#,
        )
        .unwrap();
        let status = std::process::Command::new("/usr/bin/clang")
            .args(["-O0", "-o"])
            .arg(&executable)
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());
        fake_macho_tools_with_executable(executable.to_str().unwrap())
    }

    fn fake_macho_tools_with_executable(executable: &str) -> FakeTools {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("ViewerVideoRuntime");
        fs::create_dir_all(root.join("bin")).unwrap();
        let ffprobe = root.join("bin/ffprobe");
        let ffmpeg = root.join("bin/ffmpeg");
        fs::copy(executable, &ffprobe).unwrap();
        fs::copy(executable, &ffmpeg).unwrap();
        fs::write(
            root.join("runtime.lock.json"),
            r#"{"schemaVersion":1,"target":"universal-apple-darwin","mpv":{"tag":"v0.41.0","commit":"41f6a64","mesonOptions":{}},"ffmpeg":{"tag":"n8.0","configureOptions":["--disable-gpl","--disable-nonfree","--disable-network","--disable-ffplay","--enable-zlib","--enable-encoder=png"]},"components":[]}"#,
        )
        .unwrap();
        let digest = format!("{:x}", Sha256::digest(fs::read(&ffprobe).unwrap()));
        let ffmpeg_digest = format!("{:x}", Sha256::digest(fs::read(&ffmpeg).unwrap()));
        fs::write(
            root.join("runtime.inventory.sha256"),
            format!("{ffmpeg_digest}  bin/ffmpeg\n{digest}  bin/ffprobe\n"),
        )
        .unwrap();
        let layout = RuntimeLayout {
            libmpv: root.join("lib/libmpv.2.dylib"),
            ffmpeg,
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
