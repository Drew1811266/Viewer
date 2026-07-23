# Viewer Engineering Baseline and CI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give Viewer one documented developer entry point, strict and consistently formatted TypeScript, deterministic quality/security commands, and CI that validates behavior rather than frozen text.

**Architecture:** Add repository hygiene first, enable strict TypeScript with measured fixes, introduce one frontend lint/format tool, compose stable root commands, then migrate CI and repository policy around those commands.

**Tech Stack:** Node.js 24.18.0, pnpm 10.0.0, TypeScript 6.0, Biome, React 19, Vitest 4, Rust 1.97.0, cargo-deny 0.20.2, GitHub Actions macos-15.

## Global Constraints

- Consume ADR 0005 and the stage model from the governance plan.
- Do not change product behavior, IPC contracts, runtime permissions, or supported file types.
- Use exactly one TypeScript lint/format tool; this plan selects Biome.
- Add Biome with `pnpm --dir ui add -D --save-exact @biomejs/biome` so the lockfile records the resolved version.
- `strict: true` must be enabled immediately because the current source passes that probe.
- `noUncheckedIndexedAccess` must be enabled only after the 16 measured array/index errors are fixed.
- `verify:clean` compares before/after porcelain snapshots and must tolerate unrelated pre-existing user changes.
- Never globally ignore `.viewer`.
- Update direct dependency inventory and `THIRD_PARTY_NOTICES.md` for Biome.

---

### Task 1: Add Onboarding and Repository Hygiene

**Files:**
- Create: `README.md`
- Create: `CONTRIBUTING.md`
- Create: `.editorconfig`
- Modify: `.gitignore`
- Modify: `scripts/repository-policy.test.mjs`

**Interfaces:**
- Produces: documented entry command `pnpm verify:clean`.
- Produces: repository conventions consumed by every later plan.

- [ ] **Step 1: Write failing repository hygiene tests**

Append to `scripts/repository-policy.test.mjs`:

```js
test('repository exposes one onboarding path and recursive macOS hygiene', async () => {
  const [readme, contributing, editorConfig, ignore] = await Promise.all([
    read('README.md'),
    read('CONTRIBUTING.md'),
    read('.editorconfig'),
    read('.gitignore'),
  ])
  assert.match(readme, /pnpm verify:clean/)
  assert.match(readme, /Viewer 0\.1.*development-stage baseline/i)
  assert.match(contributing, /Do not stage unrelated user changes/i)
  assert.match(editorConfig, /root = true/)
  assert.match(editorConfig, /charset = utf-8/)
  assert.match(ignore, /(?:^|\n)\.DS_Store(?:\n|$)/)
  assert.doesNotMatch(ignore, /(?:^|\n)(?:\*\*\/)?\.viewer\/?(?:\n|$)/)
})
```

- [ ] **Step 2: Run the policy test and verify it fails**

Run:

```bash
node --test scripts/repository-policy.test.mjs
```

Expected: FAIL with `ENOENT` for `README.md`.

- [ ] **Step 3: Add exact editor and ignore rules**

Create `.editorconfig`:

```ini
root = true

[*]
charset = utf-8
end_of_line = lf
insert_final_newline = true
trim_trailing_whitespace = true

[*.{json,jsonc,md,yml,yaml}]
indent_style = space
indent_size = 2

[*.{rs,toml}]
indent_style = space
indent_size = 4

[Makefile]
indent_style = tab
```

Replace `/.DS_Store` in `.gitignore` with:

```gitignore
.DS_Store
```

Do not add any `.viewer` ignore rule.

- [ ] **Step 4: Write README and CONTRIBUTING**

`README.md` must use this section order:

```markdown
# Viewer

Viewer is a local-first macOS image and text review application under active development. Viewer 0.1 is a development-stage baseline, not a release or delivery target.

## Current scope
## Architecture
## Requirements
## Install
## Develop
## Verify
## Repository map
## Active documentation
## License
```

Under Verify, document:

```bash
pnpm verify:clean
```

Explain that the command snapshots the pre-existing porcelain entry set and fails if verification adds or removes an entry.

Under Requirements, state the pinned development baseline: Apple Silicon macOS 13 or newer, Xcode Command Line Tools, Node.js 24.18.0, Corepack-managed pnpm 10.0.0, and Rust 1.97.0 with `rustfmt` and `clippy`. Under Architecture and Repository map, link Domain, Application, Infrastructure, Platform/macOS, Tauri/Desktop, UI, integration tests, ADRs, the product spec, and the engineering optimization roadmap.

`CONTRIBUTING.md` must include:

```markdown
## Branches and commits

- Use a short-lived descriptive branch; Codex-created branches use the `codex/` prefix.
- Keep one reviewable concern per commit and use `type(scope): outcome` commit subjects.
- Do not combine formatter-only changes with runtime refactors.

## Verification

- Run the closest focused test before editing and after each responsibility move.
- Run `pnpm verify:clean` before requesting review.
- Treat security, recovery, path-boundary and data-consistency failures as blockers.

## Dependency changes

- Use locked/frozen package-manager commands.
- Record every new direct dependency in `THIRD_PARTY_NOTICES.md` and repository policy.
- Add or update a dated exception entry before changing `deny.toml` advisory ignores.

## Architecture records

- Add a design and implementation plan before cross-module behavior changes.
- Add or supersede an ADR when a public contract, persistence schema or security boundary changes.
- Check architecture-health output before adding responsibility to a file already above its baseline.

## Change discipline

- Do not stage unrelated user changes.
- Preserve IPC, Domain/Application and persistence contracts during refactors.
- Run focused tests before `pnpm verify:clean`.
- Update `THIRD_PARTY_NOTICES.md` and dependency policy for every direct dependency.
- Do not commit `.DS_Store`, generated caches or accidental project `.viewer` metadata.
- Never open `tests/fixtures/images` as a writable Viewer project; copy static media into a temporary project directory.
- Tests that create `.viewer` metadata must own a `tempfile::TempDir` or `mkdtemp` root and let it clean up after the test.
```

- [ ] **Step 5: Run the policy test**

```bash
node --test scripts/repository-policy.test.mjs
```

Expected: PASS for the onboarding/hygiene test; any unrelated pre-existing tests also pass.

- [ ] **Step 6: Commit onboarding and hygiene**

```bash
git add README.md CONTRIBUTING.md .editorconfig .gitignore scripts/repository-policy.test.mjs
git commit -m "docs: add developer onboarding and repository hygiene"
```

### Task 2: Enable TypeScript Strict Mode

**Files:**
- Modify: `ui/tsconfig.app.json`

**Interfaces:**
- Produces: `strict: true` for all `ui/src` TypeScript and tests.

- [ ] **Step 1: Record the current strict probe**

Run:

```bash
pnpm --dir ui exec tsc -p tsconfig.app.json --noEmit --strict
```

Expected: PASS with exit 0.

- [ ] **Step 2: Make strict mode permanent**

Add under `compilerOptions`:

```json
"strict": true,
```

Do not enable `noUncheckedIndexedAccess` in this commit.

- [ ] **Step 3: Run typecheck, tests and build**

```bash
pnpm --dir ui exec tsc -p tsconfig.app.json --noEmit
pnpm --dir ui test
pnpm --dir ui build
```

Expected: all commands exit 0; 35 current test files pass.

- [ ] **Step 4: Commit strict mode**

```bash
git add ui/tsconfig.app.json
git commit -m "build(ui): enable strict TypeScript"
```

### Task 3: Enable Checked Indexed Access

**Files:**
- Modify: `ui/tsconfig.app.json`
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/components/ImagePreview.test.tsx`
- Modify: `ui/src/components/InfoOverlay.tsx`
- Modify: `ui/src/state/compareModel.test.ts`
- Modify: `ui/src/state/useViewerController.ts`

**Interfaces:**
- Produces: `noUncheckedIndexedAccess: true`.
- Preserves: existing runtime guards; non-null assertions are allowed only after a length/cardinality proof.

- [ ] **Step 1: Enable the option and verify the measured failure**

Add:

```json
"noUncheckedIndexedAccess": true,
```

Run:

```bash
pnpm --dir ui exec tsc -p tsconfig.app.json --noEmit
```

Expected: FAIL with 16 diagnostics in exactly the five source/test areas listed above: `paths[0]`, two `files[1]` props, `updated.images[0]`, `files[0]`, and eleven keyed compare transforms.

- [ ] **Step 2: Fix guarded array access**

Apply these exact patterns:

```ts
// ContentBrowser.test.tsx: workspace(4) guarantees at least one image.
updated.images[0] = {
  ...updated.images[0]!,
  marker: { reviewState: 'keep', favorite: true },
}

// ImagePreview.test.tsx: each fixture array is declared with the indexed member.
file={files[1]!}
expect(request).toHaveBeenCalledWith(files[1]!, expect.objectContaining({
  kind: 'original100_percent',
}))

// InfoOverlay.tsx: rendering is guarded by files.length === 1.
<SingleFileInfo file={files[0]!} dimensions={dimensions} />

// useViewerController.ts: the callback returned after paths.length === 1.
void openProject(paths[0]!)
```

In `compareModel.test.ts`, use non-null assertions only for fixture-guaranteed keys:

```ts
expect(state.transforms.a!.scale).toBe(4)
expect(state.transforms.b!.scale).toBe(4)
expect(three.state.transforms.b!.scale).toBe(2)
```

Apply the same `!` pattern to all eleven measured transform assertions; do not add fallback values that could hide a missing pane.

- [ ] **Step 3: Run the strict typecheck**

```bash
pnpm --dir ui exec tsc -p tsconfig.app.json --noEmit
```

Expected: PASS with exit 0.

- [ ] **Step 4: Run focused tests**

```bash
pnpm --dir ui test -- ContentBrowser.test.tsx ImagePreview.test.tsx compareModel.test.ts
```

Expected: all selected tests pass.

- [ ] **Step 5: Commit indexed-access safety**

```bash
git add ui/tsconfig.app.json ui/src/components/ContentBrowser.test.tsx ui/src/components/ImagePreview.test.tsx ui/src/components/InfoOverlay.tsx ui/src/state/compareModel.test.ts ui/src/state/useViewerController.ts
git commit -m "build(ui): check indexed TypeScript access"
```

### Task 4: Add Biome Lint and Format

**Files:**
- Create: `ui/biome.json`
- Modify: `ui/package.json`
- Modify: `package.json`
- Modify: `pnpm-lock.yaml`
- Modify: `scripts/repository-policy.test.mjs`
- Modify: `THIRD_PARTY_NOTICES.md`
- Modify: all `ui/src/**/*.{ts,tsx}` files only as required by deterministic formatting/lint.

**Interfaces:**
- Produces: `pnpm --dir ui check`, `lint`, `format`, `format:check`, `typecheck`.
- Adds: direct dev dependency `@biomejs/biome`.

- [ ] **Step 1: Add a failing dependency/policy expectation**

Add `@biomejs/biome` to `directDependencies` in `scripts/repository-policy.test.mjs`.

Append:

```js
test('the UI has one strict lint and format tool', async () => {
  const [uiPackageText, config] = await Promise.all([
    read('ui/package.json'),
    read('ui/biome.json'),
  ])
  const uiPackage = JSON.parse(uiPackageText)
  assert.equal(uiPackage.scripts.typecheck, 'tsc -b')
  assert.equal(uiPackage.scripts.lint, 'biome lint .')
  assert.equal(uiPackage.scripts['format:check'], 'biome format .')
  assert.equal(uiPackage.scripts.check, 'biome check . && tsc -b')
  assert.equal(JSON.parse(config).formatter.indentStyle, 'space')
})
```

- [ ] **Step 2: Run the policy test and verify it fails**

```bash
node --test scripts/repository-policy.test.mjs
```

Expected: FAIL because the direct dependency notice and `ui/biome.json` do not exist.

- [ ] **Step 3: Add the dependency and configuration**

Run:

```bash
pnpm --dir ui add -D --save-exact @biomejs/biome
```

Create `ui/biome.json`:

```json
{
  "files": {
    "includes": ["src/**/*.ts", "src/**/*.tsx", "vite.config.ts"]
  },
  "formatter": {
    "enabled": true,
    "indentStyle": "space",
    "indentWidth": 2,
    "lineWidth": 100
  },
  "linter": {
    "enabled": true,
    "rules": {
      "recommended": true
    }
  },
  "javascript": {
    "formatter": {
      "quoteStyle": "single",
      "semicolons": "asNeeded",
      "trailingCommas": "all"
    }
  }
}
```

Set UI scripts:

```json
{
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "typecheck": "tsc -b",
    "lint": "biome lint .",
    "format": "biome format --write .",
    "format:check": "biome format .",
    "check": "biome check . && tsc -b",
    "test": "vitest run",
    "preview": "vite preview"
  }
}
```

Add the resolved Biome version, license, purpose and upstream URL to `THIRD_PARTY_NOTICES.md`.

- [ ] **Step 4: Apply deterministic formatting**

Run:

```bash
pnpm --dir ui exec biome check --write .
pnpm --dir ui check
```

Expected: Biome applies its deterministic safe fixes to TypeScript/TSX/config files and `check` exits 0. Resolve any remaining diagnostic at its reported source expression; do not add global `noExplicitAny`, hook-rule, or file-level suppressions.

- [ ] **Step 5: Run UI tests and build**

```bash
pnpm --dir ui test
pnpm --dir ui build
node --test scripts/repository-policy.test.mjs
```

Expected: all commands pass and dependency inventory includes Biome.

- [ ] **Step 6: Commit Biome**

```bash
git add ui/biome.json ui/package.json package.json pnpm-lock.yaml ui/src ui/vite.config.ts scripts/repository-policy.test.mjs THIRD_PARTY_NOTICES.md
git commit -m "build(ui): enforce Biome lint and format"
```

### Task 5: Add Clean Verification and Stable Root Commands

**Files:**
- Create: `scripts/verify-clean.mjs`
- Create: `scripts/verify-clean.test.mjs`
- Modify: `package.json`
- Modify: `scripts/repository-policy.test.mjs`

**Interfaces:**
- Produces: `pnpm quality`, `pnpm security`, `pnpm verify`, `pnpm verify:clean`.
- Produces: `comparePorcelain(before: Buffer, after: Buffer): string[]`.

- [ ] **Step 1: Write failing snapshot-comparison tests**

Create `scripts/verify-clean.test.mjs`:

```js
import assert from 'node:assert/strict'
import test from 'node:test'

import { comparePorcelain } from './verify-clean.mjs'

test('pre-existing user changes are preserved without failing', () => {
  const before = Buffer.from('?? user-file\0')
  const after = Buffer.from('?? user-file\0')
  assert.deepEqual(comparePorcelain(before, after), [])
})

test('new verification artifacts are reported', () => {
  const before = Buffer.from('?? user-file\0')
  const after = Buffer.from('?? user-file\0?? tests/.DS_Store\0')
  assert.deepEqual(comparePorcelain(before, after), ['added: ?? tests/.DS_Store'])
})

test('removed worktree entries are reported', () => {
  const before = Buffer.from('?? user-file\0?? generated-cache\0')
  const after = Buffer.from('?? user-file\0')
  assert.deepEqual(comparePorcelain(before, after), ['removed: ?? generated-cache'])
})
```

- [ ] **Step 2: Run the test and verify it fails**

```bash
node --test scripts/verify-clean.test.mjs
```

Expected: FAIL with `ERR_MODULE_NOT_FOUND`.

- [ ] **Step 3: Implement the clean wrapper**

Create `scripts/verify-clean.mjs`:

```js
#!/usr/bin/env node

import { execFileSync, spawnSync } from 'node:child_process'
import { pathToFileURL } from 'node:url'

const entries = (buffer) =>
  buffer
    .toString('utf8')
    .split('\0')
    .filter(Boolean)
    .sort()

export function comparePorcelain(before, after) {
  const baseline = new Set(entries(before))
  const current = new Set(entries(after))
  return [
    ...[...baseline].filter((entry) => !current.has(entry)).map((entry) => `removed: ${entry}`),
    ...[...current].filter((entry) => !baseline.has(entry)).map((entry) => `added: ${entry}`),
  ].sort()
}

function porcelain() {
  return execFileSync('git', ['status', '--porcelain=v1', '-z'], { encoding: 'buffer' })
}

export function main() {
  const before = porcelain()
  const result = spawnSync('pnpm', ['verify'], { stdio: 'inherit' })
  const changes = comparePorcelain(before, porcelain())
  if (changes.length > 0) {
    process.stderr.write(`verification changed worktree entries:\n${changes.join('\n')}\n`)
    return 1
  }
  return result.status ?? 1
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) process.exitCode = main()
```

Set root scripts:

```json
{
  "scripts": {
    "quality": "node --test scripts/repository-policy.test.mjs scripts/scope-coverage.test.mjs scripts/verify-clean.test.mjs && pnpm --dir ui check && pnpm --dir ui test && pnpm --dir ui build && cargo fmt --check && cargo clippy --locked --workspace --all-targets -- -D warnings && cargo test --locked --workspace",
    "security": "./scripts/check-tauri-security.sh && cargo deny --offline --locked check && node scripts/check-npm-licenses.mjs",
    "verify": "pnpm quality && pnpm security",
    "verify:clean": "node scripts/verify-clean.mjs"
  }
}
```

- [ ] **Step 4: Replace exact verify-string policy with behavior checks**

Delete `expectedVerify`. Assert individual scripts:

```js
assert.match(packageJson.scripts.quality, /pnpm --dir ui check/)
assert.match(packageJson.scripts.quality, /cargo clippy --locked --workspace --all-targets/)
assert.match(packageJson.scripts.security, /check-tauri-security\.sh/)
assert.match(packageJson.scripts.security, /cargo deny --offline --locked check/)
assert.equal(packageJson.scripts.verify, 'pnpm quality && pnpm security')
assert.equal(packageJson.scripts['verify:clean'], 'node scripts/verify-clean.mjs')
```

- [ ] **Step 5: Run focused wrapper and policy tests**

```bash
node --test scripts/verify-clean.test.mjs scripts/repository-policy.test.mjs
```

Expected: PASS.

- [ ] **Step 6: Run clean verification**

```bash
pnpm verify:clean
```

Expected: exit 0 and no new worktree entry relative to the pre-command snapshot.

- [ ] **Step 7: Commit stable commands**

```bash
git add scripts/verify-clean.mjs scripts/verify-clean.test.mjs scripts/repository-policy.test.mjs package.json
git commit -m "build: compose deterministic quality and security gates"
```

### Task 6: Replace Frozen Workflow Text with Deterministic CI Jobs

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `scripts/repository-policy.test.mjs`

**Interfaces:**
- Consumes: `pnpm quality` and `pnpm security`.
- Produces: independent `quality` and `security` GitHub jobs.

- [ ] **Step 1: Replace exact workflow comparison with invariant tests**

Delete `expectedWorkflow`. Add:

```js
assert.match(workflow, /^name: CI/m)
assert.match(workflow, /^\s*push:\s*$/m)
assert.match(workflow, /^\s*pull_request:\s*$/m)
assert.match(workflow, /permissions:\n\s+contents: read/)
assert.match(workflow, /runs-on: macos-15/g)
assert.match(workflow, /pnpm install --frozen-lockfile/g)
assert.match(workflow, /run: pnpm quality/)
assert.match(workflow, /run: pnpm security/)
assert.doesNotMatch(workflow, /run: pnpm audit/)
for (const action of workflow.matchAll(/uses:\s+([^@\s]+)@([^\s#]+)/g)) {
  assert.match(action[2], /^[0-9a-f]{40}$/, `${action[1]} must be pinned to a commit`)
}
```

- [ ] **Step 2: Rewrite CI around the root interfaces**

Define two jobs:

```yaml
name: CI

on:
  push:
  pull_request:

permissions:
  contents: read

jobs:
  quality:
    name: Deterministic quality
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
          rustup toolchain install 1.97.0 --profile minimal --component clippy,rustfmt --target aarch64-apple-darwin
          rustc --version --verbose
      - name: Install JavaScript dependencies
        run: pnpm install --frozen-lockfile
      - name: Run quality gate
        run: pnpm quality
      - name: Assert verification left no artifacts
        run: test -z "$(git status --porcelain)"

  security:
    name: Deterministic security
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
          rustup toolchain install 1.97.0 --profile minimal --component clippy,rustfmt --target aarch64-apple-darwin
          rustc --version --verbose
      - name: Install JavaScript dependencies
        run: pnpm install --frozen-lockfile
      - name: Install cargo-deny 0.20.2
        run: cargo install cargo-deny --version 0.20.2 --locked
      - name: Run security gate
        run: pnpm security
      - name: Assert verification left no artifacts
        run: test -z "$(git status --porcelain)"
```

- [ ] **Step 3: Run policy and local gates**

```bash
node --test scripts/repository-policy.test.mjs
pnpm quality
pnpm security
```

Expected: every command passes.

- [ ] **Step 4: Commit CI migration**

```bash
git add .github/workflows/ci.yml scripts/repository-policy.test.mjs
git commit -m "ci: separate deterministic quality and security gates"
```

### Task 7: Run the Engineering Baseline Exit Gate

**Files:**
- Verify only.

**Interfaces:**
- Confirms: all Plan 2 commands are stable for Plans 3–6.

- [ ] **Step 1: Run strict UI gates**

```bash
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
```

Expected: all commands pass.

- [ ] **Step 2: Run the clean full gate**

```bash
pnpm verify:clean
```

Expected: exit 0; the before/after porcelain snapshots match.

- [ ] **Step 3: Inspect tracked changes**

```bash
git diff --check
git status --short
```

Expected: no tracked change remains from the plan; only pre-existing user-owned untracked artifacts may appear.
