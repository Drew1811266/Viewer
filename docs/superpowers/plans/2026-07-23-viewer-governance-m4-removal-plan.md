# Viewer Governance and M4 Removal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove M4 and Viewer 0.1 delivery semantics from active governance while preserving historical evidence and continuous safety requirements.

**Architecture:** Introduce ADR 0005 as the new governance source of truth, make scope status validation explicit, migrate active documents, and mark historical M4 references without rewriting their original evidence.

**Tech Stack:** Markdown, Node.js 24 test runner, ECMAScript modules, repository policy scripts.

## Global Constraints

- Viewer 0.1 is a small development-stage baseline, not a release or delivery target.
- Do not add product features or change runtime behavior.
- Preserve every stable `REQ-*` identifier exactly once in the scope matrix.
- Allowed scope stages are exactly `M1`, `M2`, `M3`, `Continuous`, `Future`, and `NotApplicable`.
- Historical G1–G4 and M1–M3 evidence must remain intact.
- Active documents must not assign ownership, prerequisites, or completion to M4.
- Pure distribution, signing, notarization, clean-install, and final-release acceptance requirements are `NotApplicable`.
- Existing automated safety, privacy, dependency, memory, and performance regression requirements remain `Continuous`.

---

### Task 1: Make Scope Status Validation Explicit

**Files:**
- Create: `scripts/scope-coverage.mjs`
- Create: `scripts/scope-coverage.test.mjs`
- Modify: `scripts/check-scope-coverage.mjs`
- Modify: `package.json`

**Interfaces:**
- Produces: `VALID_STAGES: ReadonlySet<string>`
- Produces: `validateScopeCoverage(productSpec: string, matrix: string): { requirementCount: number; m3Count: number }`
- Consumes: Markdown source strings; no filesystem access inside the validator.

- [ ] **Step 1: Write failing unit tests for the new stage model**

Create `scripts/scope-coverage.test.mjs`:

```js
import assert from 'node:assert/strict'
import test from 'node:test'

import { VALID_STAGES, validateScopeCoverage } from './scope-coverage.mjs'

const spec = '# Product\n\n## Requirement [REQ-ONE]\n\n## Requirement [REQ-TWO]\n'
const matrix = (oneStage, twoStage) => `| Requirement ID | Summary | Owner | Stage | Tests | Acceptance | Gate |
| --- | --- | --- | --- | --- | --- | --- |
| REQ-ONE | One | Owner | ${oneStage} | Test one | Accept one | G1 |
| REQ-TWO | Two | Owner | ${twoStage} | Test two | Accept two | G2 |
`

test('scope stages include continuous development and exclude M4', () => {
  assert.deepEqual([...VALID_STAGES], [
    'M1',
    'M2',
    'M3',
    'Continuous',
    'Future',
    'NotApplicable',
  ])
})

test('continuous, future and not-applicable rows remain mapped exactly once', () => {
  assert.deepEqual(validateScopeCoverage(spec, matrix('Continuous', 'Future')), {
    requirementCount: 2,
    m3Count: 0,
  })
  assert.equal(
    validateScopeCoverage(spec, matrix('M1/Continuous', 'NotApplicable')).requirementCount,
    2,
  )
})

test('M4 ownership is rejected', () => {
  assert.throws(
    () => validateScopeCoverage(spec, matrix('M4', 'Continuous')),
    /invalid stage token M4 in REQ-ONE/,
  )
})
```

- [ ] **Step 2: Run the test and verify it fails**

Run:

```bash
node --test scripts/scope-coverage.test.mjs
```

Expected: FAIL with `ERR_MODULE_NOT_FOUND` for `scripts/scope-coverage.mjs`.

- [ ] **Step 3: Extract the validator from the CLI**

Create `scripts/scope-coverage.mjs` with these public definitions and move the existing duplicate-ID, exact-coverage, seven-column, and frozen-M3 checks into `validateScopeCoverage`:

```js
export const VALID_STAGES = new Set([
  'M1',
  'M2',
  'M3',
  'Continuous',
  'Future',
  'NotApplicable',
])

const idPattern = /\bREQ-[A-Z0-9]+(?:-[A-Z0-9]+)*\b/g

export function validateScopeCoverage(productSpec, matrix) {
  const specIds = [...productSpec.matchAll(idPattern)].map(([id]) => id)
  const matrixRows = matrix
    .split(/\r?\n/)
    .filter((line) => /^\|\s*REQ-[A-Z0-9-]+\s*\|/.test(line))
    .map((line) => line.split('|').slice(1, -1).map((cell) => cell.trim()))

  assertNoDuplicates('product spec', specIds)
  assertNoDuplicates('scope matrix', matrixRows.map(([id]) => id))
  assertExactCoverage(specIds, matrixRows.map(([id]) => id))

  for (const row of matrixRows) {
    if (row.length !== 7) {
      throw new Error(`matrix row ${row[0]} must contain exactly 7 columns`)
    }
    const [id, summary, owner, stage, tests, acceptance, gate] = row
    for (const [name, value] of Object.entries({
      summary,
      owner,
      stage,
      tests,
      acceptance,
      gate,
    })) {
      if (!value || value === '—') throw new Error(`matrix row ${id} has an empty ${name} column`)
    }
    for (const token of stage.split('/')) {
      if (!VALID_STAGES.has(token)) throw new Error(`invalid stage token ${token} in ${id}`)
    }
  }

  const actualM3Ids = matrixRows
    .filter(([, , , stage]) => stage.split('/').includes('M3'))
    .map(([id]) => id)
    .sort()
  assertFrozenM3(actualM3Ids)

  if (specIds.length === 0) throw new Error('product spec contains no stable requirement IDs')
  return { requirementCount: specIds.length, m3Count: actualM3Ids.length }
}
```

Keep the existing `expectedM3Ids` values unchanged. Implement `assertNoDuplicates`, `assertExactCoverage`, and `assertFrozenM3` by moving the current script logic without altering messages other than `milestone` → `stage`.

Replace `scripts/check-scope-coverage.mjs` with a filesystem-only CLI:

```js
#!/usr/bin/env node

import { readFileSync } from 'node:fs'
import { validateScopeCoverage } from './scope-coverage.mjs'

const result = validateScopeCoverage(
  readFileSync('docs/PRODUCT_SPEC.md', 'utf8'),
  readFileSync('docs/milestones/viewer-0.1-scope-matrix.md', 'utf8'),
)

console.log(
  `Viewer scope coverage passed: ${result.requirementCount} requirements mapped exactly once; ${result.m3Count} frozen M3 requirements`,
)
```

Add the focused test to root scripts:

```json
{
  "scripts": {
    "test:policy": "node --test scripts/repository-policy.test.mjs scripts/scope-coverage.test.mjs"
  }
}
```

- [ ] **Step 4: Run focused validation**

Run:

```bash
node --test scripts/scope-coverage.test.mjs
node scripts/check-scope-coverage.mjs
```

Expected: the unit tests pass; the CLI still fails because active matrix rows still contain `M4`.

- [ ] **Step 5: Commit the validator extraction**

```bash
git add scripts/scope-coverage.mjs scripts/scope-coverage.test.mjs scripts/check-scope-coverage.mjs package.json
git commit -m "test: model continuous scope ownership"
```

### Task 2: Record the Governance Decision

**Files:**
- Create: `docs/adr/0005-continuous-development-governance.md`
- Modify: `scripts/repository-policy.test.mjs`

**Interfaces:**
- Produces: ADR 0005 as the source of truth for Viewer 0.1 and M4 semantics.
- Produces: repository-policy protection against reintroducing active M4 ownership.

- [ ] **Step 1: Add a failing active-governance policy test**

Append to `scripts/repository-policy.test.mjs`:

```js
test('active governance has no M4 owner or Viewer 0.1 delivery gate', async () => {
  const active = await Promise.all([
    read('docs/PRODUCT_SPEC.md'),
    read('docs/TECHNICAL_FOUNDATIONS.md'),
    read('docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md'),
    read('docs/milestones/viewer-0.1-scope-matrix.md'),
    read('docs/architecture/viewer-0.1-api-baseline.md'),
  ])
  const text = active.join('\n')
  for (const forbidden of [
    /\bM4 Internal Release\b/i,
    /M4 内部发布/,
    /next executable action[^.]*\bM4\b/i,
    /M4 final acceptance/i,
    /Viewer 0\.1 for Mac/i,
  ]) {
    assert.doesNotMatch(text, forbidden)
  }
})
```

- [ ] **Step 2: Run the test and verify it fails**

Run:

```bash
node --test scripts/repository-policy.test.mjs
```

Expected: FAIL on `M4 Internal Release` or `M4 内部发布`.

- [ ] **Step 3: Create ADR 0005**

Create `docs/adr/0005-continuous-development-governance.md`:

```markdown
# ADR 0005: Replace M4 release acceptance with continuous engineering governance

> Status: Accepted
>
> Date: 2026-07-23

## Context

Viewer 0.1 is an early development stage with substantial product work still ahead. Treating 0.1 as an internal release target and assigning remaining quality work to M4 conflates continuous engineering constraints with delivery acceptance.

## Decision

Viewer 0.1 is a development-stage baseline, not a release or delivery target. The M4 milestone is cancelled. Deterministic safety, dependency, regression and accessibility checks move to continuous gates or the owning feature's definition of done. Real-media, Finder, VoiceOver, soak, signing, notarization and clean-install exercises are optional specialist checks and do not block stage completion.

Active scope uses M1, M2, M3, Continuous, Future and NotApplicable. No renamed phase may recreate a centralized final acceptance gate.

## Consequences

- Existing G1–G4 and M1–M3 evidence remains historical.
- Active roadmaps no longer point to M4.
- Distribution and release acceptance requirements are not applicable to the current development stage.
- Synthetic regression evidence remains valid only for the behavior it measures.
- Future product features are planned independently from this engineering optimization program.
```

- [ ] **Step 4: Run the policy test and confirm it still fails only on active documents**

Run:

```bash
node --test scripts/repository-policy.test.mjs
```

Expected: the new test still fails; all pre-existing policy tests pass.

- [ ] **Step 5: Commit the ADR and failing policy**

```bash
git add docs/adr/0005-continuous-development-governance.md scripts/repository-policy.test.mjs
git commit -m "docs: record continuous development governance"
```

### Task 3: Migrate Product Spec and Roadmap

**Files:**
- Modify: `docs/PRODUCT_SPEC.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md`

**Interfaces:**
- Consumes: ADR 0005.
- Produces: active product and roadmap language with no delivery target.

- [ ] **Step 1: Replace Product Spec delivery language**

In `docs/PRODUCT_SPEC.md`, replace section 8 with this stage model while keeping all existing `REQ-*` IDs:

```markdown
## 8. 当前开发阶段

### 8.1 Viewer 0.1 定位

Viewer 0.1 是持续开发中的小型阶段性基线，不代表功能完成、内部发布或正式交付。M1、M2、M3 记录已经实现并验证的能力；后续功能按独立设计与计划继续开发，不设置 M4 或集中式最终验收阶段。

### 8.2 当前必须持续保持 `[REQ-SCOPE-MUST]`

- 已实现能力不得回归，数据安全、路径边界、权限最小化和本地优先约束持续生效。
- 新功能必须通过对应模块的自动化测试与持续门禁。
- 合成性能数据只用于回归比较，不代表真实素材最终验收。

### 8.3 当前明确不做 `[REQ-SCOPE-EXCLUSIONS]`

- 图片或文本编辑。
- AI 图片生成、提示词生成或模型调用。
- 云同步、多人协作、账户和在线服务。
- 同时打开多个项目或跨项目搜索。
- 插件系统和快捷键自定义。
- JPG、PNG、Markdown、TXT 之外的文件预览格式。
- Mac App Store 上架、自动更新和公开销售能力。
- Windows 版本的开发、打包与测试。

### 8.4 Windows 边界 `[REQ-SCOPE-WINDOWS]`

当前不开发 Windows 版本，也不为 Windows 进度牺牲当前 Mac 开发阶段的体验和推进速度。核心对象、元数据版本和业务接口避免无必要地绑定 macOS，但所有 Windows 兼容性均属于未来重新评估的工作，不计入当前开发阶段。

### 8.5 发布验收状态 `[REQ-RELEASE-ACCEPTANCE]`

当前开发阶段不设置发布验收。`.app`/DMG 交付、签名、公证、真实素材最终验收、干净机器安装和两小时稳定性验收均不属于 Viewer 0.1。
```

Remove the stale “后续文档章节” list. Replace the final decision list with:

```markdown
1. Viewer 自身采用 Apache-2.0。
2. G1–G4 和 M1–M3 是已完成的历史技术与功能证据。
3. Viewer 0.1 是阶段性开发基线，不是发布或交付目标。
4. M4 已由 ADR 0005 取消；持续工程约束由日常门禁承担。
```

- [ ] **Step 2: Replace the roadmap tail**

In `docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md`:

- Change the goal from delivery after M1–M4 to documenting completed foundation/G1–G4/M1–M3 work and continuing feature development.
- Remove the “no test build until all four milestones” global constraint.
- Rename `Product Milestones After G4` to `Completed Product Milestones`.
- Remove the M4 section and the instruction to create an M4 plan.
- Replace the M3 tail with:

```markdown
M3 closes the historical milestone sequence. Subsequent product work starts from an independently approved design and plan. Continuous engineering constraints follow ADR 0005 and are not grouped into a final release milestone.
```

- Replace `M1–M4 continuous checks` with `Continuous`.
- Replace `M4 final acceptance` with `Continuous regression`.
- Add a “Current Governance” section linking ADR 0005 and the approved engineering optimization design.

- [ ] **Step 3: Run the active-governance policy**

Run:

```bash
node --test scripts/repository-policy.test.mjs
```

Expected: the M4 policy may still fail on Technical Foundations, scope matrix, or API baseline, but not on Product Spec or roadmap.

- [ ] **Step 4: Commit active product and roadmap migration**

```bash
git add docs/PRODUCT_SPEC.md docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md
git commit -m "docs: redefine viewer 0.1 as a development baseline"
```

### Task 4: Reassign Every Scope Matrix Row

**Files:**
- Modify: `docs/milestones/viewer-0.1-scope-matrix.md`

**Interfaces:**
- Consumes: `VALID_STAGES`.
- Produces: exactly one seven-column row per Product Spec requirement.

- [ ] **Step 1: Replace matrix terminology and stage assignments**

Rename column `Milestone` to `Stage`. Replace every stage containing M4 according to this exact mapping:

| Requirement | New stage |
| --- | --- |
| REQ-SCOPE-BOUNDARY | M1/Continuous |
| REQ-PRODUCT-PRINCIPLES | M1/Continuous |
| REQ-FLOW-SHORTCUTS | M2/M3/Continuous |
| REQ-FLOW-TEXT-PREVIEW | M1/Continuous |
| REQ-IA-SURFACES | M1/Future |
| REQ-IA-IMAGE-PREVIEW | M1/Continuous |
| REQ-IA-ACCESSIBILITY | Future |
| REQ-IA-COLOR-ORIENTATION | Continuous |
| REQ-TECH-BASE | M1/Continuous |
| REQ-TECH-PORTABLE-METADATA | M2/Future |
| REQ-TECH-PERFORMANCE | Continuous |
| REQ-TECH-MEMORY | M2/Continuous |
| REQ-TECH-CACHE | M1/Continuous |
| REQ-TECH-PATH-SECURITY | M1/M3/Continuous |
| REQ-TECH-LARGE-FILES | M1/Continuous |
| REQ-TECH-PRIVACY | Continuous/Future |
| REQ-TECH-DISTRIBUTION | NotApplicable |
| REQ-TECH-OPEN-SOURCE | Continuous |
| REQ-SCOPE-MUST | Future |
| REQ-SCOPE-EXCLUSIONS | Continuous |
| REQ-SCOPE-WINDOWS | Continuous |
| REQ-RELEASE-ACCEPTANCE | NotApplicable |

Replace the ownership summary with:

```markdown
- **M1–M3:** completed historical product milestones.
- **Continuous:** constraints and regression checks applied to ordinary development.
- **Future:** unimplemented product behavior requiring a separate future design and plan.
- **NotApplicable:** release/distribution acceptance outside the current development stage.
```

- [ ] **Step 2: Run scope validation**

Run:

```bash
node scripts/check-scope-coverage.mjs
```

Expected: PASS with 47 requirements mapped exactly once and 13 frozen M3 requirements.

- [ ] **Step 3: Run policy tests**

Run:

```bash
node --test scripts/scope-coverage.test.mjs scripts/repository-policy.test.mjs
```

Expected: scope tests pass; active-governance policy may still fail only on remaining active documents.

- [ ] **Step 4: Commit matrix migration**

```bash
git add docs/milestones/viewer-0.1-scope-matrix.md
git commit -m "docs: move scope ownership to continuous development"
```

### Task 5: Migrate Technical Foundations and API Baseline

**Files:**
- Modify: `docs/TECHNICAL_FOUNDATIONS.md`
- Modify: `docs/architecture/viewer-0.1-api-baseline.md`
- Modify: `docs/adr/0004-viewer-0.1-architecture-freeze.md`

**Interfaces:**
- Produces: active architecture language compatible with ADR 0005.
- Preserves: measured G1/G3 benchmark numbers and accepted API rows.

- [ ] **Step 1: Correct Technical Foundations status**

Make these exact semantic changes:

- Replace “Viewer 0.1 技术基线已冻结” with “G1～G4 技术证据已完成；当前开发治理见 ADR 0005”.
- Keep M4 in “Apple M4” hardware names.
- Remove sentences requiring a future M4 real-folder rerun.
- Add:

```markdown
合成夹具数据只用于持续回归比较，不代表真实素材最终验收。真实素材体验检查可以按功能需要单独执行，但不阻塞阶段完成。
```

- Change the TanStack Virtual row from `正式依赖候选` to the implementation actually present in `ui/package.json`; if absent, describe the custom `VirtualGrid`/`VirtualList` implementation and do not list TanStack as a dependency.

- [ ] **Step 2: Update the API baseline**

Replace M1–M4 lifecycle wording with:

```markdown
This document inventories the public Rust surface consumed by adapters, infrastructure, the Tauri composition root and current development tests. An incompatible change requires architecture review, an updated row here and the named focused gate.
```

Do not change API rows or gate names.

- [ ] **Step 3: Supersede only the freeze consequence in ADR 0004**

Add below the ADR 0004 status:

```markdown
> Governance note (2026-07-23): ADR 0005 supersedes the Viewer 0.1 freeze and M1–M4 delivery consequences. The accepted G1–G4 evidence and dependency/API facts remain historical evidence.
```

- [ ] **Step 4: Run active policy**

Run:

```bash
node --test scripts/repository-policy.test.mjs
```

Expected: PASS, including “active governance has no M4 owner or Viewer 0.1 delivery gate”.

- [ ] **Step 5: Commit architecture document migration**

```bash
git add docs/TECHNICAL_FOUNDATIONS.md docs/architecture/viewer-0.1-api-baseline.md docs/adr/0004-viewer-0.1-architecture-freeze.md
git commit -m "docs: align architecture baseline with continuous governance"
```

### Task 6: Mark Historical M4 References

**Files:**
- Modify: `docs/reviews/2026-07-16-g1-image-pipeline-review.md`
- Modify: `docs/reviews/2026-07-16-g3-scan-search-review.md`
- Modify: `docs/reviews/2026-07-16-g4-architecture-freeze-review.md`
- Modify: `docs/reviews/2026-07-16-m1-browsing-core-review.md`
- Modify: `docs/reviews/2026-07-16-m2-review-efficiency-review.md`
- Modify: `docs/reviews/2026-07-16-m3-organization-comparison-review.md`
- Modify: `docs/adr/0001-macos-image-pipeline.md`
- Modify: `docs/adr/0003-scan-search-and-generation.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-g1-image-pipeline-plan.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-g3-scan-search-plan.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-g4-architecture-freeze-plan.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-m1-browsing-core-plan.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-m2-review-efficiency-plan.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-m3-organization-comparison-plan.md`
- Modify: `docs/superpowers/plans/2026-07-20-viewer-m3-drag-session-rearchitecture-plan.md`
- Modify: `docs/superpowers/specs/2026-07-16-viewer-m3-organization-comparison-design.md`
- Modify: `docs/superpowers/specs/2026-07-16-viewer-system-architecture-design.md`
- Modify: `docs/superpowers/specs/2026-07-22-viewer-simplified-workspace-radial-menu-design.md`

**Interfaces:**
- Produces: explicit historical status without editing original evidence.

- [ ] **Step 1: Add the historical banner**

Insert after each title:

```markdown
> **Historical governance note (2026-07-23):** References to M4 or Viewer 0.1 release acceptance below describe the plan at the time this evidence was recorded. ADR 0005 cancelled M4 and replaced release acceptance with continuous development governance. The measured evidence in this document is unchanged.
```

For the simplified workspace design, use:

```markdown
> **Superseded target note (2026-07-23):** “M4 界面收敛” no longer names an active milestone. The approved interaction design remains valid; current governance follows ADR 0005.
```

- [ ] **Step 2: Verify all remaining M4 references are historical or cancellation records**

Run:

```bash
rg -n -i '\bM4\b|M4内部|M4 内部' docs
```

Expected: every result is an explicit M4 cancellation statement in an active governance/optimization source, or appears in a document containing the historical/superseded banner. No result assigns current work or acceptance ownership to M4.

- [ ] **Step 3: Commit historical annotations**

```bash
git add docs/reviews docs/adr/0001-macos-image-pipeline.md docs/adr/0003-scan-search-and-generation.md docs/superpowers/plans/2026-07-16-viewer-g1-image-pipeline-plan.md docs/superpowers/plans/2026-07-16-viewer-g3-scan-search-plan.md docs/superpowers/plans/2026-07-16-viewer-g4-architecture-freeze-plan.md docs/superpowers/plans/2026-07-16-viewer-m1-browsing-core-plan.md docs/superpowers/plans/2026-07-16-viewer-m2-review-efficiency-plan.md docs/superpowers/plans/2026-07-16-viewer-m3-organization-comparison-plan.md docs/superpowers/plans/2026-07-20-viewer-m3-drag-session-rearchitecture-plan.md docs/superpowers/specs/2026-07-16-viewer-m3-organization-comparison-design.md docs/superpowers/specs/2026-07-16-viewer-system-architecture-design.md docs/superpowers/specs/2026-07-22-viewer-simplified-workspace-radial-menu-design.md
git commit -m "docs: mark former M4 references as historical"
```

### Task 7: Run the Governance Exit Gate

**Files:**
- Verify only; do not edit unrelated files.

**Interfaces:**
- Confirms: Plan 1 outputs satisfy ADR 0005 and existing repository gates.

- [ ] **Step 1: Run focused policy and scope tests**

```bash
node --test scripts/scope-coverage.test.mjs scripts/repository-policy.test.mjs
node scripts/check-scope-coverage.mjs
```

Expected: all tests pass; 47 requirements are mapped exactly once.

- [ ] **Step 2: Run the full repository gate**

```bash
pnpm verify
```

Expected: exit 0 with UI tests/build, Rust fmt, Clippy and workspace tests passing.

- [ ] **Step 3: Inspect the diff and worktree**

```bash
git diff --check
git status --short
```

Expected: no whitespace errors; only pre-existing user-owned untracked artifacts may remain.
