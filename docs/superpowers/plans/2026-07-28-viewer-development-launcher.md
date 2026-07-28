# Viewer Development Launcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add one tested `pnpm start:viewer` command that replaces any existing Viewer session and launches the current local working tree's exact Tauri development executable.

**Architecture:** Keep process classification and orchestration in an importable Node module, with a thin CLI entry point and Node built-in tests. The launcher derives paths from its own repository, safely terminates only matching Viewer processes, starts `pnpm tauri dev` detached with state and logs under `target/dev-launcher`, and confirms the exact repository executable before returning.

**Tech Stack:** Node.js 24 ESM, `node:test`, pnpm 10, Tauri 2, macOS `ps` and POSIX process signals.

## Global Constraints

- Use the current local working tree, including uncommitted changes.
- Never run Git fetch, pull, checkout, clean, stash, commit, reset, or any source-changing command from the launcher.
- Resolve the repository root from the launcher files, not the caller's current directory.
- Start `pnpm tauri dev` from that repository root.
- Stop an existing Viewer session before starting its replacement.
- Only terminate Viewer processes belonging to this repository, its `.worktrees`, or a macOS `Viewer.app` bundle.
- Never terminate an arbitrary process merely because it owns the Vite port.
- Store runtime state only below `target/dev-launcher/`.
- Wait at most 120 seconds for the exact `<repository>/target/debug/viewer-desktop` process.
- Leave packaged `Viewer.app` copies outside the development launch path.
- Preserve the user's existing uncommitted UI and fixture changes.

## File Structure

- Create `scripts/viewer-dev-launcher.mjs`: path derivation, process-table parsing and selection, state safety, process termination, detached launch, readiness polling, and launch result formatting.
- Create `scripts/viewer-dev-launcher.test.mjs`: Node tests for classification, selection, stale-session protection, orchestration ordering, readiness, and failure behavior.
- Create `scripts/start-viewer-dev.mjs`: thin executable entry point that wires production dependencies to the launcher and prints diagnostics.
- Modify `package.json`: expose the canonical `start:viewer` command.
- Modify `README.md`: replace the raw Tauri command with the canonical launcher and explain local dirty-tree behavior.

---

### Task 1: Safe process model and target selection

**Files:**
- Create: `scripts/viewer-dev-launcher.mjs`
- Create: `scripts/viewer-dev-launcher.test.mjs`

**Interfaces:**
- Produces: `buildLauncherPaths(moduleUrl: string): LauncherPaths`
- Produces: `parseProcessTable(output: string): ProcessInfo[]`
- Produces: `isViewerExecutable(command: string, repoRoot: string): boolean`
- Produces: `isTauriDevProcess(command: string, repoRoot: string): boolean`
- Produces: `selectStopTargets(processes: ProcessInfo[], repoRoot: string, currentPid?: number): StopTarget[]`
- Produces: `isExactDevelopmentViewerRunning(processes: ProcessInfo[], executablePath: string): boolean`

```js
/**
 * @typedef {{
 *   repoRoot: string,
 *   stateDir: string,
 *   statePath: string,
 *   logPath: string,
 *   executablePath: string
 * }} LauncherPaths
 *
 * @typedef {{pid: number, ppid: number, pgid: number, command: string}} ProcessInfo
 * @typedef {{kind: "group" | "process", id: number, viewerPid: number}} StopTarget
 */
```

- [ ] **Step 1: Write failing path, parsing, selection, and readiness tests**

Create `scripts/viewer-dev-launcher.test.mjs`:

```js
import assert from "node:assert/strict"
import { describe, it } from "node:test"
import {
  buildLauncherPaths,
  isExactDevelopmentViewerRunning,
  isTauriDevProcess,
  isViewerExecutable,
  parseProcessTable,
  selectStopTargets,
} from "./viewer-dev-launcher.mjs"

const repoRoot = "/Users/example/Project/Viewer"

describe("buildLauncherPaths", () => {
  it("derives repository and runtime paths from the module URL", () => {
    const paths = buildLauncherPaths(
      "file:///Users/example/Project/Viewer/scripts/start-viewer-dev.mjs",
    )

    assert.equal(paths.repoRoot, repoRoot)
    assert.equal(paths.stateDir, `${repoRoot}/target/dev-launcher`)
    assert.equal(paths.statePath, `${repoRoot}/target/dev-launcher/session.json`)
    assert.equal(paths.logPath, `${repoRoot}/target/dev-launcher/tauri-dev.log`)
    assert.equal(paths.executablePath, `${repoRoot}/target/debug/viewer-desktop`)
  })
})

describe("process selection", () => {
  const table = `
  120 1 120 node /Users/example/.local/bin/pnpm tauri dev
  121 120 120 node /Users/example/Project/Viewer/node_modules/@tauri-apps/cli/tauri.js dev
  122 121 120 /Users/example/Project/Viewer/target/debug/viewer-desktop
  220 1 220 /Users/example/Project/Viewer/.worktrees/theme/target/debug/viewer-desktop
  320 1 320 /Applications/Viewer.app/Contents/MacOS/viewer-desktop
  420 1 420 /tmp/another-product/viewer-desktop
  520 1 520 vite --port 5173
  `

  it("parses pid, parent, group, and complete command", () => {
    assert.deepEqual(parseProcessTable(table)[0], {
      pid: 120,
      ppid: 1,
      pgid: 120,
      command: "node /Users/example/.local/bin/pnpm tauri dev",
    })
  })

  it("recognizes only eligible Viewer executables", () => {
    assert.equal(
      isViewerExecutable(`${repoRoot}/target/debug/viewer-desktop`, repoRoot),
      true,
    )
    assert.equal(
      isViewerExecutable(
        `${repoRoot}/.worktrees/theme/target/debug/viewer-desktop`,
        repoRoot,
      ),
      true,
    )
    assert.equal(
      isViewerExecutable(
        "/Applications/Viewer.app/Contents/MacOS/viewer-desktop",
        repoRoot,
      ),
      true,
    )
    assert.equal(isViewerExecutable("/tmp/another-product/viewer-desktop", repoRoot), false)
  })

  it("recognizes repository Tauri development commands", () => {
    assert.equal(
      isTauriDevProcess(
        `node ${repoRoot}/node_modules/@tauri-apps/cli/tauri.js dev`,
        repoRoot,
      ),
      true,
    )
    assert.equal(isTauriDevProcess("vite --port 5173", repoRoot), false)
  })

  it("uses the development process group and exact packaged process", () => {
    assert.deepEqual(selectStopTargets(parseProcessTable(table), repoRoot, 999), [
      { kind: "group", id: 120, viewerPid: 122 },
      { kind: "process", id: 220, viewerPid: 220 },
      { kind: "process", id: 320, viewerPid: 320 },
    ])
  })

  it("never selects the current launcher's process group", () => {
    const processes = parseProcessTable(table)
    processes.push({
      pid: 999,
      ppid: 1,
      pgid: 120,
      command: `node ${repoRoot}/scripts/start-viewer-dev.mjs`,
    })

    assert.deepEqual(selectStopTargets(processes, repoRoot, 999), [
      { kind: "process", id: 122, viewerPid: 122 },
      { kind: "process", id: 220, viewerPid: 220 },
      { kind: "process", id: 320, viewerPid: 320 },
    ])
  })

  it("requires the exact current repository executable for readiness", () => {
    const processes = parseProcessTable(table)
    assert.equal(
      isExactDevelopmentViewerRunning(
        processes,
        `${repoRoot}/target/debug/viewer-desktop`,
      ),
      true,
    )
    assert.equal(
      isExactDevelopmentViewerRunning(
        processes,
        `${repoRoot}/other/target/debug/viewer-desktop`,
      ),
      false,
    )
  })
})
```

- [ ] **Step 2: Run the tests and verify RED**

Run:

```bash
node --test scripts/viewer-dev-launcher.test.mjs
```

Expected: FAIL because `scripts/viewer-dev-launcher.mjs` does not exist.

- [ ] **Step 3: Implement the minimal process model**

Create `scripts/viewer-dev-launcher.mjs` with:

```js
import path from "node:path"
import { fileURLToPath } from "node:url"

export function buildLauncherPaths(moduleUrl) {
  const scriptPath = fileURLToPath(moduleUrl)
  const repoRoot = path.dirname(path.dirname(scriptPath))
  const stateDir = path.join(repoRoot, "target", "dev-launcher")

  return {
    repoRoot,
    stateDir,
    statePath: path.join(stateDir, "session.json"),
    logPath: path.join(stateDir, "tauri-dev.log"),
    executablePath: path.join(repoRoot, "target", "debug", "viewer-desktop"),
  }
}

export function parseProcessTable(output) {
  return output
    .split("\n")
    .map((line) => line.match(/^\s*(\d+)\s+(\d+)\s+(\d+)\s+(.+?)\s*$/))
    .filter(Boolean)
    .map((match) => ({
      pid: Number(match[1]),
      ppid: Number(match[2]),
      pgid: Number(match[3]),
      command: match[4],
    }))
}

function commandExecutable(command) {
  return command.trim().split(/\s+/, 1)[0]
}

export function isViewerExecutable(command, repoRoot) {
  const executable = commandExecutable(command)
  const repositoryViewer =
    executable.startsWith(`${repoRoot}${path.sep}`) &&
    executable.endsWith(`${path.sep}viewer-desktop`)
  const packagedViewer =
    executable.endsWith(`${path.sep}Viewer.app${path.sep}Contents${path.sep}MacOS${path.sep}viewer-desktop`)

  return repositoryViewer || packagedViewer
}

export function isTauriDevProcess(command, repoRoot) {
  return (
    command.includes(`${repoRoot}${path.sep}node_modules${path.sep}`) &&
    /(?:tauri\.js|tauri)\s+["']?dev["']?(?:\s|$)/.test(command)
  )
}

export function selectStopTargets(processes, repoRoot, currentPid = process.pid) {
  const byPid = new Map(processes.map((item) => [item.pid, item]))
  const current = byPid.get(currentPid)
  const currentGroup = current?.pgid
  const targets = []
  const seen = new Set()

  for (const viewer of processes.filter((item) => isViewerExecutable(item.command, repoRoot))) {
    let ancestor = viewer
    let hasTauriAncestor = false

    while (ancestor) {
      if (isTauriDevProcess(ancestor.command, repoRoot)) {
        hasTauriAncestor = true
        break
      }
      ancestor = byPid.get(ancestor.ppid)
    }

    const useGroup =
      hasTauriAncestor && viewer.pgid > 1 && viewer.pgid !== currentGroup
    const target = useGroup
      ? { kind: "group", id: viewer.pgid, viewerPid: viewer.pid }
      : { kind: "process", id: viewer.pid, viewerPid: viewer.pid }
    const key = `${target.kind}:${target.id}`

    if (!seen.has(key)) {
      seen.add(key)
      targets.push(target)
    }
  }

  return targets
}

export function isExactDevelopmentViewerRunning(processes, executablePath) {
  return processes.some(
    (item) => commandExecutable(item.command) === executablePath,
  )
}
```

- [ ] **Step 4: Run the tests and verify GREEN**

Run:

```bash
node --test scripts/viewer-dev-launcher.test.mjs
```

Expected: all process-model tests PASS.

- [ ] **Step 5: Commit the process model**

```bash
git add scripts/viewer-dev-launcher.mjs scripts/viewer-dev-launcher.test.mjs
git commit -m "test: define safe Viewer process selection"
```

---

### Task 2: Restart orchestration, state safety, and diagnostics

**Files:**
- Modify: `scripts/viewer-dev-launcher.mjs`
- Modify: `scripts/viewer-dev-launcher.test.mjs`
- Create: `scripts/start-viewer-dev.mjs`

**Interfaces:**
- Consumes: process helpers from Task 1.
- Produces: `createSystemRuntime(paths: LauncherPaths): LauncherRuntime`
- Produces: `restartDevelopmentViewer(options: RestartOptions): Promise<LaunchResult>`
- Produces: `runCli(): Promise<void>` in the CLI entry point.

```js
/**
 * @typedef {{
 *   listProcesses(): Promise<ProcessInfo[]>,
 *   readSession(): Promise<SessionState | undefined>,
 *   writeSession(state: SessionState): Promise<void>,
 *   clearSession(): Promise<void>,
 *   stop(target: StopTarget): Promise<void>,
 *   spawn(): Promise<{pid: number, pgid: number}>,
 *   isAlive(pid: number): boolean,
 *   sleep(ms: number): Promise<void>,
 *   tailLog(lines: number): Promise<string>
 * }} LauncherRuntime
 *
 * @typedef {{version: 1, pid: number, pgid: number, repoRoot: string, startedAt: string}} SessionState
 * @typedef {{paths: LauncherPaths, runtime: LauncherRuntime, timeoutMs?: number, pollMs?: number}} RestartOptions
 * @typedef {{pid: number, executablePath: string, logPath: string}} LaunchResult
 */
```

- [ ] **Step 1: Write failing orchestration tests**

Append to `scripts/viewer-dev-launcher.test.mjs`:

```js
import { restartDevelopmentViewer, sessionMatchesProcess } from "./viewer-dev-launcher.mjs"

describe("session safety", () => {
  it("rejects a stale or reused session pid", () => {
    const session = {
      version: 1,
      pid: 700,
      pgid: 700,
      repoRoot,
      startedAt: "2026-07-28T00:00:00.000Z",
    }
    assert.equal(
      sessionMatchesProcess(
        session,
        { pid: 700, ppid: 1, pgid: 700, command: "sleep 120" },
        repoRoot,
      ),
      false,
    )
  })
})

describe("restartDevelopmentViewer", () => {
  it("stops an existing Viewer before spawning and waits for the exact executable", async () => {
    const events = []
    let snapshots = [
      [
        {
          pid: 122,
          ppid: 121,
          pgid: 120,
          command: `${repoRoot}/target/debug/viewer-desktop`,
        },
        {
          pid: 121,
          ppid: 120,
          pgid: 120,
          command: `node ${repoRoot}/node_modules/@tauri-apps/cli/tauri.js dev`,
        },
      ],
      [],
      [
        {
          pid: 900,
          ppid: 899,
          pgid: 899,
          command: `${repoRoot}/target/debug/viewer-desktop`,
        },
      ],
    ]
    const runtime = {
      async listProcesses() {
        events.push("list")
        return snapshots.shift() ?? []
      },
      async readSession() {
        return undefined
      },
      async writeSession(state) {
        events.push(`write:${state.pid}`)
      },
      async clearSession() {
        events.push("clear")
      },
      async stop(target) {
        events.push(`stop:${target.kind}:${target.id}`)
      },
      async spawn() {
        events.push("spawn")
        return { pid: 899, pgid: 899 }
      },
      isAlive() {
        return true
      },
      async sleep() {},
      async tailLog() {
        return ""
      },
    }

    const result = await restartDevelopmentViewer({
      paths: {
        repoRoot,
        stateDir: `${repoRoot}/target/dev-launcher`,
        statePath: `${repoRoot}/target/dev-launcher/session.json`,
        logPath: `${repoRoot}/target/dev-launcher/tauri-dev.log`,
        executablePath: `${repoRoot}/target/debug/viewer-desktop`,
      },
      runtime,
      timeoutMs: 100,
      pollMs: 1,
    })

    assert.deepEqual(events, [
      "list",
      "stop:group:120",
      "list",
      "spawn",
      "write:899",
      "list",
    ])
    assert.equal(result.pid, 899)
    assert.equal(result.executablePath, `${repoRoot}/target/debug/viewer-desktop`)
  })

  it("cleans up and includes the log tail when startup exits early", async () => {
    let spawned = false
    const runtime = {
      async listProcesses() {
        return []
      },
      async readSession() {
        return undefined
      },
      async writeSession() {},
      async clearSession() {},
      async stop() {},
      async spawn() {
        spawned = true
        return { pid: 901, pgid: 901 }
      },
      isAlive() {
        return !spawned
      },
      async sleep() {},
      async tailLog() {
        return "compile failed"
      },
    }

    await assert.rejects(
      restartDevelopmentViewer({
        paths: {
          repoRoot,
          stateDir: `${repoRoot}/target/dev-launcher`,
          statePath: `${repoRoot}/target/dev-launcher/session.json`,
          logPath: `${repoRoot}/target/dev-launcher/tauri-dev.log`,
          executablePath: `${repoRoot}/target/debug/viewer-desktop`,
        },
        runtime,
        timeoutMs: 100,
        pollMs: 1,
      }),
      /compile failed/,
    )
  })
})
```

- [ ] **Step 2: Run the tests and verify RED**

Run:

```bash
node --test scripts/viewer-dev-launcher.test.mjs
```

Expected: FAIL because `sessionMatchesProcess` and
`restartDevelopmentViewer` are not exported.

- [ ] **Step 3: Implement minimal restart orchestration**

Extend `scripts/viewer-dev-launcher.mjs` with:

```js
export function sessionMatchesProcess(session, liveProcess, repoRoot) {
  return (
    session?.version === 1 &&
    session.repoRoot === repoRoot &&
    liveProcess?.pid === session.pid &&
    liveProcess.pgid === session.pgid &&
    /(?:pnpm\s+tauri\s+["']?dev["']?|tauri\.js\s+["']?dev["']?)(?:\s|$)/.test(
      liveProcess.command,
    )
  )
}

export async function restartDevelopmentViewer({
  paths,
  runtime,
  timeoutMs = 120_000,
  pollMs = 250,
}) {
  const initial = await runtime.listProcesses()
  const stored = await runtime.readSession()
  const storedProcess = stored
    ? initial.find((item) => item.pid === stored.pid)
    : undefined

  if (stored && sessionMatchesProcess(stored, storedProcess, paths.repoRoot)) {
    await runtime.stop({ kind: "group", id: stored.pgid, viewerPid: stored.pid })
  } else if (stored) {
    await runtime.clearSession()
  }

  for (const target of selectStopTargets(initial, paths.repoRoot)) {
    if (stored?.pgid !== target.id || target.kind !== "group") {
      await runtime.stop(target)
    }
  }

  const remaining = await runtime.listProcesses()
  if (remaining.some((item) => isViewerExecutable(item.command, paths.repoRoot))) {
    throw new Error("Existing Viewer process did not stop")
  }

  const child = await runtime.spawn()
  await runtime.writeSession({
    version: 1,
    pid: child.pid,
    pgid: child.pgid,
    repoRoot: paths.repoRoot,
    startedAt: new Date().toISOString(),
  })

  const deadline = Date.now() + timeoutMs
  while (Date.now() <= deadline) {
    const processes = await runtime.listProcesses()
    if (isExactDevelopmentViewerRunning(processes, paths.executablePath)) {
      return {
        pid: child.pid,
        executablePath: paths.executablePath,
        logPath: paths.logPath,
      }
    }
    if (!runtime.isAlive(child.pid)) {
      const tail = await runtime.tailLog(40)
      await runtime.clearSession()
      throw new Error(`Viewer development launcher exited early.\n${tail}`)
    }
    await runtime.sleep(pollMs)
  }

  await runtime.stop({ kind: "group", id: child.pgid, viewerPid: child.pid })
  await runtime.clearSession()
  const tail = await runtime.tailLog(40)
  throw new Error(`Viewer development startup timed out.\n${tail}`)
}
```

Implement `createSystemRuntime(paths)` using only Node standard-library APIs:

- `execFile("ps", ["-axo", "pid=,ppid=,pgid=,command="])` for process
  snapshots;
- `fs.mkdir(paths.stateDir, { recursive: true })`;
- JSON read/write for `session.json`, treating missing or invalid JSON as stale;
- `process.kill(-pgid, "SIGTERM")` for group targets and
  `process.kill(pid, "SIGTERM")` for exact-process targets;
- poll for up to 5 seconds after each signal and throw if the target remains;
- `spawn("pnpm", ["tauri", "dev"], { cwd: paths.repoRoot, detached: true,
  stdio: ["ignore", logFd, logFd] })`, then `unref()`;
- `process.kill(pid, 0)` for liveness;
- the final 40 UTF-8 log lines for diagnostics.

Before spawning, validate `package.json` and `src-tauri` below
`paths.repoRoot`, and surface an explicit error if `pnpm` cannot be executed.

- [ ] **Step 4: Add the thin CLI**

Create `scripts/start-viewer-dev.mjs`:

```js
#!/usr/bin/env node

import { execFile } from "node:child_process"
import { promisify } from "node:util"
import {
  buildLauncherPaths,
  createSystemRuntime,
  restartDevelopmentViewer,
} from "./viewer-dev-launcher.mjs"

const execFileAsync = promisify(execFile)

export async function runCli() {
  const paths = buildLauncherPaths(import.meta.url)
  const runtime = createSystemRuntime(paths)
  const result = await restartDevelopmentViewer({ paths, runtime })
  const { stdout: branch } = await execFileAsync(
    "git",
    ["rev-parse", "--abbrev-ref", "HEAD"],
    { cwd: paths.repoRoot },
  )
  const { stdout: commit } = await execFileAsync(
    "git",
    ["rev-parse", "--short", "HEAD"],
    { cwd: paths.repoRoot },
  )
  const { stdout: dirty } = await execFileAsync(
    "git",
    ["status", "--porcelain"],
    { cwd: paths.repoRoot },
  )

  console.log(`Viewer development version is running: ${result.executablePath}`)
  console.log(`Source: ${branch.trim()} @ ${commit.trim()}${dirty ? " (dirty)" : ""}`)
  console.log(`Log: ${result.logPath}`)
}

runCli().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exitCode = 1
})
```

Mark it executable:

```bash
chmod +x scripts/start-viewer-dev.mjs
```

- [ ] **Step 5: Run focused tests and verify GREEN**

Run:

```bash
node --test scripts/viewer-dev-launcher.test.mjs
```

Expected: all tests PASS with no warnings.

- [ ] **Step 6: Run repository policy tests**

Run:

```bash
pnpm test:policy
```

Expected: policy tests PASS. The current repository policy has no script-file
allowlist that needs updating.

- [ ] **Step 7: Commit orchestration**

```bash
git add scripts/viewer-dev-launcher.mjs scripts/viewer-dev-launcher.test.mjs scripts/start-viewer-dev.mjs
git commit -m "feat: add safe development launcher"
```

---

### Task 3: Canonical command, documentation, and live restart verification

**Files:**
- Modify: `package.json`
- Modify: `README.md`
- Test: `scripts/viewer-dev-launcher.test.mjs`

**Interfaces:**
- Consumes: `scripts/start-viewer-dev.mjs` from Task 2.
- Produces: root command `pnpm start:viewer`.

- [ ] **Step 1: Write a failing package-command contract test**

Append to `scripts/viewer-dev-launcher.test.mjs`:

```js
import fs from "node:fs/promises"

describe("package command", () => {
  it("exposes the canonical Viewer development launcher", async () => {
    const packageJson = JSON.parse(
      await fs.readFile(new URL("../package.json", import.meta.url), "utf8"),
    )
    assert.equal(
      packageJson.scripts["start:viewer"],
      "node scripts/start-viewer-dev.mjs",
    )
  })
})
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
node --test scripts/viewer-dev-launcher.test.mjs
```

Expected: FAIL because `package.json` has no `start:viewer` script.

- [ ] **Step 3: Add the package command**

Add this exact root script in `package.json`:

```json
"start:viewer": "node scripts/start-viewer-dev.mjs"
```

Keep the existing `dev` and `tauri` commands unchanged.

- [ ] **Step 4: Update the README launch instructions**

Replace the “启动开发版” command with:

````markdown
### 启动开发版

使用项目启动器打开当前本地源码，包括尚未提交的修改：

```bash
pnpm start:viewer
```

启动器会关闭已有 Viewer 开发进程，再从当前仓库执行 Tauri 开发启动。
日志保存在 `target/dev-launcher/tauri-dev.log`。它不会拉取远端代码、
切换分支或启动已打包的 `Viewer.app`。
````

- [ ] **Step 5: Run focused and repository checks**

Run:

```bash
node --test scripts/viewer-dev-launcher.test.mjs
pnpm test:policy
pnpm --dir ui check
```

Expected: all commands PASS. The existing Biome deprecation information may be
printed, but there must be no check failure.

- [ ] **Step 6: Capture pre-launch source status**

Run:

```bash
git status --porcelain=v1 --untracked-files=all > /tmp/viewer-launch-status-before
pgrep -x viewer-desktop || true
```

Expected: the existing user-owned changes are recorded without modification.

- [ ] **Step 7: Launch once and verify the exact executable**

Run:

```bash
pnpm start:viewer
pgrep -x viewer-desktop
ps -p "$(pgrep -x viewer-desktop | head -n 1)" -o command=
```

Expected:

- `pnpm start:viewer` exits zero after readiness;
- the command is exactly
  `/Users/abc/Project/Viewer/target/debug/viewer-desktop`;
- no `/Applications/Viewer.app` executable is running.

Save the first Viewer PID:

```bash
first_viewer_pid="$(pgrep -x viewer-desktop | head -n 1)"
```

- [ ] **Step 8: Launch again and verify replacement**

Run:

```bash
pnpm start:viewer
second_viewer_pid="$(pgrep -x viewer-desktop | head -n 1)"
test "$first_viewer_pid" != "$second_viewer_pid"
ps -p "$second_viewer_pid" -o command=
```

Expected: the PID changes and the exact repository development executable is
running after the replacement.

- [ ] **Step 9: Verify source state and logs**

Run:

```bash
git status --porcelain=v1 --untracked-files=all > /tmp/viewer-launch-status-after
diff -u /tmp/viewer-launch-status-before /tmp/viewer-launch-status-after
test -s target/dev-launcher/tauri-dev.log
```

Expected: no source-state difference and a non-empty launcher log. If the
running application itself updates fixture `.viewer` data, compare tracked
source paths with `git diff --name-only` and confirm no source file changed
because of the launcher.

- [ ] **Step 10: Run final verification**

Run:

```bash
node --test scripts/viewer-dev-launcher.test.mjs
pnpm test:policy
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
git diff --check
```

Expected:

- launcher tests PASS;
- repository policy tests PASS;
- UI checks PASS;
- all UI tests PASS;
- UI production build PASS;
- no whitespace errors.

- [ ] **Step 11: Commit command and documentation**

```bash
git add package.json README.md scripts/viewer-dev-launcher.test.mjs
git commit -m "docs: standardize Viewer development startup"
```

Do not stage the user's pre-existing UI or fixture changes.
