# Viewer Single-Instance Development Launcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every successful `pnpm start:viewer` invocation replace prior Viewer development sessions and leave exactly one current-repository desktop process.

**Architecture:** Keep the existing detached `pnpm tauri dev` launcher. Add exact cleanup for the superseded runtime wrapper, classify current and eligible Viewer processes during readiness, reject any observation other than one current process and one eligible process, and expose the verified Viewer PID without reopening the application.

**Tech Stack:** Node.js 24.18.0, pnpm 10.0.0, Node built-in test runner, Tauri 2 development CLI, macOS process table.

## Global Constraints

- `pnpm start:viewer` remains the only supported development launch command.
- The launcher starts `pnpm tauri dev`; it does not package, install, sign, or register `Viewer.app`.
- Cleanup may remove only `<repo>/target/dev-launcher/current-dev-wrapper` and existing eligible Viewer processes selected by the established safety rules.
- Git state, project data, installed application copies, and unrelated processes must remain untouched.
- A successful launch requires exactly one current-repository `target/debug/viewer-desktop` process and exactly one eligible Viewer process in total.
- On duplicate detection, stop the newly spawned Tauri process group, clear `session.json`, preserve the log, and fail.
- Development automation must not open Viewer by bundle identifier or a temporary application path after the launcher succeeds.
- Follow red-green-refactor for every production behavior change.

## File Structure

- Modify `scripts/viewer-dev-launcher.mjs`: derive the legacy-wrapper path, remove it through the runtime boundary, inspect exact and eligible Viewer counts, enforce readiness, and return `viewerPid`.
- Modify `scripts/start-viewer-dev.mjs`: print the verified Viewer PID while retaining current source and log reporting.
- Modify `scripts/viewer-dev-launcher.test.mjs`: own all launcher regression tests, real temporary-directory cleanup checks, duplicate readiness cases, CLI output, and README contract checks.
- Modify `README.md`: document the single-instance development contract and prohibit secondary bundle activation.

---

### Task 1: Remove the legacy runtime wrapper before launch

**Files:**
- Modify: `scripts/viewer-dev-launcher.test.mjs:1-112,319-499`
- Modify: `scripts/viewer-dev-launcher.mjs:10-49,157-177,216-326,355-361`

**Interfaces:**
- Consumes: existing `buildLauncherPaths(moduleUrl)` and `createSystemRuntime(paths, options)`.
- Produces: `LauncherPaths.legacyWrapperPath: string` and `LauncherRuntime.removeLegacyWrapper(): Promise<void>`.

- [ ] **Step 1: Extend path and real-runtime tests before changing production code**

Update the `node:fs/promises` import in `scripts/viewer-dev-launcher.test.mjs` to include `access`. In the path test add:

```js
assert.equal(
  paths.legacyWrapperPath,
  `${repoRoot}/target/dev-launcher/current-dev-wrapper`,
)
```

In the `createSystemRuntime` integration test, add `legacyWrapperPath` to `paths`, create a sentinel file, call the desired runtime API, and assert that only the wrapper tree disappears:

```js
const legacyWrapperPath = path.join(stateDir, 'current-dev-wrapper')
const preservedPath = path.join(stateDir, 'preserved.txt')
const paths = {
  repoRoot: temporaryRoot,
  stateDir,
  statePath: path.join(stateDir, 'session.json'),
  logPath: path.join(stateDir, 'tauri-dev.log'),
  executablePath,
  legacyWrapperPath,
}

await mkdir(path.join(legacyWrapperPath, 'Viewer.app'), { recursive: true })
await writeFile(path.join(legacyWrapperPath, 'Viewer.app', 'sentinel'), 'legacy\n')
await writeFile(preservedPath, 'keep\n')

const runtime = createSystemRuntime(paths, {
  env: {
    ...process.env,
    PATH: `${fakeBin}:${process.env.PATH}`,
  },
})
await runtime.removeLegacyWrapper()
await assert.rejects(access(legacyWrapperPath), { code: 'ENOENT' })
assert.equal(await readFile(preservedPath, 'utf8'), 'keep\n')
```

Add `legacyWrapperPath` to the shared `restartDevelopmentViewer` paths object:

```js
legacyWrapperPath: `${repoRoot}/target/dev-launcher/current-dev-wrapper`,
```

Add this exact no-op to every existing `restartDevelopmentViewer` runtime double
so failures remain focused on the behavior under test:

```js
async removeLegacyWrapper() {},
```

- [ ] **Step 2: Run the launcher test and verify the new contract fails**

Run:

```bash
node --test scripts/viewer-dev-launcher.test.mjs
```

Expected: FAIL because `legacyWrapperPath` is absent and `runtime.removeLegacyWrapper` is not implemented.

- [ ] **Step 3: Implement exact-path cleanup**

Extend the launcher path type and return value:

```js
/**
 * @typedef {{
 *   repoRoot: string,
 *   stateDir: string,
 *   statePath: string,
 *   logPath: string,
 *   executablePath: string,
 *   legacyWrapperPath: string,
 * }} LauncherPaths
 */

return {
  repoRoot,
  stateDir,
  statePath: path.join(stateDir, 'session.json'),
  logPath: path.join(stateDir, 'tauri-dev.log'),
  executablePath: path.join(repoRoot, 'target', 'debug', 'viewer-desktop'),
  legacyWrapperPath: path.join(stateDir, 'current-dev-wrapper'),
}
```

Add the runtime method to the `LauncherRuntime` typedef and `createSystemRuntime`:

```js
// LauncherRuntime
removeLegacyWrapper(): Promise<void>,

// createSystemRuntime return object
async removeLegacyWrapper() {
  await rm(paths.legacyWrapperPath, { recursive: true, force: true })
},
```

Call it as the first side effect in `restartDevelopmentViewer`:

```js
await runtime.removeLegacyWrapper()
const initial = await runtime.listProcesses()
```

- [ ] **Step 4: Add orchestration coverage for cleanup ordering and failure**

In the successful restart test, record `remove-wrapper` and expect it before the first `list`:

```js
async removeLegacyWrapper() {
  events.push('remove-wrapper')
}
```

Add a focused failure test:

```js
it('does not inspect or spawn when legacy wrapper cleanup fails', async () => {
  const events = []
  const runtime = {
    async removeLegacyWrapper() {
      events.push('remove-wrapper')
      throw new Error('wrapper cleanup denied')
    },
    async listProcesses() {
      events.push('list')
      return []
    },
  }

  await assert.rejects(
    restartDevelopmentViewer({ paths, runtime, timeoutMs: 100, pollMs: 1 }),
    /wrapper cleanup denied/,
  )
  assert.deepEqual(events, ['remove-wrapper'])
})
```

- [ ] **Step 5: Run the focused tests and verify green**

Run:

```bash
node --test scripts/viewer-dev-launcher.test.mjs
```

Expected: all launcher tests PASS with no warnings or leaked child processes.

- [ ] **Step 6: Commit the cleanup boundary**

```bash
git add scripts/viewer-dev-launcher.mjs scripts/viewer-dev-launcher.test.mjs
git commit -m "fix: remove legacy Viewer launch wrapper"
```

---

### Task 2: Enforce exact single-instance readiness and expose the Viewer PID

**Files:**
- Modify: `scripts/viewer-dev-launcher.test.mjs:7-18,114-152,167-270,319-499`
- Modify: `scripts/viewer-dev-launcher.mjs:68-155,346-417`
- Modify: `scripts/start-viewer-dev.mjs:25-52`

**Interfaces:**
- Consumes: `isViewerExecutable(command, repoRoot)`, `LauncherRuntime`, and the wrapper cleanup introduced in Task 1.
- Produces: `inspectDevelopmentViewers(processes, repoRoot, executablePath): {exact: ProcessInfo[], eligible: ProcessInfo[]}` and launch result `{pid, viewerPid, executablePath, logPath}`.

- [ ] **Step 1: Write process-classification tests**

Import the wished-for helper and replace the Boolean-only readiness test with exact assertions:

```js
import {
  inspectDevelopmentViewers,
  // existing imports
} from './viewer-dev-launcher.mjs'

it('separates the exact development Viewer from every eligible Viewer', () => {
  const observation = inspectDevelopmentViewers(
    parseProcessTable(table),
    repoRoot,
    `${repoRoot}/target/debug/viewer-desktop`,
  )

  assert.deepEqual(observation.exact.map(({ pid }) => pid), [122])
  assert.deepEqual(observation.eligible.map(({ pid }) => pid), [122, 212, 220, 320])
})
```

Keep the existing `isExactDevelopmentViewerRunning` test until all call sites no longer depend on it; removing that exported compatibility helper is not required by this plan.

- [ ] **Step 2: Write restart tests for successful PID and duplicate rejection**

Update the successful result assertion:

```js
assert.deepEqual(result, {
  pid: 899,
  viewerPid: 900,
  executablePath: `${repoRoot}/target/debug/viewer-desktop`,
  logPath: `${repoRoot}/target/dev-launcher/tauri-dev.log`,
})
```

Add a duplicate test whose readiness snapshot contains two exact processes:

```js
it('stops the spawned group and clears state when two Viewers appear', async () => {
  const events = []
  const exactCommand = `${repoRoot}/target/debug/viewer-desktop`
  const snapshots = [
    [],
    [],
    [
      { pid: 910, ppid: 909, pgid: 909, command: exactCommand },
      { pid: 911, ppid: 1, pgid: 911, command: exactCommand },
    ],
  ]
  const runtime = {
    async removeLegacyWrapper() { events.push('remove-wrapper') },
    async listProcesses() { return snapshots.shift() ?? [] },
    async readSession() { return undefined },
    async writeSession(state) { events.push(`write:${state.pid}`) },
    async clearSession() { events.push('clear') },
    async stop(target) { events.push(`stop:${target.kind}:${target.id}`) },
    async spawn() { return { pid: 909, pgid: 909 } },
    isAlive() { return true },
    async sleep() {},
    async tailLog() { return '' },
  }

  await assert.rejects(
    restartDevelopmentViewer({ paths, runtime, timeoutMs: 100, pollMs: 1 }),
    /exact=2, eligible=2/,
  )
  assert.deepEqual(events, [
    'remove-wrapper',
    'write:909',
    'stop:group:909',
    'clear',
  ])
})
```

Add the parallel case with one exact current process and one packaged Viewer:

```js
it('stops the spawned group when another eligible Viewer appears', async () => {
  const events = []
  const snapshots = [
    [],
    [],
    [
      {
        pid: 920,
        ppid: 919,
        pgid: 919,
        command: `${repoRoot}/target/debug/viewer-desktop`,
      },
      {
        pid: 921,
        ppid: 1,
        pgid: 921,
        command: '/Applications/Viewer.app/Contents/MacOS/viewer-desktop',
      },
    ],
  ]
  const runtime = {
    async removeLegacyWrapper() { events.push('remove-wrapper') },
    async listProcesses() { return snapshots.shift() ?? [] },
    async readSession() { return undefined },
    async writeSession(state) { events.push(`write:${state.pid}`) },
    async clearSession() { events.push('clear') },
    async stop(target) { events.push(`stop:${target.kind}:${target.id}`) },
    async spawn() { return { pid: 919, pgid: 919 } },
    isAlive() { return true },
    async sleep() {},
    async tailLog() { return '' },
  }

  await assert.rejects(
    restartDevelopmentViewer({ paths, runtime, timeoutMs: 100, pollMs: 1 }),
    /exact=1, eligible=2/,
  )
  assert.deepEqual(events, [
    'remove-wrapper',
    'write:919',
    'stop:group:919',
    'clear',
  ])
})
```

- [ ] **Step 3: Write the failing CLI PID-output assertion**

Make the fake restart result include `viewerPid: 901` and require this output:

```js
assert.deepEqual(output, [
  `Viewer development version is running: ${repoRoot}/target/debug/viewer-desktop`,
  'Viewer process: 901',
  'Source: main @ abc1234 (dirty)',
  `Log: ${repoRoot}/target/dev-launcher/tauri-dev.log`,
])
```

- [ ] **Step 4: Run tests and verify red for missing classification and PID behavior**

Run:

```bash
node --test scripts/viewer-dev-launcher.test.mjs
```

Expected: FAIL because `inspectDevelopmentViewers` is not exported, duplicate observations do not abort, and CLI output omits the Viewer PID.

- [ ] **Step 5: Implement process inspection**

Add the helper beside the existing readiness helper:

```js
export function inspectDevelopmentViewers(processes, repoRoot, executablePath) {
  return {
    exact: processes.filter(
      (item) => commandExecutable(item.command) === executablePath,
    ),
    eligible: processes.filter((item) =>
      isViewerExecutable(item.command, repoRoot),
    ),
  }
}
```

- [ ] **Step 6: Implement strict readiness and duplicate rollback**

Replace the readiness Boolean branch with:

```js
const observation = inspectDevelopmentViewers(
  processes,
  paths.repoRoot,
  paths.executablePath,
)

if (observation.exact.length === 1 && observation.eligible.length === 1) {
  return {
    pid: child.pid,
    viewerPid: observation.exact[0].pid,
    executablePath: paths.executablePath,
    logPath: paths.logPath,
  }
}

const hasDuplicate =
  observation.exact.length > 1 ||
  observation.eligible.length > 1 ||
  (observation.exact.length === 0 && observation.eligible.length > 0)

if (hasDuplicate) {
  await runtime.stop({ kind: 'group', id: child.pgid, viewerPid: child.pid })
  await runtime.clearSession()
  throw new Error(
    `Viewer development startup violated the single-instance invariant: ` +
      `exact=${observation.exact.length}, eligible=${observation.eligible.length}`,
  )
}
```

Update the return JSDoc to:

```js
@returns {Promise<{
  pid: number,
  viewerPid: number,
  executablePath: string,
  logPath: string,
}>}
```

- [ ] **Step 7: Report the verified Viewer PID**

In `runCli`, add the line immediately after the executable line:

```js
log(`Viewer process: ${result.viewerPid}`)
```

- [ ] **Step 8: Run focused and complete Node tests**

Run:

```bash
node --test scripts/viewer-dev-launcher.test.mjs
node --test scripts/*.test.mjs
```

Expected: both commands PASS with zero failures.

- [ ] **Step 9: Commit single-instance readiness**

```bash
git add scripts/viewer-dev-launcher.mjs scripts/start-viewer-dev.mjs scripts/viewer-dev-launcher.test.mjs
git commit -m "fix: enforce one Viewer development instance"
```

---

### Task 3: Document and verify the canonical single-instance workflow

**Files:**
- Modify: `scripts/viewer-dev-launcher.test.mjs:154-165`
- Modify: `README.md:63-73`

**Interfaces:**
- Consumes: canonical `package.json` command and launch result from Tasks 1–2.
- Produces: executable README contract and live macOS acceptance evidence.

- [ ] **Step 1: Write the failing README contract test**

Extend the `package command` describe block:

```js
it('documents the single-instance launcher without a secondary app open', async () => {
  const readme = await readFile(new URL('../README.md', import.meta.url), 'utf8')

  assert.match(readme, /只保留一个当前仓库的 Viewer 开发实例/)
  assert.match(readme, /不得再通过 bundle identifier 或临时 Viewer\.app 二次打开/)
})
```

- [ ] **Step 2: Run the focused test and verify red**

Run:

```bash
node --test scripts/viewer-dev-launcher.test.mjs
```

Expected: FAIL because README does not yet contain the single-instance contract.

- [ ] **Step 3: Update README with exact operator guidance**

Replace the paragraph following `pnpm start:viewer` with:

```markdown
启动器会关闭已有 Viewer 开发进程，清理废弃的运行时包装器，再从当前仓库
执行 Tauri 开发启动；成功后只保留一个当前仓库的 Viewer 开发实例。日志保存
在 `target/dev-launcher/tauri-dev.log`。它不会拉取远端代码、切换分支、安装或
启动已打包的 `Viewer.app`。

开发自动化在命令成功后不得再通过 bundle identifier 或临时 `Viewer.app`
二次打开 Viewer；验证应使用启动器输出的 Viewer PID 和进程表。
```

- [ ] **Step 4: Run documentation and launcher tests**

Run:

```bash
node --test scripts/viewer-dev-launcher.test.mjs
```

Expected: all launcher and README contract tests PASS.

- [ ] **Step 5: Run the full repository verification**

Run:

```bash
pnpm verify:clean
```

Expected: exit code 0; UI tests, Rust formatting, Clippy, Rust tests, security checks, Cargo policy, and npm license policy all pass. Existing informational dependency-duplication and Biome deprecation notices do not constitute failures.

- [ ] **Step 6: Perform two real macOS launches and prove replacement**

Run:

```bash
pnpm start:viewer
viewer_exact_path="$PWD/target/debug/viewer-desktop"
first_viewer_pid=$(ps -axo pid=,command= | awk -v exact="$viewer_exact_path" '$2 == exact && NF == 2 {print $1}')
test -n "$first_viewer_pid"
test "$(printf '%s\n' "$first_viewer_pid" | awk 'NF {count += 1} END {print count + 0}')" -eq 1
test ! -e "$PWD/target/dev-launcher/current-dev-wrapper"
pnpm start:viewer
second_viewer_pid=$(ps -axo pid=,command= | awk -v exact="$viewer_exact_path" '$2 == exact && NF == 2 {print $1}')
test -n "$second_viewer_pid"
test "$(printf '%s\n' "$second_viewer_pid" | awk 'NF {count += 1} END {print count + 0}')" -eq 1
test "$first_viewer_pid" != "$second_viewer_pid"
if kill -0 "$first_viewer_pid" 2>/dev/null; then exit 1; fi
kill -0 "$second_viewer_pid"
test -z "$(ps -axo command= | awk '/\/\.worktrees\/.*\/viewer-desktop$/ {print}')"
git diff --check
git status --short --branch
```

Expected: the first PID is gone, the second PID is alive, exactly one current-repository Viewer remains, no worktree Viewer exists, and Git contains only the intended implementation changes.

- [ ] **Step 7: Commit documentation after live acceptance**

```bash
git add README.md scripts/viewer-dev-launcher.test.mjs
git commit -m "docs: define single-instance Viewer development launch"
```

- [ ] **Step 8: Record final evidence**

Run:

```bash
git status --short --branch
git log -3 --oneline
viewer_exact_path="$PWD/target/debug/viewer-desktop"
ps -axo pid=,ppid=,pgid=,command= | awk -v exact="$viewer_exact_path" '$4 == exact {print}'
```

Expected: clean working tree, the three task commits at HEAD, and one Viewer desktop process from the current repository left running for review.
