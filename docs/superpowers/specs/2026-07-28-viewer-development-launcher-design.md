# Viewer Development Launcher Design

**Date:** 2026-07-28
**Status:** Approved for written-spec review

## Goal

Provide one project-owned command for opening Viewer during development. Every
launch must use the current local working tree, including uncommitted changes,
and must not accidentally open an installed, packaged, or stale worktree build.

The command will be:

```bash
pnpm start:viewer
```

## Scope

The launcher will:

- resolve the repository root from the launcher file rather than the caller's
  current directory;
- leave Git state untouched: no fetch, pull, checkout, clean, stash, commit, or
  reset;
- stop an existing Viewer development session before starting a replacement;
- start `pnpm tauri dev` from the repository root;
- run the development session in the background;
- write launcher output to `target/dev-launcher/tauri-dev.log`;
- wait until the exact repository executable
  `target/debug/viewer-desktop` is running;
- return success only after that executable has been verified;
- report a useful error and the end of the log if startup fails or times out;
- document the command as the canonical development launch path.

The launcher will not install or package Viewer, synchronize with a remote Git
repository, choose a branch, or modify source files.

## Architecture

### Package command

The root `package.json` will expose `start:viewer`. This keeps the command easy
to remember and ensures it uses the repository's pinned pnpm toolchain.

### Node launcher

`scripts/start-viewer-dev.mjs` will own orchestration. Node is preferred over a
shell-only implementation because the repository already depends on Node, and
process selection, detached execution, timeouts, state files, and automated
tests are clearer and safer in JavaScript.

The launcher will derive all paths from its own file location. It will create
`target/dev-launcher/` as runtime-only state and store:

- `tauri-dev.log`: stdout and stderr from `pnpm tauri dev`;
- `session.json`: the detached launcher's process identifier, repository path,
  and launch time.

These files remain under the ignored `target/` tree and are not source
artifacts.

### Existing-process cleanup

The launcher will first inspect the stored session. It will terminate the saved
detached process group only when the live process still matches a Viewer Tauri
development command, preventing an obsolete PID file from targeting a reused
PID.

It will then inspect running `viewer-desktop` processes. A process is eligible
for termination only when its executable belongs to:

- this repository or one of its `.worktrees` directories; or
- a macOS `Viewer.app` bundle.

For a repository development process, the launcher will identify and terminate
the nearest `pnpm tauri dev` ancestor and its descendants. For a packaged
Viewer process without that ancestor, it will terminate only the exact
`viewer-desktop` process. It will wait for termination before starting the new
session and fail rather than launching a second copy if cleanup cannot be
confirmed.

The launcher will not kill an arbitrary process merely because it owns the
Vite port.

### Startup and readiness

The launcher will spawn `pnpm tauri dev` detached from the invoking terminal,
with the repository root as its working directory and output redirected to the
log file. It will record the child process only after spawn succeeds.

Readiness is defined by the process table containing the exact executable path:

```text
<repository>/target/debug/viewer-desktop
```

The launcher will poll for readiness for up to 120 seconds. It will also detect
early launcher exit. On timeout or early failure it will stop the newly created
session, remove stale session state, show the final log lines, and exit
non-zero.

On success it will print the verified executable path, source branch and short
commit for information only, and the log path. Dirty working-tree state is
allowed and will be reported without being changed.

## Error handling

The command will fail with a concise diagnostic when:

- `pnpm`, `package.json`, or `src-tauri` cannot be found;
- an existing Viewer session cannot be stopped;
- the detached Tauri command cannot be spawned;
- the Tauri command exits before the desktop executable appears;
- the exact development executable is not observed within 120 seconds.

Failures preserve the log for diagnosis. A stale state file is ignored when its
PID no longer exists or no longer matches the expected development command.

## Testing

Node's built-in test runner will cover production process-selection and command
construction logic before implementation:

- repository paths and launch commands are derived deterministically;
- current-repository and `.worktrees` Viewer processes are selected;
- packaged `Viewer.app` processes are selected;
- unrelated processes and reused PIDs are rejected;
- the correct development executable is required for readiness;
- ancestor selection targets the development launcher rather than unrelated
  parent processes.

After unit tests pass, a live integration check will:

1. run `pnpm start:viewer`;
2. verify the command returns successfully;
3. verify exactly the repository's `target/debug/viewer-desktop` is running;
4. run the command again;
5. verify the first session was replaced and the new development executable is
   running;
6. confirm the Git working tree was not altered by the launcher.

## Documentation and future convention

README's “启动开发版” section will use `pnpm start:viewer` as the canonical
command and explain that it launches the current local source, including
uncommitted changes.

For this project, a user request to “启动软件” means running
`pnpm start:viewer` from the project. Packaged `Viewer.app` copies are never the
development launch target.
