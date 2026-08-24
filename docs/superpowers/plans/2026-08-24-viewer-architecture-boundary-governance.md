# Viewer Architecture Boundary Governance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> Status: Active

**Goal:** Separate blocking architecture boundaries from non-blocking maintenance trends, formally classify the media driver dependency, and make the boundary gate part of Viewer’s normal verification path.

**Architecture:** Keep dependency and UI-import enforcement in a focused blocking CLI, while retaining source-size and complexity analysis in a separate reporting CLI. Reuse the existing bridge, repository-policy, and Rust security tests as the frozen external-contract suite instead of creating a second DTO authority.

**Tech Stack:** Node.js 24 ESM, Node built-in test runner, Cargo metadata, pnpm 10, Vitest, Rust/Cargo, Markdown/JSON governance records.

**Spec:** `docs/superpowers/specs/2026-08-24-viewer-architecture-governance-and-workspace-orchestration-design.md`

**Execution order:** This plan must complete before `docs/superpowers/plans/2026-08-24-viewer-workspace-orchestration-refactor.md` begins.

## Global Constraints

- This is architecture-only work: add no product feature and change no user-visible behavior.
- Keep Tauri command names, event names, DTO shapes, capabilities, CSP, SQLite schemas, `.viewer` schemas, shortcuts, accessibility semantics, copy, and error timing unchanged.
- Treat `viewer-video-mpv` as a low-level driver that may be used by `viewer-infrastructure`, `viewer-platform-macos`, and `viewer-desktop`; do not split the crate in this round.
- Keep `viewer-desktop` as the only production composition root.
- Ignore dev-dependencies when enforcing production direction, but enforce both normal and build dependencies.
- Blocking checks fail on illegal dependency direction, production cycles, direct production UI imports of Tauri, or frozen-contract drift.
- Trend findings remain non-blocking; never lower coverage, broaden ignore lists, or blindly refresh a baseline.
- Every task ends in an independently testable commit and must leave the working tree clean.
- If a task requires changing a frozen contract, weakening a gate, or merging an unverified phase, stop and return to architecture review instead of continuing.

---

## File Structure

### Blocking boundaries

- `scripts/architecture-boundaries.mjs` — collect Rust workspace production edges, detect forbidden edges/cycles, and reject direct production UI imports of Tauri.
- `scripts/architecture-boundaries.test.mjs` — unit and CLI mutation tests for the blocking boundary rules.
- `package.json` — expose `architecture:contracts` and `architecture:boundaries`, and add the boundary gate to `quality`.
- `scripts/repository-policy.test.mjs` — freeze command composition and capability assertions.

### Non-blocking trends

- `scripts/architecture-trends.mjs` — source line, function length, decision-score, and test-ratio reporting only.
- `scripts/architecture-trends.test.mjs` — metric, classification validation, and non-blocking CLI tests.
- `docs/quality/architecture-trends-baseline.json` — reviewed metric identities and current values.
- `docs/quality/architecture-trend-classifications.json` — explicit owner, rationale, classification, and review trigger for every baseline outlier.

### Removed compatibility names

- Delete: `scripts/architecture-health.mjs`
- Delete: `scripts/architecture-health.test.mjs`
- Delete: `docs/quality/architecture-health-baseline.json`

### Active documentation

- `README.md` — distinguish the mandatory boundary gate from optional trend evidence.
- `CONTRIBUTING.md` — require boundary verification and reviewed trend classification.
- `docs/TECHNICAL_FOUNDATIONS.md` — record the driver layer and governance split.

---

### Task 1: Establish the Rust production dependency boundary

**Files:**

- Create: `scripts/architecture-boundaries.mjs`
- Create: `scripts/architecture-boundaries.test.mjs`
- Read from: `scripts/architecture-health.mjs:560-623`

**Interfaces:**

- Consumes: `cargo metadata --locked --no-deps --format-version 1`.
- Produces:

```js
readCargoMetadata(root, execFile?): object
collectWorkspaceDependencyEdges(metadata): WorkspaceEdge[]
findForbiddenWorkspaceEdges(edges): WorkspaceEdge[]
findWorkspaceDependencyCycles(edges): string[][]
runArchitectureBoundariesCli({ root, stdout, stderr }): number
```

where `WorkspaceEdge` is `{ from: string, to: string, kind: 'normal' | 'build' | 'dev' }`.

- [ ] **Step 1: Write failing dependency-direction and cycle tests**

Create tests that include both newly accepted video-driver edges and rejected reversals:

```js
test('accepts the approved production graph including the media driver', () => {
  const edges = [
    { from: 'viewer-application', to: 'viewer-domain', kind: 'normal' },
    { from: 'viewer-infrastructure', to: 'viewer-application', kind: 'normal' },
    { from: 'viewer-infrastructure', to: 'viewer-domain', kind: 'normal' },
    { from: 'viewer-infrastructure', to: 'viewer-video-mpv', kind: 'normal' },
    { from: 'viewer-platform-macos', to: 'viewer-application', kind: 'normal' },
    { from: 'viewer-platform-macos', to: 'viewer-domain', kind: 'build' },
    { from: 'viewer-platform-macos', to: 'viewer-video-mpv', kind: 'normal' },
    { from: 'viewer-desktop', to: 'viewer-platform-macos', kind: 'normal' },
    { from: 'viewer-test-support', to: 'viewer-domain', kind: 'normal' },
  ]
  assert.deepEqual(findForbiddenWorkspaceEdges(edges), [])
  assert.deepEqual(findWorkspaceDependencyCycles(edges), [])
})

test('rejects reversals and production cycles while ignoring dev-only cycles', () => {
  const forbidden = { from: 'viewer-domain', to: 'viewer-application', kind: 'normal' }
  const cycle = [
    { from: 'viewer-application', to: 'viewer-domain', kind: 'normal' },
    { from: 'viewer-domain', to: 'viewer-application', kind: 'normal' },
  ]
  assert.deepEqual(findForbiddenWorkspaceEdges([forbidden]), [forbidden])
  assert.deepEqual(findWorkspaceDependencyCycles(cycle), [
    ['viewer-application', 'viewer-domain', 'viewer-application'],
  ])
  assert.deepEqual(findWorkspaceDependencyCycles([
    { from: 'viewer-domain', to: 'viewer-application', kind: 'dev' },
  ]), [])
})
```

- [ ] **Step 2: Run the tests and verify the new module is missing**

Run:

```bash
node --test scripts/architecture-boundaries.test.mjs
```

Expected: FAIL because `scripts/architecture-boundaries.mjs` and its exports do not exist.

- [ ] **Step 3: Move the workspace graph logic behind an explicit allow map**

Implement the production targets exactly as follows; `undefined` is not used as an unrestricted escape hatch:

```js
export const PRODUCTION_DEPENDENCY_TARGETS = new Map([
  ['viewer-domain', new Set()],
  ['viewer-application', new Set(['viewer-domain'])],
  ['viewer-infrastructure', new Set([
    'viewer-application',
    'viewer-domain',
    'viewer-video-mpv',
  ])],
  ['viewer-platform-macos', new Set([
    'viewer-application',
    'viewer-domain',
    'viewer-video-mpv',
  ])],
  ['viewer-video-mpv', new Set()],
  ['viewer-test-support', new Set(['viewer-application', 'viewer-domain'])],
  ['viewer-desktop', new Set([
    'viewer-application',
    'viewer-domain',
    'viewer-infrastructure',
    'viewer-platform-macos',
    'viewer-video-mpv',
  ])],
])
```

Reuse the existing deterministic Cargo metadata invocation and sorted edge collection. For `findForbiddenWorkspaceEdges`, skip only `kind === 'dev'`; an unknown Viewer package or target is forbidden. Implement deterministic depth-first cycle detection over non-dev edges and canonicalize each reported cycle to start at its lexicographically smallest package.

- [ ] **Step 4: Make the CLI fail for either violation class**

`runArchitectureBoundariesCli` must emit stable messages and exit `1` when violations exist:

```text
ERROR: forbidden workspace dependency: viewer-domain -> viewer-application (normal)
ERROR: workspace dependency cycle: viewer-application -> viewer-domain -> viewer-application
```

When no violation exists, emit:

```text
Architecture boundary check passed.
```

- [ ] **Step 5: Run focused and live-graph verification**

Run:

```bash
node --test scripts/architecture-boundaries.test.mjs
node scripts/architecture-boundaries.mjs
```

Expected: all Node tests PASS, and the live graph passes with both video-driver edges accepted.

- [ ] **Step 6: Commit the Rust boundary**

```bash
git add scripts/architecture-boundaries.mjs scripts/architecture-boundaries.test.mjs
git commit -m "build: enforce Viewer workspace dependency boundaries"
```

---

### Task 2: Add the production UI-to-Tauri import boundary

**Files:**

- Modify: `scripts/architecture-boundaries.mjs`
- Modify: `scripts/architecture-boundaries.test.mjs`
- Read from: `ui/src/api/viewer.ts:1-5`
- Exception roots: `ui/src/acceptance/`, `*.test.ts`, `*.test.tsx`

**Interfaces:**

- Consumes: TypeScript/TSX files below `ui/src`.
- Produces:

```js
collectProductionUiFiles(root): Map<string, string>
findForbiddenTauriImports(files): Array<{ path: string, specifier: string }>
```

- [ ] **Step 1: Write failing import-boundary mutation tests**

Add tests using an in-memory file map:

```js
test('allows the API adapter and acceptance harness but rejects production component imports', () => {
  const files = new Map([
    ['ui/src/api/viewer.ts', "import { invoke } from '@tauri-apps/api/core'\n"],
    ['ui/src/components/Card.tsx', "import { invoke } from '@tauri-apps/api/core'\n"],
    ['ui/src/acceptance/scene.tsx', "import { listen } from '@tauri-apps/api/event'\n"],
    ['ui/src/components/Card.test.tsx', "import { invoke } from '@tauri-apps/api/core'\n"],
  ])
  assert.deepEqual(findForbiddenTauriImports(files), [{
    path: 'ui/src/components/Card.tsx',
    specifier: '@tauri-apps/api/core',
  }])
})
```

Also test static `import`, side-effect `import`, and dynamic `import()` syntax for both `@tauri-apps/api/*` and `@tauri-apps/plugin-*`.

- [ ] **Step 2: Run the focused test and verify failure**

Run:

```bash
node --test --test-name-pattern='UI|Tauri|import' scripts/architecture-boundaries.test.mjs
```

Expected: FAIL because the UI collection and import-check exports do not exist.

- [ ] **Step 3: Implement the production-file and import rules**

Use these exact exclusions:

```js
const isProductionUiPath = (path) =>
  path.startsWith('ui/src/')
  && !path.startsWith('ui/src/acceptance/')
  && !/\.test\.tsx?$/.test(path)

const isTauriSpecifier = (specifier) =>
  specifier === '@tauri-apps/api'
  || specifier.startsWith('@tauri-apps/api/')
  || specifier.startsWith('@tauri-apps/plugin-')

const ALLOWED_TAURI_UI_FILES = new Set(['ui/src/api/viewer.ts'])
```

Scan only `.ts` and `.tsx` files. Return violations sorted by path, then specifier. Do not flag type-only imports from `ui/src/api/viewer.ts`; do flag imports from Tauri packages regardless of whether the imported binding is used as a type.

- [ ] **Step 4: Integrate violations into the boundary CLI**

Emit one line per violation and return `1`:

```text
ERROR: forbidden production UI Tauri import: ui/src/components/Card.tsx -> @tauri-apps/api/core
```

The CLI succeeds only when the Rust graph and UI import checks both succeed.

- [ ] **Step 5: Run tests and the live boundary**

```bash
node --test scripts/architecture-boundaries.test.mjs
node scripts/architecture-boundaries.mjs
```

Expected: PASS. The existing direct imports under `ui/src/acceptance/` are explicitly treated as acceptance-only exceptions; no production component imports Tauri.

- [ ] **Step 6: Commit the UI boundary**

```bash
git add scripts/architecture-boundaries.mjs scripts/architecture-boundaries.test.mjs
git commit -m "build: block direct Tauri imports from production UI"
```

---

### Task 3: Bind frozen contracts and architecture boundaries into `verify`

**Files:**

- Modify: `package.json:18-29`
- Modify: `scripts/repository-policy.test.mjs:830-851,1386-1393`
- Test: `ui/src/api/viewer.test.ts`
- Test: `ui/src/state/viewerControllerContract.test.tsx`
- Test: `tests/security_boundaries.rs`
- Test: `tests/video_security_boundaries.rs`

**Interfaces:**

- Consumes: the existing bridge invocation/listener tests, controller facade type test, capability policy assertion, and Rust security DTO tests.
- Produces: self-contained `pnpm architecture:contracts` and `pnpm architecture:boundaries` commands; `pnpm quality` calls the boundary command.

- [ ] **Step 1: Change policy expectations first**

Rename the policy test `M3 adds no broad desktop capability or network/update dependency` to `desktop capability and network boundary remains frozen`. Add assertions for the new scripts:

```js
assert.equal(
  packageJson.scripts['architecture:contracts'],
  "node --test --test-name-pattern='desktop capability and network boundary remains frozen' scripts/repository-policy.test.mjs && pnpm --dir ui exec vitest run src/api/viewer.test.ts src/state/viewerControllerContract.test.tsx && cargo test --locked -p viewer-desktop --test security_boundaries --test video_security_boundaries",
)
assert.equal(
  packageJson.scripts['architecture:boundaries'],
  'node scripts/architecture-boundaries.mjs && pnpm architecture:contracts',
)
assert.match(packageJson.scripts.quality, /pnpm architecture:boundaries/)
assert.doesNotMatch(packageJson.scripts.verify, /architecture:trends|quality:report/)
```

- [ ] **Step 2: Run the policy test and verify it fails**

```bash
node --test --test-name-pattern='capability|quality reports|architecture' scripts/repository-policy.test.mjs
```

Expected: FAIL because the scripts are not present and `quality` does not invoke the boundary gate.

- [ ] **Step 3: Add exact package scripts**

Add:

```json
"architecture:contracts": "node --test --test-name-pattern='desktop capability and network boundary remains frozen' scripts/repository-policy.test.mjs && pnpm --dir ui exec vitest run src/api/viewer.test.ts src/state/viewerControllerContract.test.tsx && cargo test --locked -p viewer-desktop --test security_boundaries --test video_security_boundaries",
"architecture:boundaries": "node scripts/architecture-boundaries.mjs && pnpm architecture:contracts"
```

Insert `pnpm architecture:boundaries` in `quality` immediately after `pnpm test:policy`. Do not add trend or coverage commands to `verify`.

- [ ] **Step 4: Run each contract owner directly**

```bash
pnpm architecture:contracts
pnpm architecture:boundaries
node --test scripts/repository-policy.test.mjs
```

Expected: all commands PASS. The package command must not rewrite any baseline or source file.

- [ ] **Step 5: Commit the formal gate**

```bash
git add package.json scripts/repository-policy.test.mjs
git commit -m "build: add architecture boundaries to verification"
```

---

### Task 4: Establish a parallel trends-only reporter

**Files:**

- Create: `scripts/architecture-trends.mjs`
- Create: `scripts/architecture-trends.test.mjs`
- Retain for one compatibility commit: `scripts/architecture-health.mjs`
- Retain for one compatibility commit: `scripts/architecture-health.test.mjs`

**Interfaces:**

- Consumes: Rust/TypeScript source files and a reviewed trends baseline.
- Produces:

```js
measureSourceFiles(files): SourceMeasurements
measureFunctions(files): FunctionMeasurements
collectSourceFiles(root): Map<string, string>
collectArchitectureTrends(root): ArchitectureTrends
compareArchitectureTrends(current, baseline): TrendFinding[]
runArchitectureTrendsCli(argv, dependencies?): number
```

`ArchitectureTrends` must not contain `workspaceDependencyEdges`.

- [ ] **Step 1: Copy the source-metric tests into the new responsibility**

Create `architecture-trends.test.mjs` from the existing production/test line, function-length, decision-score, source collection, comparison, and import-safety tests. Change imports to `./architecture-trends.mjs`. Do not copy the workspace dependency, forbidden-edge, Cargo metadata, or mixed CLI tests; Tasks 1-2 already moved their ownership to `architecture-boundaries.test.mjs`.

Keep the old health files unchanged in this task so the existing `package.json` command remains valid until Task 5 performs the atomic command/baseline cutover.

- [ ] **Step 2: Write the new CLI behavior test before changing implementation**

Replace the mixed boundary/trend CLI test with:

```js
test('trend checks report regressions without blocking and never inspect dependencies', () => {
  const errors = []
  const result = runArchitectureTrendsCli(
    ['--check', '/tmp/baseline.json', '/tmp/classifications.json'],
    {
      collect: () => trends({
        filesOver1000Lines: [{ path: 'ui/src/new.ts', lines: 1001 }],
      }),
      readJson: (path) => path.endsWith('baseline.json') ? trends() : classifications(),
      stdout: { write() {} },
      stderr: { write: (value) => errors.push(value) },
    },
  )
  assert.equal(result, 0)
  assert.match(errors.join(''), /WARNING: unclassified trend finding/)
})
```

Define test helpers `trends()` and `classifications()` with schema version `1` and no workspace edge field.

- [ ] **Step 3: Run the renamed tests and verify failure**

```bash
node --test scripts/architecture-trends.test.mjs
```

Expected: FAIL because the new trends module does not exist.

- [ ] **Step 4: Remove dependency enforcement and write support from the trends CLI**

Copy only the source measurement, collection, comparison, schema validation, and CLI code into `architecture-trends.mjs`. Do not include `readCargoMetadata`, `collectWorkspaceDependencyEdges`, `findForbiddenWorkspaceEdges`, or Cargo metadata imports; their authority lives in `architecture-boundaries.mjs`.

Support only these modes:

```text
architecture-trends.mjs --report
architecture-trends.mjs --check <baseline.json> <classifications.json>
```

`--report` prints current JSON to stdout and never writes a file. `--check` prints findings and always returns `0` when inputs are valid. Invalid JSON, schema mismatch, duplicate classifications, or a baseline entry without a classification returns `1` because those are governance-data errors, not trend outcomes.

- [ ] **Step 5: Run parity and import-safety tests**

```bash
node --test scripts/architecture-trends.test.mjs
node scripts/architecture-trends.mjs --report
```

Expected: tests PASS; the report contains source metrics only and exits zero.

- [ ] **Step 6: Commit the responsibility split**

```bash
git add scripts/architecture-trends.mjs scripts/architecture-trends.test.mjs
git commit -m "build: separate architecture trend reporting"
```

---

### Task 5: Establish reviewed trend classifications and update active documentation

**Files:**

- Rename: `docs/quality/architecture-health-baseline.json` → `docs/quality/architecture-trends-baseline.json`
- Delete: `scripts/architecture-health.mjs`
- Delete: `scripts/architecture-health.test.mjs`
- Create: `docs/quality/architecture-trend-classifications.json`
- Modify: `scripts/architecture-trends.mjs`
- Modify: `scripts/architecture-trends.test.mjs`
- Modify: `package.json:18-20`
- Modify: `scripts/repository-policy.test.mjs:1386-1393`
- Modify: `README.md:108-123`
- Modify: `CONTRIBUTING.md:20-29`
- Modify: `docs/TECHNICAL_FOUNDATIONS.md`

**Interfaces:**

- Consumes: the exact outlier identities in the reviewed baseline.
- Produces: a one-to-one classification registry with entries shaped as:

```json
{
  "metric": "function-decision-score",
  "path": "ui/src/App.tsx",
  "symbol": "ViewerWorkspace",
  "classification": "governance-target",
  "owner": "Viewer maintainers",
  "rationale": "Workspace orchestration is the approved frontend architecture pilot.",
  "reviewTrigger": "Re-evaluate when the workspace orchestration plan completes."
}
```

Allowed classifications are exactly `accepted`, `governance-target`, and `test-exception`.

- [ ] **Step 1: Add classification validation tests**

Test that each baseline file/function outlier maps to exactly one classification key:

```js
assert.deepEqual(validateTrendClassifications(baseline, registry), [])
assert.deepEqual(
  validateTrendClassifications(baseline, { ...registry, entries: [] }),
  ['missing classification: file-over-1000\0ui/src/App.tsx\0'],
)
```

Also reject blank `owner`, `rationale`, or `reviewTrigger`, unknown categories, duplicate keys, and classification entries with no matching baseline outlier.

- [ ] **Step 2: Run the validator test and verify failure**

```bash
node --test --test-name-pattern='classification' scripts/architecture-trends.test.mjs
```

Expected: FAIL because the validator and registry do not exist.

- [ ] **Step 3: Capture and review the current metrics without an update command**

Run:

```bash
git mv docs/quality/architecture-health-baseline.json docs/quality/architecture-trends-baseline.json
node scripts/architecture-trends.mjs --report
```

Use `apply_patch` to replace the renamed baseline with that exact report. Do not redirect output into the repository and do not add an automatic baseline-update mode.

- [ ] **Step 4: Classify every baseline outlier explicitly**

Use exact metric/path/symbol keys. Apply these reviewed ownership decisions:

| Classification | Exact paths | Rationale and trigger |
| --- | --- | --- |
| `governance-target` | `ui/src/App.tsx` | Current approved workspace orchestration pilot; review when the dependent plan completes. |
| `test-exception` | `ui/src/acceptance/acceptanceBridge.ts`, `ui/src/acceptance/scenes/videoFeasibilityScene.tsx`, `ui/src/acceptance/scenes/viewingScenes.tsx`, `ui/src/acceptance/scenes/workspaceScenes.tsx` | Acceptance-only deterministic harness code; review if imported by production UI. |
| `accepted` | `crates/viewer-video-mpv/src/process.rs`, `src-tauri/src/video_runtime.rs`, `crates/viewer-platform-macos/src/video/media_worker.rs`, `crates/viewer-application/src/video.rs` | Focused media runtime/driver boundary outside this refactor; review on media lifecycle or crate-boundary change. |
| `accepted` | `crates/viewer-infrastructure/src/operation/**`, `crates/viewer-infrastructure/src/search/index/**`, `crates/viewer-infrastructure/src/scan/walker.rs`, `crates/viewer-application/src/undo.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/state/**`, `src-tauri/src/watcher_runtime.rs` | Existing tested backend responsibilities outside the frontend pilot; review when the named module receives a new responsibility. |
| `accepted` | All remaining exact `ui/src/components/**` and `ui/src/state/**` outlier keys in the report | Existing focused UI/state units outside root orchestration; review on a new responsibility or a further 20% size/decision increase. |

The table is a review aid, not a wildcard registry: write one JSON entry for every exact outlier key so a future new symbol cannot inherit an old approval.

- [ ] **Step 5: Rename package commands and update policy**

Use these exact scripts:

```json
"architecture:trends": "node scripts/architecture-trends.mjs --check docs/quality/architecture-trends-baseline.json docs/quality/architecture-trend-classifications.json",
"quality:report": "pnpm coverage:ui && pnpm coverage:rust && pnpm architecture:trends"
```

Remove `architecture:health`. Update the repository policy expectation to the new `quality:report` string and assert that `architecture:trends` is absent from `verify` while `architecture:boundaries` remains present through `quality`.
In the same change, delete `scripts/architecture-health.mjs` and `scripts/architecture-health.test.mjs` with `apply_patch`; the package and baseline cutover is now complete, so no active consumer remains.

- [ ] **Step 6: Update active prose only**

Document:

- `pnpm architecture:boundaries` is mandatory and deterministic;
- `pnpm architecture:trends` is non-blocking evidence;
- `pnpm quality:report` still requires the coverage toolchain;
- baseline changes require exact classification review;
- `viewer-video-mpv` is an approved low-level driver dependency.

Do not rewrite historical specs or plans that mention the former command.

- [ ] **Step 7: Verify the new governance surface**

```bash
node --test scripts/architecture-boundaries.test.mjs scripts/architecture-trends.test.mjs scripts/repository-policy.test.mjs
pnpm architecture:boundaries
pnpm architecture:trends
pnpm test:policy
git diff --check
```

Expected: all blocking commands PASS; the trends command exits zero and has no unclassified current finding.

- [ ] **Step 8: Commit the reviewed governance records**

```bash
git add package.json README.md CONTRIBUTING.md docs/TECHNICAL_FOUNDATIONS.md docs/quality scripts
git commit -m "docs: classify Viewer architecture trends"
```

---

### Task 6: Run the complete governance checkpoint

**Files:**

- Modify only if exact measured evidence improves: `docs/quality/architecture-trends-baseline.json`
- Modify only if a removed baseline key requires removal: `docs/quality/architecture-trend-classifications.json`
- Do not modify: UI/Rust production behavior.

**Interfaces:**

- Consumes: all commands established by Tasks 1-5.
- Produces: a clean, independently mergeable governance checkpoint that the workspace orchestration plan can use.

- [ ] **Step 1: Run focused governance tests**

```bash
node --test scripts/architecture-boundaries.test.mjs scripts/architecture-trends.test.mjs scripts/repository-policy.test.mjs scripts/scope-coverage.test.mjs
pnpm architecture:boundaries
pnpm architecture:trends
```

Expected: zero failed tests; the blocking boundary command exits zero; trends contain no unclassified current item.

- [ ] **Step 2: Run coverage gates**

```bash
pnpm coverage:ui
pnpm coverage:rust
```

Expected: both checked-in coverage baselines pass without lowering thresholds.

- [ ] **Step 3: Run the canonical clean verification**

```bash
pnpm verify:clean
```

Expected: exit `0`, all UI/Rust/security/build gates pass, and verification leaves no generated repository changes.

- [ ] **Step 4: Inspect repository state and architecture commands**

```bash
git status --short
rg -n "architecture:health|architecture-health" package.json README.md CONTRIBUTING.md docs/TECHNICAL_FOUNDATIONS.md scripts
```

Expected: clean worktree; no active-code or active-document references to the removed health command. Historical specs and plans are intentionally outside this search.

- [ ] **Step 5: Commit checkpoint evidence only if verification changed reviewed JSON**

If no tracked file changed, do not create an empty commit. If exact measured improvements required removing stale baseline/classification entries:

```bash
git add docs/quality/architecture-trends-baseline.json docs/quality/architecture-trend-classifications.json
git commit -m "test: refresh reviewed architecture trend evidence"
```

Expected final state: clean worktree, governance plan independently mergeable, and no workspace-orchestration implementation started.
