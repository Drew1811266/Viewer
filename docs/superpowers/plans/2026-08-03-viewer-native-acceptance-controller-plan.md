# Viewer PID-Bound Native Acceptance Controller Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a non-shipping, PID-bound macOS acceptance controller that safely drives the unique current-worktree Viewer development process through all 89 atlas migration states and records exact dual-viewport native evidence.

**Architecture:** A Node.js orchestrator owns repository/process/window/path guards, disposable fixtures, recipes, evidence manifests, and a strict JSON-lines client. A small Swift helper owns PID-scoped Accessibility queries/actions, CoreGraphics input, and verified window capture; it accepts only a fixed protocol and never decides filesystem scope. The controller drives real product interactions and remains outside the React/Tauri product and application bundle.

**Tech Stack:** Node.js ESM, `node:test`, Swift 6 system compiler, macOS ApplicationServices/CoreGraphics/ImageIO, existing `pnpm start:viewer` launcher, existing atlas migration ledger and ignored `target/` evidence directories.

## Global Constraints

- Work only in `/Users/abc/Project/Viewer/.worktrees/viewer-atlas-product-migration` on `codex/viewer-atlas-product-migration`; never implement on `main`.
- Do not modify `ui/`, `src-tauri/`, React state, Tauri commands, localStorage, or release resources to make acceptance states easier to enter.
- Accept exactly one running Viewer whose real executable is this worktree's `target/debug/viewer-desktop`; reject packaged apps and every other checkout/worktree.
- Only write or perform destructive file operations below `$HOME/ViewerAcceptanceRuns/<run-id>/`; writable projects use `<variant>/测试图/` so the approved project display name is deterministic, the worktree `ViewerAcceptance` baseline is read-only, and each exact run is removed after acceptance.
- Every UI state must be reached through real keyboard, pointer, context-menu, drag, file dialog, or system actions and must have an observable state assertion.
- The only accepted viewports are exact `1024 × 720` and `1440 × 900` Viewer windows.
- Generated evidence stays ignored under `target/atlas-product-migration-acceptance/<commit>/`; only the ledger and verification index are committed.
- Exported atlas references stay ignored under `target/atlas-product-migration-reference/<atlas-sha256>/<viewport>/<ID>/reference.png`; the controller rejects a missing hash-bound image or dimensions that differ from the requested viewport.
- A passing automated test never substitutes for the atlas/reference plus native-product joint visual comparison.
- Use test-driven development: write one failing test, observe the expected failure, implement the minimum behavior, and rerun the focused test before broad gates.
- Do not add third-party runtime dependencies or include the controller/helper in Tauri builds or `Viewer.app`.

## File Structure

```text
scripts/viewer-native-acceptance.mjs
  Pure guard functions, fixture/evidence path policy, protocol client, helper build,
  recipe registry, state runner and CLI.

scripts/viewer-native-acceptance.test.mjs
  Unit and integration tests for guards, protocol, recipes, helper test mode and CLI.

scripts/viewer-native-acceptance.swift
  JSON-lines protocol parser, PID/window validation, AX tree operations, CGEvent input,
  verified window capture and protocol-test mode.

package.json
  Explicit non-shipping test and acceptance commands only.

docs/superpowers/plans/2026-08-02-viewer-atlas-to-product-complete-migration.md
  Task 15 prerequisite and canonical controller commands.

docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md
  Current run metadata and per-ID evidence paths/results.

docs/reviews/2026-07-30-viewer-ui-visual-upgrade-verification.md
  Fresh complete gate output and final current-commit evidence statement.
```

---

### Task 1: Fail-Closed Process, Window, Coordinate, Fixture and Evidence Guards

**Files:**
- Create: `scripts/viewer-native-acceptance.test.mjs`
- Create: `scripts/viewer-native-acceptance.mjs`

**Interfaces:**
- Consumes: `ps -axo pid=,ppid=,pgid=,command=`, current repository root, target commit, viewport, ledger ID and run ID.
- Produces:
  - `parseProcessTable(output: string): ProcessInfo[]`
  - `selectExactViewer(processes, { repoRoot, executablePath, controllerPid, helperPid? }): ProcessInfo`
  - `validateWindow(window, { pid, viewport }): WindowInfo`
  - `validateWindowPoint(point, window): { x: number, y: number }`
  - `validateFixturePath(candidate, { repoRoot, runId, realpath? }): string`
  - `validateEvidencePath(candidate, { repoRoot, commit, viewport, id }): string`
  - `validateCommand(request): AcceptanceRequest`
  - stable `AcceptanceError` codes for every refusal.

- [ ] **Step 1: Write failing process-selection tests**

Create the test file with real data objects and no process mocks beyond supplying the process-table string:

```js
import assert from 'node:assert/strict'
import { describe, it } from 'node:test'

import {
  parseProcessTable,
  selectExactViewer,
} from './viewer-native-acceptance.mjs'

const repoRoot = '/Users/example/Project/Viewer/.worktrees/atlas'
const executablePath = `${repoRoot}/target/debug/viewer-desktop`

describe('selectExactViewer', () => {
  it('returns the only exact current-worktree bare development process', () => {
    const processes = parseProcessTable(`
      101 1 101 ${executablePath}
      202 1 202 /Users/example/Project/Viewer/target/debug/viewer-desktop
    `)

    assert.equal(
      selectExactViewer(processes, {
        repoRoot,
        executablePath,
        controllerPid: 999,
      }).pid,
      101,
    )
  })

  it('rejects zero, duplicate, packaged, other-worktree and self processes', () => {
    const exact = { pid: 101, ppid: 1, pgid: 101, command: executablePath }
    assert.throws(
      () => selectExactViewer([], { repoRoot, executablePath, controllerPid: 999 }),
      { code: 'PRECONDITION_VIEWER_COUNT' },
    )
    assert.throws(
      () => selectExactViewer([exact, { ...exact, pid: 102 }], { repoRoot, executablePath, controllerPid: 999 }),
      { code: 'PRECONDITION_VIEWER_COUNT' },
    )
    assert.throws(
      () => selectExactViewer([{ ...exact, command: `${repoRoot}/target/debug/bundle/macos/Viewer.app/Contents/MacOS/viewer-desktop` }], { repoRoot, executablePath, controllerPid: 999 }),
      { code: 'PRECONDITION_VIEWER_PATH' },
    )
    assert.throws(
      () => selectExactViewer([exact], { repoRoot, executablePath, controllerPid: 101 }),
      { code: 'PRECONDITION_VIEWER_SELF' },
    )
  })
})
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
node --test scripts/viewer-native-acceptance.test.mjs
```

Expected: FAIL because `scripts/viewer-native-acceptance.mjs` does not exist or the named exports are missing.

- [ ] **Step 3: Implement process parsing and exact selection minimally**

Create `viewer-native-acceptance.mjs` with `AcceptanceError`, `commandExecutable`, `parseProcessTable`, candidate classification and `selectExactViewer`. Candidate discovery must detect every repository/path-ending `viewer-desktop` and every packaged `Viewer.app` process before requiring exactly one current-worktree exact process; it must not silently ignore a second Viewer from another worktree.

Use this public error shape:

```js
export class AcceptanceError extends Error {
  constructor(code, message, details = {}) {
    super(message)
    this.name = 'AcceptanceError'
    this.code = code
    this.details = details
  }
}
```

- [ ] **Step 4: Run the focused test and verify GREEN**

Run `node --test scripts/viewer-native-acceptance.test.mjs`.

Expected: all process-selection tests PASS with zero warnings.

- [ ] **Step 5: Write failing window and coordinate tests**

Add tests that accept `{ pid: 101, windowId: 44, x: 20, y: 30, width: 1024, height: 720 }`, reject wrong PID, multiple/missing main windows, non-integer window IDs, `1023 × 720`, `1440 × 899`, and points on or outside the right/bottom edge. Assert error codes `PRECONDITION_WINDOW_OWNER`, `PRECONDITION_WINDOW_COUNT`, `PRECONDITION_VIEWPORT`, and `SAFETY_POINT_OUTSIDE_WINDOW`.

- [ ] **Step 6: Run the focused tests and verify RED**

Run `node --test --test-name-pattern='window|coordinate' scripts/viewer-native-acceptance.test.mjs`.

Expected: FAIL because `validateWindow` and `validateWindowPoint` are missing.

- [ ] **Step 7: Implement exact window and point validation**

Implement both exports. Only `{ width: 1024, height: 720 }` and `{ width: 1440, height: 900 }` are legal. Convert window-relative points to screen points only after checking `0 <= x < width` and `0 <= y < height`; never clamp invalid input.

- [ ] **Step 8: Run focused tests and verify GREEN**

Run the Step 6 command.

Expected: all window and coordinate tests PASS.

- [ ] **Step 9: Write failing fixture and evidence path tests**

Use `mkdtemp`, `mkdir`, `realpath`, and `symlink` to create a real temporary worktree-shaped source and an exact home-scoped run. Test that only `$HOME/ViewerAcceptanceRuns/run-123/**` is writable, while `/`, the home directory, repository root, the read-only `ViewerAcceptance` baseline, sibling run IDs, `..` escape and a symlink escape are rejected. Test that cleanup removes only the exact run ID. Screenshots remain accepted only below `target/atlas-product-migration-acceptance/abc123/1024x720/FIL-01/` or the exact corresponding `1440x900` directory, with error codes `SAFETY_FIXTURE_PATH` and `SAFETY_EVIDENCE_PATH`.

- [ ] **Step 10: Run path tests and verify RED**

Run:

```bash
node --test --test-name-pattern='fixture|evidence' scripts/viewer-native-acceptance.test.mjs
```

Expected: FAIL because the path validators are missing.

- [ ] **Step 11: Implement realpath-based path validators**

Implement containment with `path.relative` plus `realpath` for existing ancestors. Reject absolute-path mismatch, empty strings, glob metacharacters, unresolved `$`/`~`, symlinks, run-ID mismatch, commit mismatch, viewport mismatch and ID mismatch. Creation targets may be absent only when their nearest existing ancestor resolves inside the approved root.

- [ ] **Step 12: Run the complete Task 1 test file**

Run `node --test scripts/viewer-native-acceptance.test.mjs`.

Expected: all Task 1 tests PASS.

- [ ] **Step 13: Commit Task 1**

```bash
git add scripts/viewer-native-acceptance.mjs scripts/viewer-native-acceptance.test.mjs
git commit -m "test: guard Viewer native acceptance scope"
```

---

### Task 2: Strict JSON-Lines Protocol and Swift Protocol-Test Helper

**Files:**
- Modify: `scripts/viewer-native-acceptance.mjs`
- Modify: `scripts/viewer-native-acceptance.test.mjs`
- Create: `scripts/viewer-native-acceptance.swift`

**Interfaces:**
- Consumes: newline-delimited UTF-8 JSON over child stdin/stdout.
- Produces:
  - `PROTOCOL_VERSION = 1`
  - `ALLOWED_COMMANDS = new Set(['inspect','query','activate','focus','setValue','key','pointer','drag','capture','shutdown'])`
  - `validateCommand(request): AcceptanceRequest`
  - `NativeAcceptanceClient` with `start()`, `request(command, payload, options)`, and `close()`.
  - Swift `--protocol-test` mode that parses and validates without Accessibility access or event emission.

- [ ] **Step 1: Write failing command-schema tests**

Test a valid `inspect` request and reject protocol version `2`, sequence `0`, PID `0`, window ID `0`, unknown command, extra top-level field, timeout outside `100...10000`, text above 4096 Unicode scalars, unapproved key combinations and pointer/drag coordinates outside the validated window. Assert `SAFETY_PROTOCOL` or `SAFETY_COMMAND` as appropriate.

- [ ] **Step 2: Run command-schema tests and verify RED**

Run `node --test --test-name-pattern='protocol schema' scripts/viewer-native-acceptance.test.mjs`.

Expected: FAIL because protocol constants and `validateCommand` are missing.

- [ ] **Step 3: Implement the minimum strict schema validator**

Use explicit own-key sets for the common request and each payload variant. Do not accept or discard unknown fields. Allow keys `tab`, `enter`, `space`, `escape`, `arrowUp`, `arrowDown`, `arrowLeft`, `arrowRight`, `home`, `end`, `delete`, `backspace`, `a`, `c`, and `v`; allow only `shift`, `control`, `option`, and `command` modifiers.

- [ ] **Step 4: Run command-schema tests and verify GREEN**

Run the Step 2 command.

Expected: all schema tests PASS.

- [ ] **Step 5: Write failing Swift helper build and protocol tests**

Add a test that runs:

```bash
xcrun swiftc -warnings-as-errors scripts/viewer-native-acceptance.swift -o <temporary-helper>
```

Then spawn `<temporary-helper> --protocol-test`, send one valid `inspect` line followed by one unknown command, and assert the exact correlated responses:

```json
{"version":1,"sequence":1,"ok":true,"result":{"mode":"protocol-test"}}
{"version":1,"sequence":2,"ok":false,"error":{"code":"SAFETY_COMMAND","message":"Unsupported command"}}
```

- [ ] **Step 6: Run Swift protocol tests and verify RED**

Run `node --test --test-name-pattern='Swift helper protocol' scripts/viewer-native-acceptance.test.mjs`.

Expected: FAIL because the Swift helper does not exist.

- [ ] **Step 7: Implement Swift Codable envelopes and protocol-test mode**

Define `RequestEnvelope`, `ResponseEnvelope`, `ProtocolError`, a fixed `Command` enum and an `@main` line-processing loop. In `--protocol-test`, validate version, positive sequence/PID/window ID, timeout range and command name; never call AX or CG APIs. Serialize every response on one line and flush stdout.

- [ ] **Step 8: Run Swift protocol tests and verify GREEN**

Run the Step 6 command.

Expected: Swift compiles with warnings treated as errors and both responses match.

- [ ] **Step 9: Write failing Node client correlation tests**

Use a temporary executable fixture that echoes delayed JSON lines. Test ordered requests, rejection of duplicate/out-of-order sequence, malformed JSON, unexpected EOF, stderr diagnostics, response timeout and child termination. The timeout test must wait on the request promise, not a fixed sleep.

- [ ] **Step 10: Run Node client tests and verify RED**

Run `node --test --test-name-pattern='NativeAcceptanceClient' scripts/viewer-native-acceptance.test.mjs`.

Expected: FAIL because the client is missing.

- [ ] **Step 11: Implement helper build caching and protocol client**

Compile the helper to `target/native-acceptance-tools/<sha256-of-swift-source>/viewer-native-acceptance-helper`; use `xcrun swiftc -warnings-as-errors`. `NativeAcceptanceClient.start()` performs an `inspect` handshake; `request()` assigns the next sequence, validates the response and uses an abortable timeout; `close()` sends `shutdown`, closes stdin and force-terminates only if graceful shutdown fails.

- [ ] **Step 12: Run all controller tests**

Run `node --test scripts/viewer-native-acceptance.test.mjs`.

Expected: every Task 1–2 test PASS.

- [ ] **Step 13: Commit Task 2**

```bash
git add scripts/viewer-native-acceptance.mjs scripts/viewer-native-acceptance.test.mjs scripts/viewer-native-acceptance.swift
git commit -m "feat: add Viewer native acceptance protocol"
```

---

### Task 3: PID-Scoped Accessibility, Input and Window Capture

**Files:**
- Modify: `scripts/viewer-native-acceptance.swift`
- Modify: `scripts/viewer-native-acceptance.test.mjs`
- Modify: `scripts/viewer-native-acceptance.mjs`

**Interfaces:**
- Consumes: a validated PID, window ID, command and payload from Task 2.
- Produces:
  - `inspect` result `{ pid, windowId, title, frame, focused, frontmost }`.
  - `query` result containing stable role, title, value, identifier, frame and child path fields.
  - real implementations for `activate`, `focus`, `setValue`, `key`, `pointer`, `drag`, and `capture`.
  - stable native errors `PRECONDITION_ACCESSIBILITY`, `PRECONDITION_WINDOW_OWNER`, `STATE_TARGET_NOT_FOUND`, `STATE_TARGET_NOT_UNIQUE`, `STATE_ACTION_FAILED`, and `CAPTURE_WINDOW_MISMATCH`.

- [ ] **Step 1: Write failing native validation tests**

Extend protocol-test mode with a deterministic in-memory accessibility tree selected by `--protocol-test-fixture`. Assert that `inspect` rejects wrong PID/window, `query` returns one named button, duplicate matches return `STATE_TARGET_NOT_UNIQUE`, no match returns `STATE_TARGET_NOT_FOUND`, and a point on the fixture window's right edge returns `SAFETY_POINT_OUTSIDE_WINDOW`.

- [ ] **Step 2: Run native validation tests and verify RED**

Run `node --test --test-name-pattern='native validation' scripts/viewer-native-acceptance.test.mjs`.

Expected: FAIL because protocol-test fixtures and native error mapping are missing.

- [ ] **Step 3: Implement shared Swift validation and deterministic fixture adapter**

Separate command dispatch from the platform adapter with a small `NativeAdapter` protocol. Implement `FixtureAdapter` for tests and preserve the production `MacAdapter` boundary. Both adapters must use the same request validation, unique-target selection, window ownership and coordinate checks.

- [ ] **Step 4: Run native validation tests and verify GREEN**

Run the Step 2 command.

Expected: all fixture-adapter tests PASS.

- [ ] **Step 5: Implement and compile the production AX inspection/query adapter**

Use `AXUIElementCreateApplication(pid_t(pid))`, require Accessibility trust, read `AXWindows`, select one standard main window, validate its `AXWindowNumber`, owner PID and exact frame, then recursively return only the approved attributes: role, subrole, title, description, value, identifier, enabled, focused, frame and child path. Cap traversal depth and node count; return an explicit error rather than truncating silently.

- [ ] **Step 6: Add a no-permission/invalid-PID smoke test**

Spawn the compiled helper without protocol-test mode using a guaranteed-invalid PID and assert `PRECONDITION_WINDOW_OWNER` or `PRECONDITION_VIEWER_PID` without a crash. If Accessibility permission is absent, assert `PRECONDITION_ACCESSIBILITY` and print the exact System Settings path; do not weaken the permission check.

- [ ] **Step 7: Implement AX actions and CoreGraphics input**

Use `AXUIElementPerformAction(kAXPressAction)` for `activate`, `kAXFocusedAttribute` for `focus`, and `kAXValueAttribute` for allowed text fields. Use `CGEvent` at `.cghidEventTap` for key and pointer input. Before each event, re-read frontmost PID, window number and frame; reject any change. Implement drag as mouse-down, condition-based interpolated movement for the requested duration, and mouse-up, always releasing the mouse in `defer` on error.

- [ ] **Step 8: Implement verified window capture**

Immediately before capture, validate PID, window ID, exact viewport and frontmost ownership. Capture only that window at best resolution with CoreGraphics and encode PNG using ImageIO to the already validated path. Return width, height and SHA-256; reject empty or dimension-mismatched output and remove an invalid partial file.

- [ ] **Step 9: Run Swift compilation and all controller tests**

Run:

```bash
xcrun swiftc -warnings-as-errors scripts/viewer-native-acceptance.swift -o target/native-acceptance-tools/manual-check
node --test scripts/viewer-native-acceptance.test.mjs
```

Expected: compiler exits `0`; every test passes; no helper process remains.

- [ ] **Step 10: Commit Task 3**

```bash
git add scripts/viewer-native-acceptance.mjs scripts/viewer-native-acceptance.test.mjs scripts/viewer-native-acceptance.swift
git commit -m "feat: drive exact Viewer process natively"
```

---

### Task 4: Disposable Fixture, 89-State Recipe Registry and Evidence Manifests

**Files:**
- Modify: `scripts/viewer-native-acceptance.mjs`
- Modify: `scripts/viewer-native-acceptance.test.mjs`
- Modify: `docs/superpowers/plans/2026-08-02-viewer-atlas-to-product-complete-migration.md`

**Interfaces:**
- Consumes: 89 ledger IDs, read-only `target/atlas-product-migration-fixture/ViewerAcceptance`, Task 1 guards and Task 3 client.
- Produces:
  - `AUDIT_IDS: readonly string[]` with exactly 89 unique IDs.
  - `STATE_RECIPES: ReadonlyMap<string, StateRecipe>` with exactly one recipe per ID.
  - `createFixtureRun({ repoRoot, runId }): Promise<FixtureRun>`.
  - `resetFixtureVariant(run, variant): Promise<string>`.
  - `buildEvidenceManifest(context): EvidenceManifest`.
  - CLI modes `--preflight`, `--list`, `--id <ID>`, `--wave <1|2|3|4>`, `--viewport <1024x720|1440x900>` and `--all`.

- [ ] **Step 1: Write failing registry coverage tests**

Parse the audit and ledger with the same table-ID rule used by `ui/src/visualMigrationCoverage.test.ts`. Assert audit IDs, ledger IDs, `AUDIT_IDS` and recipe keys are each length `89`, unique and set-equal. Assert every recipe declares fixture variant, viewport-independent steps, a non-empty visible assertion and one of waves `1...4`.

- [ ] **Step 2: Run registry tests and verify RED**

Run `node --test --test-name-pattern='recipe registry' scripts/viewer-native-acceptance.test.mjs`.

Expected: FAIL because the registry is missing.

- [ ] **Step 3: Add the exact 89-ID registry and typed recipe validation**

Populate IDs by the authoritative ranges:

```text
LAU-01..09
SID-01..04  STR-01..05  THU-01..07  OTH-01..03
SEA-01..05  FIL-01..04  MEN-01..03
RAD-01..07
PRE-01..07  COM-01..04
DOC-01..07  INF-01..02
DIA-01..07
TAS-01..05  RES-01..05
A11Y-01..05
```

Encode the ledger's `Native entry recipe` as real user-action steps. Shared setup may be composed from named immutable step arrays, but each final recipe must be materialized and validated independently; no recipe may be an empty alias.

- [ ] **Step 4: Run registry tests and verify GREEN**

Run the Step 2 command.

Expected: exact set equality PASS with 89 states.

- [ ] **Step 5: Write failing disposable-fixture tests**

Create a temporary baseline containing images, documents, empty folder, corrupt file and conflict pairs. Assert `createFixtureRun` copies it under `runs/<run-id>/`, never mutates the baseline, rejects an existing run ID, records a baseline manifest, and `resetFixtureVariant` restores deleted/renamed files deterministically. Include a symlink in the temporary baseline and assert the copy is rejected.

- [ ] **Step 6: Run fixture tests and verify RED**

Run `node --test --test-name-pattern='fixture run' scripts/viewer-native-acceptance.test.mjs`.

Expected: FAIL because fixture-run functions are missing.

- [ ] **Step 7: Implement safe fixture copy/reset**

Use `fs.cp` with explicit dereference disabled, pre-scan with `lstat`, reject symbolic links, and write `fixture-manifest.json` containing relative path, size and SHA-256 for every file. Reset by creating a fresh variant directory and atomically renaming it into place; never recursively delete outside the validated current run root.

- [ ] **Step 8: Write failing evidence-manifest and CLI tests**

Assert manifests contain schema version, ID, wave, commit, branch, dirty flag, PID, executable realpath, window ID/frame, viewport, fixture run/variant, ordered actions, assertion result, raw PNG path/hash, reference path/hash, combined path/hash, timestamp and verdict. Assert CLI rejects dirty worktrees for capture, unknown IDs, conflicting selectors, missing viewport, unsafe output roots and process/window mismatch; `--list` and `--preflight` remain non-destructive.

- [ ] **Step 9: Run manifest/CLI tests and verify RED**

Run `node --test --test-name-pattern='manifest|CLI' scripts/viewer-native-acceptance.test.mjs`.

Expected: FAIL because manifest and CLI functions are missing.

- [ ] **Step 10: Implement manifests, CLI parsing and condition waits**

Write one `manifest.json` and `actions.jsonl` per ID/viewport. Provide `waitFor(predicate, { timeoutMs, intervalMs })` using condition polling capped at 10 seconds; no acceptance recipe may contain an arbitrary sleep step. `--all` runs only after preflight and requires a clean commit.

- [ ] **Step 11: Add the controller prerequisite to the parent Task 15 plan**

Modify Task 15 to list the three script files and this plan. Add a Step 0 that runs:

```bash
node --test scripts/viewer-native-acceptance.test.mjs
node scripts/viewer-native-acceptance.mjs --preflight --viewport 1024x720
```

State that both must pass before Task 15 capture and that the controller never replaces joint visual review.

- [ ] **Step 12: Run Task 4 tests and parent coverage gate**

Run:

```bash
node --test scripts/viewer-native-acceptance.test.mjs
pnpm test:policy
```

Expected: controller tests pass; policy reports 28 passing tests and 47-row scope coverage or the current higher passing count.

- [ ] **Step 13: Commit Task 4**

```bash
git add scripts/viewer-native-acceptance.mjs scripts/viewer-native-acceptance.test.mjs docs/superpowers/plans/2026-08-02-viewer-atlas-to-product-complete-migration.md
git commit -m "feat: map Viewer native acceptance states"
```

---

### Task 5: Non-Shipping Commands and Real Bare-Process Smoke Gate

**Files:**
- Modify: `package.json`
- Modify: `scripts/viewer-native-acceptance.test.mjs`
- Modify: `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`
- Modify: `docs/reviews/2026-07-30-viewer-ui-visual-upgrade-verification.md`

**Interfaces:**
- Consumes: canonical `pnpm start:viewer`, controller preflight, one clean current-worktree bare Viewer and disposable fixture.
- Produces: `pnpm test:native-acceptance`, `pnpm accept:native -- ...`, one verified LAU-01 smoke manifest/capture, and recorded controller gate metadata.

- [ ] **Step 1: Write failing package-command and exclusion tests**

Parse `package.json` and Tauri configuration. Assert:

```js
assert.equal(packageJson.scripts['test:native-acceptance'], 'node --test scripts/viewer-native-acceptance.test.mjs')
assert.equal(packageJson.scripts['accept:native'], 'node scripts/viewer-native-acceptance.mjs')
```

Assert neither script/helper path appears in `src-tauri/tauri.conf.json`, bundle resources, frontend imports, Rust source or production build output.

- [ ] **Step 2: Run package-command tests and verify RED**

Run `node --test --test-name-pattern='package commands|non-shipping' scripts/viewer-native-acceptance.test.mjs`.

Expected: FAIL because the package commands are absent.

- [ ] **Step 3: Add only the two non-shipping package commands**

Modify root `package.json` scripts with the exact values above. Do not alter `start:viewer`, Tauri configuration or release resources.

- [ ] **Step 4: Run focused and complete controller tests**

Run:

```bash
pnpm test:native-acceptance
pnpm test:policy
```

Expected: both commands exit `0`.

- [ ] **Step 5: Establish the exact 1024 native smoke process**

Verify the worktree is clean, launch only through `pnpm start:viewer` with the existing exact-viewport Tauri config, and run:

```bash
pnpm accept:native -- --preflight --viewport 1024x720
pnpm accept:native -- --id LAU-01 --viewport 1024x720
```

Expected: one exact bare Viewer PID; main window exactly `1024 × 720`; LAU-01 manifest, action log and PNG under the current commit directory; no fixture mutation because LAU-01 is non-destructive.

- [ ] **Step 6: Inspect the smoke evidence jointly**

Put the approved LAU-01 reference and the new native screenshot in one combined comparison image. Verify PID, window, commit and hash from the manifest, then inspect typography, padding, alignment, radius, border and focus. If a product difference exists, follow systematic debugging and TDD in the formal product code, recapture, and do not patch the evidence generator.

- [ ] **Step 7: Record controller smoke metadata**

Update the ledger header and verification document with controller commit, helper source hash, commands, exit codes, exact PID/executable/window/viewport and smoke evidence paths. Do not mark any additional row `pass` until both viewports and visual verdict exist.

- [ ] **Step 8: Restore user environment and verify the single process**

Restore display mode, Dock preferences and accessibility settings to recorded original values. Keep exactly one latest current-worktree Viewer process at the user's normal development viewport and verify no helper process remains.

- [ ] **Step 9: Run the Task 5 gate and commit**

Run:

```bash
pnpm test:native-acceptance
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
pnpm test:policy
```

Expected: all commands exit `0`; UI and policy counts are recorded from fresh output.

Then commit:

```bash
git add package.json scripts/viewer-native-acceptance.test.mjs docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md docs/reviews/2026-07-30-viewer-ui-visual-upgrade-verification.md
git commit -m "test: verify Viewer native acceptance control"
```

---

### Task 6: Execute Remaining 88 States, Correct Formal Product UI and Close Task 15

**Files:**
- Modify as required by a proven visual difference: formal product files listed in Tasks 6–14 of `docs/superpowers/plans/2026-08-02-viewer-atlas-to-product-complete-migration.md`
- Modify: `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`
- Modify: `docs/reviews/2026-07-30-viewer-ui-visual-upgrade-verification.md`
- Generated but not staged: `target/atlas-product-migration-acceptance/<commit>/**`

**Interfaces:**
- Consumes: all 89 validated recipes, the approved atlas states, current formal product code and exact dual viewports.
- Produces: 89/89 current-commit automated evidence, 89/89 `1024 × 720` joint comparisons, 89/89 `1440 × 900` joint comparisons, zero P0/P1/P2 differences, closed ledger and one latest runnable development process.

- [ ] **Step 1: Capture and judge Wave 1 at 1024 × 720**

Run the controller for LAU/SID/STR/THU/OTH/SEA/FIL/MEN states at `1024x720`. For each ID, open `combined.png`, inspect the reference and native product together, and record a verdict. On any difference, write a failing component/style test that names the approved rule, observe RED, minimally correct formal product code, run GREEN, relaunch the new commit and invalidate all older-commit screenshots.

- [ ] **Step 2: Capture and judge Wave 1 at 1440 × 900**

Repeat the exact same recipes and fixture variants at `1440x900`. Do not reuse the 1024 verdict. Require exact current-commit manifests and joint comparisons for every row.

- [ ] **Step 3: Capture and judge Wave 2 at both viewports**

Run RAD/PRE/COM/DOC/INF recipes at `1024x720`, inspect and correct, then repeat at `1440x900`. Verify radial pointer/keyboard geometry, preview loading/error/navigation, comparison counts/overflow, text encodings/truncation/dual-pane and both inspectors.

- [ ] **Step 4: Capture and judge Wave 3 at both viewports**

Run DIA/TAS/RES recipes at both viewports. Destructive recipes must start from a reset disposable fixture variant and retain before/after manifests. Verify dialog focus/actions, real progress semantics, local feedback, operation results and collision states.

- [ ] **Step 5: Capture and judge Wave 4 accessibility states**

Exercise A11Y-01..05 with real keyboard input and system settings. Record and restore Reduce Motion, Increase Contrast, Reduce Transparency, system appearance and effective zoom. Pair A11Y-04 macOS high-contrast evidence with the passing forced-colors CSS contract; do not claim Windows-native forced-colors evidence.

- [ ] **Step 6: Close the 89-row ledger only from current evidence**

Require every row to have named passing automated evidence, a current-commit 1024 combined path, a current-commit 1440 combined path and `Result = pass`. A script check must fail on `not-recorded`, `pending`, `blocked`, wrong commit paths, missing files, missing manifest fields, duplicate IDs or any nonzero P0/P1/P2 verdict.

- [ ] **Step 7: Run the complete final gate fresh**

Run:

```bash
pnpm test:native-acceptance
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
pnpm test:policy
pnpm security
node --test scripts/viewer-dev-launcher.test.mjs
cargo fmt --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Expected: every command exits `0`; counts and any non-failing informational output are recorded verbatim in the verification document.

- [ ] **Step 8: Verify repository and runtime invariants**

Confirm `target/atlas-product-migration-acceptance/` is ignored and unstaged; the ledger/verification docs point only to the final current commit; no packaged Viewer or second development Viewer runs; no helper remains; the one current process executable is the exact worktree `target/debug/viewer-desktop`; the app opens at the user's restored normal development viewport.

- [ ] **Step 9: Commit the final evidence index**

```bash
git add docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md docs/reviews/2026-07-30-viewer-ui-visual-upgrade-verification.md
git commit -m "docs: verify complete Viewer UI migration"
```

Do not stage generated evidence.

## Plan Self-Review

- **Spec coverage:** Tasks 1–3 cover process/window/path/command gates, JSON-lines protocol, AX/CG input and capture. Task 4 covers disposable fixtures, all 89 recipes and manifests. Task 5 proves non-shipping integration and a real bare-process smoke state. Task 6 preserves the parent plan's complete dual-viewport visual, accessibility, correction and closure gates.
- **Placeholder scan:** The plan contains no deferred implementation instruction, undefined placeholder or generic “handle errors” step. Angle-bracket path segments denote runtime-generated values defined by their surrounding interfaces.
- **Type consistency:** `ProcessInfo`, `WindowInfo`, `AcceptanceRequest`, `NativeAcceptanceClient`, `StateRecipe`, `FixtureRun` and `EvidenceManifest` are introduced before consumption; command names, error codes, viewport strings and output roots are consistent across tasks.
- **Scope check:** The controller is one bounded non-shipping subsystem. The 89-state execution remains the already approved parent Task 15 rather than creating a second product architecture.
