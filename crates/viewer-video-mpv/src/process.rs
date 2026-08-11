use std::{
    ffi::OsString,
    fs::File,
    io::Read,
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
const FFPROBE_ENTRIES: &str = "stream=codec_type,codec_name,width,height,avg_frame_rate,r_frame_rate:stream_tags=rotate:stream_side_data=rotation:format=duration";

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
    ffprobe: PathBuf,
    ffprobe_identity: FileIdentity,
    ffprobe_sha256: String,
    gate: Arc<Semaphore>,
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
        let (ffprobe, ffprobe_identity, ffprobe_sha256) = bind_ffprobe(layout)?;
        Ok(Self {
            ffmpeg: layout.ffmpeg.clone(),
            ffprobe,
            ffprobe_identity,
            ffprobe_sha256,
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
        }
        let mut child = Command::new(executable)
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

        let mut child = Command::new(&self.ffprobe)
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

    fn revalidate_ffprobe(&self) -> Result<(), MediaToolError> {
        self.revalidate_ffprobe_identity()?;
        if sha256_file(&self.ffprobe).map_err(|_| MediaToolError::ExecutableChanged)?
            != self.ffprobe_sha256
        {
            return Err(MediaToolError::ExecutableChanged);
        }
        self.revalidate_ffprobe_identity()
    }

    fn revalidate_ffprobe_identity(&self) -> Result<(), MediaToolError> {
        let metadata = std::fs::symlink_metadata(&self.ffprobe)
            .map_err(|_| MediaToolError::ExecutableChanged)?;
        let valid = !metadata.file_type().is_symlink()
            && metadata.is_file()
            && file_identity(&metadata) == self.ffprobe_identity
            && self
                .ffprobe
                .canonicalize()
                .is_ok_and(|canonical| canonical == self.ffprobe);
        valid.then_some(()).ok_or(MediaToolError::ExecutableChanged)
    }
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

fn bind_ffprobe(layout: &RuntimeLayout) -> Result<(PathBuf, FileIdentity, String), MediaToolError> {
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
        std::fs::symlink_metadata(&layout.ffprobe).map_err(|_| MediaToolError::UnsafeExecutable)?;
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
    let canonical_ffprobe = layout
        .ffprobe
        .canonicalize()
        .map_err(|_| MediaToolError::UnsafeExecutable)?;
    if canonical_ffprobe != canonical_root.join("bin/ffprobe") {
        return Err(MediaToolError::UnsafeExecutable);
    }
    let ffprobe_sha256 = validate_runtime_integrity(layout, &canonical_root)?;
    Ok((
        canonical_ffprobe,
        file_identity(&executable_metadata),
        ffprobe_sha256,
    ))
}

fn validate_runtime_integrity(
    layout: &RuntimeLayout,
    canonical_root: &Path,
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
        .filter(|line| line.ends_with("  bin/ffprobe"))
        .collect::<Vec<_>>();
    if entries.len() != 1 || entries[0].len() != 64 + "  bin/ffprobe".len() {
        return Err(MediaToolError::RuntimeIntegrity);
    }
    let expected = &entries[0][..64];
    if !expected
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || sha256_file(&layout.ffprobe).map_err(|_| MediaToolError::RuntimeIntegrity)? != expected
    {
        return Err(MediaToolError::RuntimeIntegrity);
    }
    Ok(expected.to_owned())
}

fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
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
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        process::Stdio,
        sync::LazyLock,
        time::Duration,
    };
    use tokio_util::sync::CancellationToken;

    use super::{
        BundledMediaTools, MediaFileIdentity, MediaToolError, collect_ffprobe_output, read_limited,
        validate_media_path_after_open,
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
        let task = {
            let tools = fixture.tools();
            let media = media.clone();
            tokio::spawn(async move { tools.ffprobe_json(&media, CancellationToken::new()).await })
        };
        let _pid = wait_for_pid(&fixture.ffprobe_path().with_extension("pid")).await;
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
            .ffprobe_json_with_timeout(
                &media,
                None,
                CancellationToken::new(),
                Duration::from_secs(1),
            )
            .await;
        let pid = wait_for_pid(&fixture.ffprobe_path().with_extension("pid")).await;

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
        let result = fixture.tools().ffprobe(&[]).await;
        let pid = wait_for_pid(&fixture.ffprobe_path().with_extension("pid")).await;

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
        fs::write(
            root.join("runtime.lock.json"),
            r#"{"schemaVersion":1,"target":"universal-apple-darwin","mpv":{"tag":"v0.41.0","commit":"41f6a64","mesonOptions":{}},"ffmpeg":{"tag":"n8.0","configureOptions":["--disable-gpl","--disable-nonfree","--disable-network","--disable-ffplay"]},"components":[]}"#,
        )
        .unwrap();
        let digest = format!("{:x}", Sha256::digest(fs::read(&ffprobe).unwrap()));
        fs::write(
            root.join("runtime.inventory.sha256"),
            format!("{digest}  bin/ffprobe\n"),
        )
        .unwrap();
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
