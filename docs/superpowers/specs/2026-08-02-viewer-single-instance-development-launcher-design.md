# Viewer Single-Instance Development Launcher Design

**Date:** 2026-08-02  
**Status:** Approved design; awaiting written-spec review

## Goal

Make `pnpm start:viewer` the only development launch path and guarantee that a
successful invocation leaves exactly one Viewer desktop process for the current
repository. Repeated launches must replace the prior development session rather
than add a second Viewer window.

This design corrects the development workflow only. It does not add a
product-level single-instance policy to packaged macOS or future Windows builds.

## Root Cause

The repository launcher correctly started one `pnpm tauri dev` session. A second
instance appeared later because a runtime-only bundle at
`target/dev-launcher/current-dev-wrapper/Viewer.app` had been registered with
macOS and was opened by bundle identifier to bring Viewer to the foreground.
That bundle linked to the same `target/debug/viewer-desktop` binary, so the two
windows showed the same source while belonging to separate processes.

The failure came from mixing two launch paths:

1. the approved repository launcher, which starts `pnpm tauri dev`; and
2. a temporary `Viewer.app` wrapper opened independently by macOS LaunchServices.

## Existing Contract

The approved development launcher design remains authoritative:

- `pnpm start:viewer` launches the current local working tree, including
  uncommitted changes;
- the launcher does not fetch, pull, switch branches, install, or package Viewer;
- `pnpm tauri dev` remains responsible for the native development process and
  hot reload;
- packaged or temporary `Viewer.app` bundles are not development launch targets;
- Git state must remain untouched.

This correction strengthens that contract without replacing the launcher
architecture.

## Selected Approach

Use defensive cleanup and exact process-count verification in the existing Node
launcher.

Two alternatives were rejected:

- changing only the operator habit would not protect against a leftover wrapper;
- adding a product-level process lock would exceed the requested scope and change
  formal application behavior.

## Architecture

### Canonical entry point

The root command remains:

```bash
pnpm start:viewer
```

No secondary application-opening command may run after this command succeeds.
The Tauri development launch itself is responsible for displaying the window.
Automation may inspect the process table and log, but it must not open Viewer by
bundle identifier or by a `Viewer.app` path.

### Runtime artifact cleanup

`buildLauncherPaths` will expose one additional path:

```text
<repo>/target/dev-launcher/current-dev-wrapper
```

Before process discovery and before a new session is spawned, the system runtime
will remove only this exact directory. The directory is an ignored runtime
artifact created by the superseded workflow; removing it cannot affect source,
project data, an installed Viewer copy, or another worktree.

Missing-path cleanup is successful. Any other cleanup failure aborts the launch
before a new Viewer process is created.

### Existing-process replacement

The launcher keeps the current safety rules:

1. validate any stored detached-session identity before signaling it;
2. stop the validated Tauri development process group;
3. stop other eligible repository, worktree, or packaged Viewer processes;
4. verify no eligible Viewer process remains;
5. spawn one detached `pnpm tauri dev` session from the current repository.

The cleanup order ensures a legacy wrapper process is stopped even when its
command resolves to `target/debug/viewer-desktop`.

### Exact readiness

Readiness changes from “at least one exact executable exists” to the following
invariant:

```text
exact current-repository Viewer process count == 1
eligible Viewer process count == 1
```

The exact current-repository process is the command whose executable is:

```text
<repo>/target/debug/viewer-desktop
```

The eligible count uses the existing `isViewerExecutable` boundary and therefore
also detects an accidentally started worktree or `Viewer.app` process.

If the exact count is zero, the launcher continues polling until the existing
120-second deadline. If either count exceeds one, or an eligible non-current
Viewer appears, startup fails immediately. The newly created Tauri process group
is stopped, session state is cleared, and the duplicate-process diagnostic is
reported.

### Launch result

The successful result will include the verified Viewer process identifier in
addition to the detached launcher process identifier, executable path, and log
path. CLI output will report the Viewer PID so later verification does not need
to rediscover or reopen the application.

## Component Changes

### `scripts/viewer-dev-launcher.mjs`

- add the exact legacy-wrapper path to `LauncherPaths`;
- add runtime cleanup for that exact path;
- count exact and eligible Viewer processes during readiness;
- fail safely when the single-instance invariant is violated;
- return the verified Viewer PID.

### `scripts/start-viewer-dev.mjs`

- report the verified Viewer PID;
- keep branch, commit, dirty-state, executable, and log reporting unchanged.

### `scripts/viewer-dev-launcher.test.mjs`

- define the single-instance behavior before production changes;
- cover the runtime-only wrapper cleanup boundary;
- cover duplicate and foreign eligible process rejection;
- cover the verified Viewer PID in the launch result and CLI output;
- retain all stale-session and process-selection safety tests.

### `README.md`

- state that `pnpm start:viewer` leaves one current-repository Viewer instance;
- state that development automation must not reopen Viewer through a bundle
  identifier or temporary application wrapper.

## Error Handling

The launcher will fail without starting a replacement when legacy-wrapper
cleanup fails.

After spawning, a duplicate-process failure will:

1. stop the new detached Tauri process group;
2. clear `session.json`;
3. preserve `tauri-dev.log`;
4. report the observed exact and eligible process counts.

This preserves the existing rule that uncertainty produces a failure instead of
another Viewer instance.

## Testing Strategy

### Automated red-green tests

Node tests will first demonstrate these missing behaviors:

- path derivation includes the exact runtime-wrapper directory;
- the system runtime removes that directory without broad deletion;
- readiness returns only when exactly one eligible process is the current
  repository executable;
- two exact repository processes fail startup;
- one current executable plus one worktree or packaged Viewer fails startup;
- successful results and CLI output contain the verified Viewer PID;
- duplicate failure stops the new group and clears session state.

### Regression suite

Run:

```bash
node --test scripts/viewer-dev-launcher.test.mjs
pnpm verify:clean
```

### Live macOS acceptance

1. stop all currently eligible Viewer development processes;
2. run `pnpm start:viewer`;
3. verify one exact current-repository `viewer-desktop` process;
4. run `pnpm start:viewer` again;
5. verify the first PID is gone and one new exact process remains;
6. verify the wrapper directory is absent;
7. verify no Viewer process references another worktree;
8. leave the latest single development instance running for visual review.

The live check will use process evidence only. It will not activate Viewer by
bundle identifier because doing so was the original second launch path.

## Acceptance Criteria

- every successful `pnpm start:viewer` leaves exactly one eligible Viewer
  process;
- a repeated launch replaces the previous process instead of adding another;
- the running executable belongs to the current repository;
- the legacy temporary wrapper is absent after launch;
- duplicate detection fails safely and leaves no newly spawned session running;
- launcher output identifies the verified Viewer PID, source revision, executable,
  and log;
- Git state is unchanged by launch and cleanup;
- the full repository verification suite passes.

## Out of Scope

- enforcing one instance in packaged Viewer releases;
- changing macOS window activation or application lifecycle behavior;
- implementing the future Windows single-instance policy;
- packaging, installing, signing, or registering a development `Viewer.app`;
- changing Viewer UI, project state, or file-operation behavior.
