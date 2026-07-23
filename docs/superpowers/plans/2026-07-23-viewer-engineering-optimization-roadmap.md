# Viewer Engineering Optimization Roadmap

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Execute the approved engineering optimization design without adding product features, changing user behavior, or recreating an M4-style final acceptance phase.

**Architecture:** The work is split into six independently reviewable plans. Establish governance and deterministic gates first, then refactor React, Tauri, and Infrastructure behind existing contracts, and finally add non-release quality trend governance.

**Tech Stack:** Tauri 2, Rust 1.97.0, React 19, TypeScript 6, Vite 8, Vitest 4, pnpm 10, SQLite/rusqlite, GitHub Actions on macos-15.

## Global Constraints

- Viewer 0.1 is a small development-stage baseline, not a release or delivery target.
- No M4 milestone, M4 owner, final acceptance phase, `.app`/DMG delivery, signing, notarization, real-media final acceptance, or two-hour release gate may be introduced.
- Preserve all existing user behavior, IPC command/event names, DTO JSON shapes, Domain/Application contracts, operation journal transitions, `.viewer` schema, and safe error codes.
- Supported content remains JPG, JPEG, PNG, Markdown, and TXT.
- Runtime content remains local; do not add login, telemetry, upload, update, HTTP, shell, broad filesystem, or database permissions.
- Keep `Cargo.lock` and `pnpm-lock.yaml` committed and use locked/frozen verification.
- Every refactor task starts from characterization tests, moves one responsibility, runs focused tests, and ends with a small commit; each sub-plan then runs its full exit gate.
- Existing untracked `.DS_Store` and `tests/fixtures/images/.viewer/` content belongs to the user and must never be staged by these plans.

---

## Plan Sequence

### Plan 1: Governance and M4 Removal

Execute [2026-07-23-viewer-governance-m4-removal-plan.md](./2026-07-23-viewer-governance-m4-removal-plan.md).

Produces:

- ADR 0005 defining the continuous-development governance model.
- Active product, roadmap, scope matrix, API baseline, and technical foundation documents without M4 ownership or 0.1 delivery semantics.
- Scope validation that accepts `M1`, `M2`, `M3`, `Continuous`, `Future`, and `NotApplicable`.
- Historical documents marked as historical or superseded without rewriting their evidence.

Exit gate:

```bash
node --test scripts/scope-coverage.test.mjs scripts/repository-policy.test.mjs
node scripts/check-scope-coverage.mjs
pnpm verify
```

Expected: every command exits 0, and no active source-of-truth document assigns work to M4.

### Plan 2: Engineering Baseline and CI

Execute [2026-07-23-viewer-engineering-baseline-ci-plan.md](./2026-07-23-viewer-engineering-baseline-ci-plan.md).

Consumes:

- ADR 0005 and the active-document list from Plan 1.

Produces:

- `README.md`, `CONTRIBUTING.md`, `.editorconfig`, corrected ignore rules.
- TypeScript `strict`, Biome lint/format, and a separately tracked `noUncheckedIndexedAccess` migration.
- Stable `quality`, `security`, `verify`, and `verify:clean` commands.
- Behavior-based repository policy tests and deterministic CI groups.

Exit gate:

```bash
pnpm verify:clean
```

Expected: exit 0 and the worktree porcelain snapshot is unchanged from before the command.

### Plan 3: React Decomposition

Execute [2026-07-23-viewer-react-decomposition-plan.md](./2026-07-23-viewer-react-decomposition-plan.md).

Consumes:

- Strict TypeScript and Biome gates from Plan 2.

Produces:

- Focused search, operation, project-session, selection/marker, and lifecycle controllers.
- Project, search, workspace, and operation reducer modules behind the existing `viewerReducer` facade.
- Focused App coordinators and ContentBrowser helpers.

Exit gate:

```bash
pnpm --dir ui test
pnpm --dir ui build
pnpm verify
```

Expected: all existing UI behavior tests and the full repository gate pass.

### Plan 4: Tauri Runtime Decomposition

Execute [2026-07-23-viewer-tauri-runtime-decomposition-plan.md](./2026-07-23-viewer-tauri-runtime-decomposition-plan.md).

Consumes:

- Deterministic gates from Plan 2.

Produces:

- Domain-specific DTO modules with stable `crate::dto::*` re-exports.
- Session, scan/index, preview, marker, and organization runtime services behind `DesktopRuntime`.
- Operation state, publication, commit, and undo modules behind `OperationRuntime`.

Exit gate:

```bash
cargo test --locked -p viewer-desktop
./scripts/check-tauri-security.sh
pnpm verify
```

Expected: public IPC fixtures, security boundaries, lifecycle tests, and the full gate pass.

### Plan 5: Infrastructure Decomposition

Execute [2026-07-23-viewer-infrastructure-decomposition-plan.md](./2026-07-23-viewer-infrastructure-decomposition-plan.md).

Consumes:

- Stable Application ports and deterministic gates.

Produces:

- Focused file-reference, staged-copy, evidence, placement, preflight, batch, execution, result, schema, writer, query, and projection modules.
- Stable `LocalFileMutation`, `LocalFileCommandAdapter`, and `SessionIndex` facades.

Exit gate:

```bash
cargo test --locked -p viewer-infrastructure
cargo test --locked -p viewer-infrastructure --test file_transactions
cargo test --locked -p viewer-infrastructure --test m3_file_commands
cargo test --locked -p viewer-infrastructure --test search
pnpm verify
```

Expected: file safety, recovery, search, and full repository tests pass without contract changes.

### Plan 6: Continuous Quality Governance

Execute [2026-07-23-viewer-continuous-quality-governance-plan.md](./2026-07-23-viewer-continuous-quality-governance-plan.md).

Consumes:

- The stable module boundaries from Plans 3–5.

Produces:

- Frontend and Rust coverage reports with checked-in baselines.
- Architecture-health trend reporting for file size, function size, module direction, and test ratio.
- Dependency exception ownership and expiry checks.
- Scheduled network audit workflow.
- Active/Superseded/Historical documentation index.

Exit gate:

```bash
pnpm quality:report
pnpm security
pnpm verify:clean
```

Expected: deterministic gates pass; trend reports are generated; no M4-equivalent completion gate exists.

## Cross-Plan Interfaces

| Producer | Interface | Consumers |
| --- | --- | --- |
| Plan 1 | `VALID_STAGES = ['M1', 'M2', 'M3', 'Continuous', 'Future', 'NotApplicable']` | Scope matrix and repository policy |
| Plan 1 | ADR 0005 plus active-document inventory | Plans 2 and 6 |
| Plan 2 | `pnpm quality`, `pnpm security`, `pnpm verify`, `pnpm verify:clean` | Plans 3–6 |
| Plan 2 | TypeScript strict and Biome configuration | Plan 3 |
| Plan 3 | Stable `useViewerController` and `viewerReducer` facades | Existing React components/tests |
| Plan 4 | Stable `DesktopRuntime`, `OperationRuntime`, and `crate::dto::*` facades | Tauri commands/tests |
| Plan 5 | Stable `LocalFileMutation`, `LocalFileCommandAdapter`, and `SessionIndex` facades | Desktop runtime and integration tests |
| Plan 6 | Baseline JSON schemas and exception register | Future continuous development |

## Execution Rules

- [ ] Execute exactly one sub-plan at a time.
- [ ] Start each sub-plan from a worktree created with `superpowers:using-git-worktrees`.
- [ ] Run the sub-plan entry gate before making changes.
- [ ] Keep unrelated user changes out of every commit.
- [ ] Stop immediately on an unexplained security, recovery, or data-consistency regression.
- [ ] Review each task commit before starting the next task.
- [ ] Run the sub-plan exit gate before marking that plan complete.
- [ ] Do not defer a failed deterministic gate to a later plan.
