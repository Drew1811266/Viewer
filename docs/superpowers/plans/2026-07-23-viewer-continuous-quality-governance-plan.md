# Viewer Continuous Quality Governance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add coverage, architecture-health, dependency-exception, scheduled audit, and document-status trends without creating a release gate or M4 equivalent.

**Architecture:** Generate reproducible reports under `target/`, check in only compact baselines and governance registers, compare current metrics to recorded baselines with explicit tolerances, and keep network-dependent audits separate from deterministic pull-request gates.

**Tech Stack:** Vitest coverage-v8, cargo-llvm-cov, Node.js 24, pnpm 10, cargo-deny, GitHub Actions, Markdown/JSON governance records.

## Global Constraints

- Plans 1–5 must be complete before final baseline capture so metrics describe the decomposed architecture.
- Coverage percentage is evidence, not a substitute for safety and behavior tests.
- Initial global coverage and architecture metrics are trend baselines, not release thresholds.
- Deterministic regression checks may block ordinary changes; network-dependent audits run on schedule or manually.
- Do not add M4, final acceptance, delivery, signing, notarization, clean-install, real-media, or soak gates.
- Generated raw coverage and report artifacts live under `target/` and remain ignored.
- Checked-in baseline JSON contains numbers and schema only, never absolute local paths.
- Every advisory exception has an owner and a concrete review date.

---

### Task 1: Add Frontend Coverage and Baseline Comparison

**Files:**
- Modify: `ui/package.json`
- Modify: `ui/vite.config.ts`
- Modify: `package.json`
- Modify: `pnpm-lock.yaml`
- Modify: `scripts/repository-policy.test.mjs`
- Modify: `THIRD_PARTY_NOTICES.md`
- Create: `scripts/coverage-baseline.mjs`
- Create: `scripts/coverage-baseline.test.mjs`
- Create: `docs/quality/ui-coverage-baseline.json` during baseline capture.

**Interfaces:**
- Produces: `pnpm --dir ui test:coverage`.
- Produces: `readCoverageSummary(path): CoverageMetrics`.
- Produces: `compareCoverage(current, baseline, tolerance): string[]`.

- [ ] **Step 1: Write coverage comparison tests**

Create `scripts/coverage-baseline.test.mjs`:

```js
import assert from 'node:assert/strict'
import test from 'node:test'

import { compareCoverage, normalizeVitestSummary } from './coverage-baseline.mjs'

const summary = (lines, branches, functions, statements) => ({
  total: {
    lines: { pct: lines },
    branches: { pct: branches },
    functions: { pct: functions },
    statements: { pct: statements },
  },
})

test('normalizes Vitest total percentages', () => {
  assert.deepEqual(normalizeVitestSummary(summary(80, 70, 75, 81)), {
    lines: 80,
    branches: 70,
    functions: 75,
    statements: 81,
  })
})

test('reports only metrics that exceed the allowed regression', () => {
  assert.deepEqual(
    compareCoverage(
      { lines: 78, branches: 69.5, functions: 75, statements: 80.5 },
      { lines: 80, branches: 70, functions: 75, statements: 81 },
      1,
    ),
    ['lines dropped from 80 to 78'],
  )
})
```

- [ ] **Step 2: Run the test and verify it fails**

```bash
node --test scripts/coverage-baseline.test.mjs
```

Expected: FAIL with `ERR_MODULE_NOT_FOUND`.

- [ ] **Step 3: Implement the baseline tool**

Create `scripts/coverage-baseline.mjs`:

```js
#!/usr/bin/env node

import { readFileSync, writeFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

const METRICS = ['lines', 'branches', 'functions', 'statements']
export const CRITICAL_UI_FILES = [
  'src/state/reducers/projectReducer.ts',
  'src/state/reducers/workspaceReducer.ts',
  'src/state/reducers/searchReducer.ts',
  'src/state/reducers/operationReducer.ts',
  'src/state/controllers/useProjectSessionController.ts',
  'src/state/controllers/useOperationController.ts',
]

export function normalizeVitestSummary(summary) {
  return Object.fromEntries(METRICS.map((metric) => [metric, summary.total[metric].pct]))
}

export function normalizeVitestReport(summary) {
  const critical = Object.fromEntries(CRITICAL_UI_FILES.map((path) => {
    const entry = Object.entries(summary).find(
      ([candidate]) => candidate === path || candidate.endsWith(`/${path}`),
    )
    if (!entry) throw new Error(`coverage report is missing critical file ${path}`)
    return [
      path,
      Object.fromEntries(METRICS.map((metric) => [metric, entry[1][metric].pct])),
    ]
  }))
  return { global: normalizeVitestSummary(summary), critical }
}

export function compareCoverage(current, baseline, tolerance = 1) {
  return METRICS.flatMap((metric) =>
    current[metric] + tolerance < baseline[metric]
      ? [`${metric} dropped from ${baseline[metric]} to ${current[metric]}`]
      : [],
  )
}

export function compareCoverageReport(current, baseline, tolerance = 1) {
  const failures = compareCoverage(current.global, baseline.global, tolerance)
  for (const path of CRITICAL_UI_FILES) {
    failures.push(
      ...compareCoverage(current.critical[path], baseline.critical[path], tolerance)
        .map((failure) => `${path}: ${failure}`),
    )
  }
  return failures
}

export function main(argv = process.argv.slice(2)) {
  const [mode, summaryPath, baselinePath] = argv
  const current = normalizeVitestReport(JSON.parse(readFileSync(summaryPath, 'utf8')))
  if (mode === '--update') {
    writeFileSync(baselinePath, `${JSON.stringify(current, null, 2)}\n`)
    return 0
  }
  if (mode !== '--check') throw new Error('usage: coverage-baseline.mjs --update|--check summary baseline')
  const failures = compareCoverageReport(
    current,
    JSON.parse(readFileSync(baselinePath, 'utf8')),
  )
  if (failures.length > 0) {
    process.stderr.write(`${failures.join('\n')}\n`)
    return 1
  }
  return 0
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) process.exitCode = main()
```

- [ ] **Step 4: Add Vitest coverage**

Run:

```bash
pnpm --dir ui add -D --save-exact @vitest/coverage-v8
```

Add to `ui/vite.config.ts`:

```ts
coverage: {
  provider: 'v8',
  reportsDirectory: '../target/coverage/ui',
  reporter: ['text', 'json-summary'],
  include: ['src/**/*.{ts,tsx}'],
  exclude: ['src/**/*.test.{ts,tsx}', 'src/setupTests.ts', 'src/main.tsx'],
},
```

Add UI script:

```json
"test:coverage": "vitest run --coverage"
```

Add the exact resolved `@vitest/coverage-v8` package to direct dependency inventory and `THIRD_PARTY_NOTICES.md`.

- [ ] **Step 5: Capture and check the initial baseline**

```bash
pnpm --dir ui test:coverage
node scripts/coverage-baseline.mjs --update target/coverage/ui/coverage-summary.json docs/quality/ui-coverage-baseline.json
node scripts/coverage-baseline.mjs --check target/coverage/ui/coverage-summary.json docs/quality/ui-coverage-baseline.json
```

Expected: coverage succeeds; update writes global metrics plus all six critical UI file baselines; check exits 0.

- [ ] **Step 6: Add root report commands**

Add:

```json
{
  "scripts": {
    "coverage:ui": "pnpm --dir ui test:coverage && node scripts/coverage-baseline.mjs --check target/coverage/ui/coverage-summary.json docs/quality/ui-coverage-baseline.json"
  }
}
```

- [ ] **Step 7: Run tests and commit**

```bash
node --test scripts/coverage-baseline.test.mjs scripts/repository-policy.test.mjs
pnpm coverage:ui
```

Expected: PASS.

```bash
git add ui/package.json ui/vite.config.ts package.json pnpm-lock.yaml scripts/coverage-baseline.mjs scripts/coverage-baseline.test.mjs scripts/repository-policy.test.mjs THIRD_PARTY_NOTICES.md docs/quality/ui-coverage-baseline.json
git commit -m "test(ui): track coverage baseline"
```

### Task 2: Add Rust Coverage Baseline

**Files:**
- Create: `scripts/run-rust-coverage.sh`
- Create: `scripts/rust-coverage-baseline.mjs`
- Create: `scripts/rust-coverage-baseline.test.mjs`
- Create: `docs/quality/rust-coverage-baseline.json` during capture.
- Modify: `package.json`
- Modify: `README.md`

**Interfaces:**
- Produces: `target/coverage/rust.json`.
- Produces: `normalizeLlvmCoverage(report): { lines: number; functions: number; regions: number }`.

- [ ] **Step 1: Write LLVM summary normalization tests**

Create `scripts/rust-coverage-baseline.test.mjs`:

```js
import assert from 'node:assert/strict'
import test from 'node:test'

import {
  compareRustCoverage,
  normalizeLlvmCoverage,
} from './rust-coverage-baseline.mjs'

test('normalizes cargo llvm-cov totals', () => {
  const report = {
    data: [{
      totals: {
        lines: { percent: 81.25 },
        functions: { percent: 76.5 },
        regions: { percent: 79.75 },
      },
    }],
  }
  assert.deepEqual(normalizeLlvmCoverage(report), {
    lines: 81.25,
    functions: 76.5,
    regions: 79.75,
  })
})
```

- [ ] **Step 2: Run the test and verify it fails**

```bash
node --test scripts/rust-coverage-baseline.test.mjs
```

Expected: FAIL with `ERR_MODULE_NOT_FOUND`.

- [ ] **Step 3: Implement Rust report generation**

Create executable `scripts/run-rust-coverage.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
mkdir -p target/coverage
cargo llvm-cov --locked --workspace --all-targets --json --output-path target/coverage/rust.json
```

Create `scripts/rust-coverage-baseline.mjs`:

```js
#!/usr/bin/env node

import { readFileSync, writeFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

const METRICS = ['lines', 'functions', 'regions']
export const CRITICAL_RUST_GROUPS = {
  'infrastructure-operation': ['crates/viewer-infrastructure/src/operation/'],
  'desktop-security-boundaries': [
    'src-tauri/src/commands/',
    'src-tauri/src/error.rs',
    'src-tauri/src/image_protocol.rs',
    'src-tauri/src/markdown.rs',
  ],
  'application-project-lifecycle': [
    'crates/viewer-application/src/project.rs',
    'crates/viewer-application/src/session.rs',
    'crates/viewer-application/src/operation_commit.rs',
  ],
}

export function normalizeLlvmCoverage(report) {
  const totals = report.data[0].totals
  return {
    lines: totals.lines.percent,
    functions: totals.functions.percent,
    regions: totals.regions.percent,
  }
}

function aggregateFiles(files) {
  return Object.fromEntries(METRICS.map((metric) => {
    const totals = files.reduce(
      (sum, file) => ({
        count: sum.count + file.summary[metric].count,
        covered: sum.covered + file.summary[metric].covered,
      }),
      { count: 0, covered: 0 },
    )
    if (totals.count === 0) throw new Error(`critical Rust group has no ${metric} data`)
    return [metric, (totals.covered / totals.count) * 100]
  }))
}

export function normalizeLlvmReport(report) {
  const files = report.data[0].files
  const critical = Object.fromEntries(
    Object.entries(CRITICAL_RUST_GROUPS).map(([group, prefixes]) => {
      const selected = files.filter((file) => {
        const path = file.filename.replaceAll('\\', '/')
        return prefixes.some((prefix) => path.endsWith(prefix) || path.includes(`/${prefix}`))
      })
      if (selected.length === 0) throw new Error(`coverage report is missing ${group}`)
      return [group, aggregateFiles(selected)]
    }),
  )
  return { global: normalizeLlvmCoverage(report), critical }
}

export function compareRustCoverage(current, baseline, tolerance = 1) {
  return METRICS.flatMap((metric) =>
    current[metric] + tolerance < baseline[metric]
      ? [`${metric} dropped from ${baseline[metric]} to ${current[metric]}`]
      : [],
  )
}

export function compareRustCoverageReport(current, baseline, tolerance = 1) {
  const failures = compareRustCoverage(current.global, baseline.global, tolerance)
  for (const group of Object.keys(CRITICAL_RUST_GROUPS)) {
    failures.push(
      ...compareRustCoverage(current.critical[group], baseline.critical[group], tolerance)
        .map((failure) => `${group}: ${failure}`),
    )
  }
  return failures
}

export function main(argv = process.argv.slice(2)) {
  const [mode, reportPath, baselinePath] = argv
  if (!reportPath || !baselinePath) {
    throw new Error('usage: rust-coverage-baseline.mjs --update|--check report baseline')
  }
  const current = normalizeLlvmReport(JSON.parse(readFileSync(reportPath, 'utf8')))
  if (mode === '--update') {
    writeFileSync(baselinePath, `${JSON.stringify(current, null, 2)}\n`)
    return 0
  }
  if (mode !== '--check') {
    throw new Error('usage: rust-coverage-baseline.mjs --update|--check report baseline')
  }
  const failures = compareRustCoverageReport(
    current,
    JSON.parse(readFileSync(baselinePath, 'utf8')),
  )
  if (failures.length > 0) {
    process.stderr.write(`${failures.join('\n')}\n`)
    return 1
  }
  return 0
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) process.exitCode = main()
```

Import `compareRustCoverage` and append:

```js
test('reports Rust metrics that regress beyond one percentage point', () => {
  assert.deepEqual(
    compareRustCoverage(
      { lines: 79, functions: 75.5, regions: 74 },
      { lines: 81, functions: 76, regions: 75 },
      1,
    ),
    ['lines dropped from 81 to 79'],
  )
})
```

- [ ] **Step 4: Document and install the developer tool**

Document in README:

```bash
cargo install cargo-llvm-cov --version 0.8.7 --locked
```

Do not add cargo-llvm-cov to the product workspace dependency graph.

- [ ] **Step 5: Capture the baseline**

```bash
./scripts/run-rust-coverage.sh
node scripts/rust-coverage-baseline.mjs --update target/coverage/rust.json docs/quality/rust-coverage-baseline.json
node scripts/rust-coverage-baseline.mjs --check target/coverage/rust.json docs/quality/rust-coverage-baseline.json
```

Expected: global metrics plus all three critical Rust groups are written and immediately pass.

- [ ] **Step 6: Add root command and commit**

Add:

```json
"coverage:rust": "./scripts/run-rust-coverage.sh && node scripts/rust-coverage-baseline.mjs --check target/coverage/rust.json docs/quality/rust-coverage-baseline.json"
```

Run:

```bash
node --test scripts/rust-coverage-baseline.test.mjs
pnpm coverage:rust
```

Expected: PASS.

```bash
git add scripts/run-rust-coverage.sh scripts/rust-coverage-baseline.mjs scripts/rust-coverage-baseline.test.mjs docs/quality/rust-coverage-baseline.json package.json README.md
git commit -m "test(rust): track coverage baseline"
```

### Task 3: Add Architecture Health Trends

**Files:**
- Create: `scripts/architecture-health.mjs`
- Create: `scripts/architecture-health.test.mjs`
- Create: `docs/quality/architecture-health-baseline.json`
- Modify: `package.json`

**Interfaces:**
- Produces: `collectArchitectureHealth(root): ArchitectureHealth`.
- Reports production/test line counts, test-to-production ratio, files above 1,000 lines, functions above 200 lines, decision-point scores above 15, and workspace dependency edges.

- [ ] **Step 1: Write deterministic metric tests**

Create `scripts/architecture-health.test.mjs` with an in-memory file map:

```js
import assert from 'node:assert/strict'
import test from 'node:test'

import { measureFunctions, measureSourceFiles } from './architecture-health.mjs'

test('reports large production files without counting test files', () => {
  const files = new Map([
    ['src/a.ts', `${'const x = 1\n'.repeat(1001)}`],
    ['src/a.test.ts', `${'test("x", () => {})\n'.repeat(1200)}`],
    ['src/b.rs', 'pub fn small() {}\n'],
  ])
  const result = measureSourceFiles(files)
  assert.deepEqual(result.filesOver1000Lines, [{ path: 'src/a.ts', lines: 1001 }])
  assert.equal(result.productionLines, 1002)
  assert.equal(result.testLines, 1200)
  assert.equal(result.testToProductionRatio, 1200 / 1002)
})

test('reports long functions and decision-point complexity', () => {
  const body = `${'  if (value) value += 1\n'.repeat(16)}${'  value += 1\n'.repeat(184)}`
  const result = measureFunctions(new Map([
    ['src/large.ts', `function large(value: number) {\n${body}}\n`],
  ]))
  assert.equal(result.functionsOver200Lines[0].name, 'large')
  assert.equal(result.functionsOverDecisionScore15[0].decisionScore, 17)
})
```

- [ ] **Step 2: Implement metric collection**

Implement:

```js
export function measureSourceFiles(files) {
  const measured = [...files]
    .map(([path, source]) => ({
      path,
      lines: source.split(/\r?\n/).length - 1,
      test: /\.test\.(ts|tsx)$/.test(path) || path.includes('/tests/'),
    }))
  const production = measured.filter((file) => !file.test)
  const testLines = measured
    .filter((file) => file.test)
    .reduce((sum, file) => sum + file.lines, 0)
  const productionLines = production.reduce((sum, file) => sum + file.lines, 0)
  return {
    productionLines,
    testLines,
    testToProductionRatio: productionLines === 0 ? 0 : testLines / productionLines,
    filesOver1000Lines: production
      .filter((file) => file.lines > 1000)
      .map(({ path, lines }) => ({ path, lines })),
  }
}
```

Export `measureFunctions(files)`. Recognize line-anchored Rust `fn`, TypeScript `function`, and `const useX = (` declarations; measure each declaration to the next recognized declaration in the same file. Define `decisionScore` as one plus the count of `if`, `for`, `while`, `match`, `case`, `&&`, `||`, and ternary `?` tokens in that span. Return sorted arrays `functionsOver200Lines` and `functionsOverDecisionScore15` with `{ path, name, startLine, lines, decisionScore }`.

The CLI reads Rust from `crates/*/src` and `src-tauri/src`, TypeScript from `ui/src`, and obtains workspace dependency edges from:

```bash
cargo metadata --locked --no-deps --format-version 1
```

Store only relative paths and Viewer workspace edges. Classify test files by `.test.ts(x)`, any `/tests/` directory, and Rust `#[cfg(test)]` modules; keep generated `target/` content out of the input map.

- [ ] **Step 3: Capture baseline and add report command**

```bash
node scripts/architecture-health.mjs --update docs/quality/architecture-health-baseline.json
node scripts/architecture-health.mjs --check docs/quality/architecture-health-baseline.json
```

The check prints non-blocking warnings when a new file crosses 1,000 lines, a function crosses 200 lines, a function crosses decision score 15, or the test-to-production ratio drops by more than `0.02`. It exits nonzero only when a workspace edge violates Domain → none, Application → Domain, Infrastructure/Platform → Application+Domain, Desktop → all.

Add:

```json
"architecture:health": "node scripts/architecture-health.mjs --check docs/quality/architecture-health-baseline.json"
```

- [ ] **Step 4: Run tests and commit**

```bash
node --test scripts/architecture-health.test.mjs
pnpm architecture:health
```

Expected: PASS.

```bash
git add scripts/architecture-health.mjs scripts/architecture-health.test.mjs docs/quality/architecture-health-baseline.json package.json
git commit -m "build: track architecture health trends"
```

### Task 4: Govern Dependency Exceptions and Warnings

**Files:**
- Create: `docs/quality/dependency-exceptions.json`
- Create: `docs/quality/DEPENDENCY_HEALTH.md`
- Create: `scripts/check-dependency-exceptions.mjs`
- Create: `scripts/check-dependency-exceptions.test.mjs`
- Modify: `deny.toml`
- Modify: `package.json`

**Interfaces:**
- Produces: exception register schema with `id`, `reason`, `chain`, `owner`, `created`, `reviewAfter`, `removeWhen`.
- Fails when a review date is in the past or a deny exception is unregistered.

- [ ] **Step 1: Write expiry tests**

Create:

```js
import assert from 'node:assert/strict'
import test from 'node:test'

import { validateExceptions } from './check-dependency-exceptions.mjs'

test('rejects expired and ownerless exceptions', () => {
  assert.throws(
    () => validateExceptions([{
      id: 'RUSTSEC-1',
      reason: 'transitive',
      chain: 'tauri > unic',
      owner: '',
      created: '2026-07-23',
      reviewAfter: '2026-07-22',
      removeWhen: 'upstream removes dependency',
    }], new Date('2026-07-23T00:00:00Z')),
    /owner|expired/,
  )
})
```

- [ ] **Step 2: Implement validation**

`validateExceptions` must reject missing fields, duplicate IDs, invalid dates, and `reviewAfter < today`.

The CLI reads `deny.toml` text and verifies every `RUSTSEC-*` ID in `ignore` exists in the JSON register, and every registered ID exists in `deny.toml`.

- [ ] **Step 3: Register current exceptions**

Create five rows for:

```json
[
  {
    "id": "RUSTSEC-2025-0075",
    "reason": "Tauri transitive dependency with no safe upgrade; remove when Tauri removes the unic-* chain",
    "chain": "viewer-desktop 0.1.0 > tauri 2.11.5/tauri-build 2.6.3/tauri-macros 2.6.3 > tauri-utils 2.9.3 > urlpattern 0.3.0 > unic-ucd-ident 0.9.0 > unic-char-property 0.9.0 > unic-char-range 0.9.0",
    "owner": "Viewer maintainers",
    "created": "2026-07-23",
    "reviewAfter": "2026-10-23",
    "removeWhen": "Tauri removes the transitive unic-* chain"
  },
  {
    "id": "RUSTSEC-2025-0080",
    "reason": "Tauri transitive dependency with no safe upgrade; remove when Tauri removes the unic-* chain",
    "chain": "viewer-desktop 0.1.0 > tauri 2.11.5/tauri-build 2.6.3/tauri-macros 2.6.3 > tauri-utils 2.9.3 > urlpattern 0.3.0 > unic-ucd-ident 0.9.0 > unic-ucd-version 0.9.0 > unic-common 0.9.0",
    "owner": "Viewer maintainers",
    "created": "2026-07-23",
    "reviewAfter": "2026-10-23",
    "removeWhen": "Tauri removes the transitive unic-* chain"
  },
  {
    "id": "RUSTSEC-2025-0081",
    "reason": "Tauri transitive dependency with no safe upgrade; remove when Tauri removes the unic-* chain",
    "chain": "viewer-desktop 0.1.0 > tauri 2.11.5/tauri-build 2.6.3/tauri-macros 2.6.3 > tauri-utils 2.9.3 > urlpattern 0.3.0 > unic-ucd-ident 0.9.0 > unic-char-property 0.9.0",
    "owner": "Viewer maintainers",
    "created": "2026-07-23",
    "reviewAfter": "2026-10-23",
    "removeWhen": "Tauri removes the transitive unic-* chain"
  },
  {
    "id": "RUSTSEC-2025-0098",
    "reason": "Tauri transitive dependency with no safe upgrade; remove when Tauri removes the unic-* chain",
    "chain": "viewer-desktop 0.1.0 > tauri 2.11.5/tauri-build 2.6.3/tauri-macros 2.6.3 > tauri-utils 2.9.3 > urlpattern 0.3.0 > unic-ucd-ident 0.9.0 > unic-ucd-version 0.9.0",
    "owner": "Viewer maintainers",
    "created": "2026-07-23",
    "reviewAfter": "2026-10-23",
    "removeWhen": "Tauri removes the transitive unic-* chain"
  },
  {
    "id": "RUSTSEC-2025-0100",
    "reason": "Tauri transitive dependency with no safe upgrade; remove when Tauri removes the unic-* chain",
    "chain": "viewer-desktop 0.1.0 > tauri 2.11.5/tauri-build 2.6.3/tauri-macros 2.6.3 > tauri-utils 2.9.3 > urlpattern 0.3.0 > unic-ucd-ident 0.9.0",
    "owner": "Viewer maintainers",
    "created": "2026-07-23",
    "reviewAfter": "2026-10-23",
    "removeWhen": "Tauri removes the transitive unic-* chain"
  }
]
```

Remove `"ISC"` from the global license allow-list because cargo-deny reports it as not encountered. Do not change the foldhash Zlib exception.

- [ ] **Step 4: Document duplicate-version classification**

Record all current duplicate warnings in `DEPENDENCY_HEALTH.md` under:

- Tauri/platform transitive;
- build-only;
- potentially direct-upgrade-removable.

Do not add patches or version overrides in this task.

- [ ] **Step 5: Add command, test and commit**

Add:

```json
"dependencies:exceptions": "node scripts/check-dependency-exceptions.mjs"
```

Run:

```bash
node --test scripts/check-dependency-exceptions.test.mjs
pnpm dependencies:exceptions
cargo deny --offline --locked check
```

Expected: PASS; cargo-deny no longer emits `license-not-encountered` for ISC.

```bash
git add docs/quality/dependency-exceptions.json docs/quality/DEPENDENCY_HEALTH.md scripts/check-dependency-exceptions.mjs scripts/check-dependency-exceptions.test.mjs deny.toml package.json
git commit -m "build: govern dependency exceptions"
```

### Task 5: Add Scheduled Network Audit

**Files:**
- Create: `.github/workflows/dependency-audit.yml`
- Modify: `scripts/repository-policy.test.mjs`

**Interfaces:**
- Produces: scheduled and manual network audit.
- Does not become a pull-request required gate.

- [ ] **Step 1: Add workflow policy test**

Append:

```js
test('network dependency audit is scheduled and never a pull-request gate', async () => {
  const workflow = await read('.github/workflows/dependency-audit.yml')
  assert.match(workflow, /schedule:/)
  assert.match(workflow, /workflow_dispatch:/)
  assert.doesNotMatch(workflow, /pull_request:/)
  assert.match(workflow, /pnpm audit --audit-level high/)
  assert.match(workflow, /cargo deny --locked check advisories/)
})
```

- [ ] **Step 2: Run and verify failure**

```bash
node --test scripts/repository-policy.test.mjs
```

Expected: FAIL with `ENOENT`.

- [ ] **Step 3: Create the workflow**

```yaml
name: Dependency audit

on:
  schedule:
    - cron: "23 10 * * 1"
  workflow_dispatch:

permissions:
  contents: read

jobs:
  audit:
    runs-on: macos-15
    timeout-minutes: 30
    steps:
      - name: Check out repository
        uses: actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0 # v7
      - name: Assert Apple Silicon runner
        shell: bash
        run: test "$(uname -m)" = "arm64"
      - name: Set up Node.js 24.18.0
        uses: actions/setup-node@820762786026740c76f36085b0efc47a31fe5020 # v7
        with:
          node-version: "24.18.0"
      - name: Activate pnpm 10.0.0 through Corepack
        shell: bash
        run: |
          corepack enable
          corepack prepare pnpm@10.0.0 --activate
          pnpm --version
      - name: Install Rust 1.97.0 for Apple Silicon
        shell: bash
        run: |
          rustup toolchain install 1.97.0 --profile minimal --target aarch64-apple-darwin
          rustc --version --verbose
      - name: Install JavaScript dependencies
        run: pnpm install --frozen-lockfile
      - name: Install cargo-deny 0.20.2
        run: cargo install cargo-deny --version 0.20.2 --locked
      - name: Audit npm advisories
        run: pnpm audit --audit-level high
      - name: Audit Rust advisories
        run: cargo deny --locked check advisories
      - name: Check exception review dates
        run: pnpm dependencies:exceptions
```

- [ ] **Step 4: Run policy and commit**

```bash
node --test scripts/repository-policy.test.mjs
```

Expected: PASS.

```bash
git add .github/workflows/dependency-audit.yml scripts/repository-policy.test.mjs
git commit -m "ci: schedule network dependency audits"
```

### Task 6: Add Documentation Status Index

**Files:**
- Create: `docs/README.md`
- Modify: `docs/PRODUCT_SPEC.md`
- Modify: `docs/TECHNICAL_FOUNDATIONS.md`
- Modify: `docs/adr/0001-macos-image-pipeline.md`
- Modify: `docs/adr/0002-file-transaction-protocol.md`
- Modify: `docs/adr/0003-scan-search-and-generation.md`
- Modify: `docs/adr/0004-viewer-0.1-architecture-freeze.md`
- Modify: `docs/adr/0005-continuous-development-governance.md`
- Modify: `docs/architecture/viewer-0.1-api-baseline.md`
- Modify: `docs/milestones/viewer-0.1-scope-matrix.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md`
- Modify: `docs/superpowers/specs/2026-07-23-viewer-engineering-optimization-governance-design.md`
- Modify: `scripts/repository-policy.test.mjs`

**Interfaces:**
- Produces: one documentation entry point.
- Allowed statuses: `Active`, `Superseded`, `Historical`.

- [ ] **Step 1: Write status policy test**

Append:

```js
test('documentation index declares one status for every listed source', async () => {
  const index = await read('docs/README.md')
  assert.match(index, /\| Document \| Status \| Replaced by \|/)
  assert.match(index, /0005-continuous-development-governance\.md.*Active/)
  assert.match(index, /0004-viewer-0\.1-architecture-freeze\.md.*Superseded/)
  assert.doesNotMatch(index, /\bTBD\b|\bTODO\b/)
})
```

- [ ] **Step 2: Create the index**

Use sections:

```markdown
# Viewer Documentation Index

## Active sources of truth
## Superseded decisions
## Historical implementation plans
## Historical review evidence
## Status rules
```

The table schema is:

```markdown
| Document | Status | Replaced by |
| --- | --- | --- |
| `adr/0005-continuous-development-governance.md` | Active | — |
| `adr/0004-viewer-0.1-architecture-freeze.md` | Superseded | `adr/0005-continuous-development-governance.md` |
```

List every ADR, current product/technical source, approved active design, historical plan, and review directory entry.

- [ ] **Step 3: Add status metadata to active documents**

Use exactly one of:

```markdown
> Status: Active
> Status: Superseded by `relative/path.md`
> Status: Historical
```

Do not modify evidence text while adding status.

- [ ] **Step 4: Run policy and commit**

```bash
node --test scripts/repository-policy.test.mjs
```

Expected: PASS.

```bash
git add docs/README.md docs/PRODUCT_SPEC.md docs/TECHNICAL_FOUNDATIONS.md docs/adr/0001-macos-image-pipeline.md docs/adr/0002-file-transaction-protocol.md docs/adr/0003-scan-search-and-generation.md docs/adr/0004-viewer-0.1-architecture-freeze.md docs/adr/0005-continuous-development-governance.md docs/architecture/viewer-0.1-api-baseline.md docs/milestones/viewer-0.1-scope-matrix.md docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md docs/superpowers/specs/2026-07-23-viewer-engineering-optimization-governance-design.md scripts/repository-policy.test.mjs
git commit -m "docs: index active and historical sources"
```

### Task 7: Compose Quality Reports Without a Release Gate

**Files:**
- Modify: `package.json`
- Modify: `README.md`
- Modify: `scripts/repository-policy.test.mjs`

**Interfaces:**
- Produces: `pnpm quality:report`.
- `pnpm verify` remains deterministic and does not require coverage tools.

- [ ] **Step 1: Add command policy**

Append:

```js
test('quality reports are continuous trends and not a release gate', async () => {
  const packageJson = JSON.parse(await read('package.json'))
  assert.equal(
    packageJson.scripts['quality:report'],
    'pnpm coverage:ui && pnpm coverage:rust && pnpm architecture:health',
  )
  assert.doesNotMatch(packageJson.scripts.verify, /coverage|quality:report|release|M4/)
})
```

- [ ] **Step 2: Add the report command**

```json
"quality:report": "pnpm coverage:ui && pnpm coverage:rust && pnpm architecture:health"
```

Document that `cargo-llvm-cov` is required only for `quality:report`, not ordinary `pnpm verify`.

- [ ] **Step 3: Run all deterministic gates and reports**

```bash
node --test scripts/repository-policy.test.mjs scripts/coverage-baseline.test.mjs scripts/rust-coverage-baseline.test.mjs scripts/architecture-health.test.mjs scripts/check-dependency-exceptions.test.mjs
pnpm quality:report
pnpm security
pnpm verify:clean
```

Expected: every command exits 0; no new worktree entries remain.

- [ ] **Step 4: Commit report composition**

```bash
git add package.json README.md scripts/repository-policy.test.mjs
git commit -m "build: compose continuous quality reports"
```

### Task 8: Run Continuous Governance Exit Gate

**Files:**
- Verify only.

- [ ] **Step 1: Confirm dependency and document governance**

```bash
pnpm dependencies:exceptions
node --test scripts/repository-policy.test.mjs
```

Expected: PASS.

- [ ] **Step 2: Confirm quality reports**

```bash
pnpm quality:report
```

Expected: UI/Rust coverage and architecture-health checks pass against checked-in baselines.

- [ ] **Step 3: Confirm ordinary development gate remains independent**

```bash
pnpm verify:clean
```

Expected: PASS without invoking cargo-llvm-cov or network audit.

- [ ] **Step 4: Confirm no centralized final acceptance stage exists**

```bash
rg -n -i 'M4 Internal Release|M4 内部发布|final acceptance|最终验收' docs .github scripts package.json
```

Expected: matches are explicit cancellation/non-reintroduction statements in active governance/optimization sources, or occur in explicitly Historical/Superseded documents; none assigns current work or acceptance ownership to M4.
