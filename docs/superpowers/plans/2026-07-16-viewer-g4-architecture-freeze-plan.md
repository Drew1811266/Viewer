# Viewer G4 Architecture Freeze Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert G1–G3 prototype evidence into the frozen Viewer 0.1 dependency, security, performance and cross-crate API baseline.

**Architecture:** G4 adds no product features. It verifies that the modular boundaries and safety assumptions survived real prototypes, updates the architecture when evidence disagrees, and produces an auditable baseline for M1–M4 implementation.

**Tech Stack:** Existing Viewer workspace, cargo metadata/deny/audit, pnpm lockfile/audit, Tauri capability/CSP tests, benchmark evidence and ADRs.

## Global Constraints

- G1, G2 and G3 must each have a passing script and an approved ADR before G4 can pass.
- A failed gate cannot be converted into a pass by changing only documentation wording.
- Direct dependencies are frozen through committed lockfiles and an explicit direct-dependency table.
- Runtime network, telemetry, updater, broad filesystem, shell and SQL frontend capabilities remain absent.
- The architecture spec is updated before freezing if a prototype changed an interface or backend policy.
- No product milestone implementation starts until G4 has one explicit approved outcome.

---

## Target File Map

```text
scripts/run-architecture-gates.sh
scripts/check-tauri-security.sh
scripts/check-locked-dependencies.sh
tests/security_boundaries.rs
docs/adr/0004-viewer-0.1-architecture-freeze.md
docs/architecture/viewer-0.1-api-baseline.md
docs/milestones/viewer-0.1-scope-matrix.md
THIRD_PARTY_NOTICES.md
SECURITY.md
Cargo.lock
pnpm-lock.yaml
docs/superpowers/specs/2026-07-16-viewer-system-architecture-design.md
```

### Task 1: Aggregate and validate G1–G3 evidence

**Files:**
- Create: `scripts/run-architecture-gates.sh`
- Create: `docs/adr/0004-viewer-0.1-architecture-freeze.md`

**Interfaces:**
- Consumes: `run-g1-image-gate.sh`, `run-g2-file-transaction-gate.sh`, `run-g3-scan-search-gate.sh`, ADR 0001–0003.
- Produces: one reproducible architecture gate command and evidence manifest.

- [ ] **Step 1: Write the aggregate script**

```bash
#!/usr/bin/env bash
set -euo pipefail

required=(
  docs/adr/0001-macos-image-pipeline.md
  docs/adr/0002-file-transaction-protocol.md
  docs/adr/0003-scan-search-and-generation.md
)

for path in "${required[@]}"; do
  test -s "$path" || { echo "missing gate evidence: $path"; exit 1; }
done

./scripts/run-g1-image-gate.sh
./scripts/run-g2-file-transaction-gate.sh
./scripts/run-g3-scan-search-gate.sh
pnpm verify
cargo deny check
echo "Viewer architecture gates passed"
```

- [ ] **Step 2: Run without G4 conclusions**

Run `chmod +x scripts/run-architecture-gates.sh` and then execute it. Expected: all gate commands exit 0. If any fails, stop G4 and repair/re-review that gate first.

- [ ] **Step 3: Create the ADR 0004 evidence table**

The ADR contains one row for every gate with commit hash, command, test corpus checksum, standard-device OS/hardware, p50/p95, peak RSS, selected outcome and linked raw report. Set overall status to `Proposed` until Tasks 2–5 pass.

- [ ] **Step 4: Commit**

```bash
git add scripts/run-architecture-gates.sh docs/adr/0004-viewer-0.1-architecture-freeze.md
git commit -m "test: aggregate Viewer architecture gates"
```

### Task 2: Freeze and audit direct dependencies

**Files:**
- Create: `scripts/check-locked-dependencies.sh`
- Create: `LICENSE`
- Create: `THIRD_PARTY_NOTICES.md`
- Modify: `deny.toml`
- Modify: `docs/TECHNICAL_FOUNDATIONS.md`
- Modify: `Cargo.lock`
- Modify: `pnpm-lock.yaml`

**Interfaces:**
- Produces: immutable installs and a direct dependency/license inventory for Viewer 0.1.

- [ ] **Step 1: Obtain and apply the Viewer project license decision**

Ask the user to explicitly choose Viewer's own repository license before continuing this task. Present MIT, Apache-2.0 and MIT OR Apache-2.0 with their attribution/patent trade-offs; do not infer a choice from dependency licenses. Add the exact approved license text to `LICENSE`, then add the matching SPDX expression to `[workspace.package]` and every distributable package.

- [ ] **Step 2: Add locked dependency verification**

```bash
#!/usr/bin/env bash
set -euo pipefail

cargo metadata --locked --format-version 1 >/dev/null
pnpm install --frozen-lockfile
cargo deny check advisories bans licenses sources
pnpm audit --audit-level high
echo "locked dependencies verified"
```

The script must not run an updater or modify either lockfile.

- [ ] **Step 3: Generate and manually review the direct dependency table**

List every direct Cargo and npm dependency with exact resolved version, license, purpose, upstream URL and whether it enters the distributed binary. Include Tauri, React, Vite, rusqlite, Quick Look/Image I/O bindings, notify, trash-rs, nucleo-matcher, test-only dependencies and build tools in separate sections.

`THIRD_PARTY_NOTICES.md` contains actual build dependencies only. Architecture inspiration remains in `ACKNOWLEDGEMENTS.md` and is not mislabeled as linked code.

- [ ] **Step 4: Enforce license policy**

Allow permissive licenses and the already-reviewed unmodified MPL-2.0 dependency. Deny GPL, AGPL, SSPL, FSL, unknown and unlicensed packages unless the architecture review is reopened with a written exception.

- [ ] **Step 5: Verify and commit**

Run `./scripts/check-locked-dependencies.sh`. Expected: exit 0 and no lockfile change in `git status --short`.

```bash
git add LICENSE Cargo.toml crates src-tauri/Cargo.toml scripts/check-locked-dependencies.sh THIRD_PARTY_NOTICES.md deny.toml docs/TECHNICAL_FOUNDATIONS.md Cargo.lock pnpm-lock.yaml
git commit -m "chore: freeze Viewer 0.1 dependencies"
```

### Task 3: Freeze the Tauri security boundary

**Files:**
- Create: `scripts/check-tauri-security.sh`
- Create: `tests/security_boundaries.rs`
- Create: `SECURITY.md`
- Modify: `src-tauri/capabilities/main.json`
- Modify: `src-tauri/tauri.conf.json`

**Interfaces:**
- Produces: an executable allowlist for commands, protocols and CSP sources.

- [ ] **Step 1: Add negative security tests**

Tests must assert:

- project-relative parser rejects absolute paths, `..`, `.viewer` and symlink targets;
- image protocol rejects unknown session/token, another session, non-image MIME and traversal syntax;
- Markdown sanitizer removes script, iframe, inline event handlers, remote image and remote stylesheet;
- frontend capability does not include filesystem, shell, SQL, HTTP, updater, websocket or upload permissions;
- CSP contains no `*`, remote `http:`, remote `https:`, `unsafe-eval` or broad `connect-src`.

Run tests before enforcement. Expected: any current gap fails explicitly.

- [ ] **Step 2: Reduce capabilities and CSP to the tested allowlist**

The main capability may include `core:default`, the folder-open dialog and opener permission for user-clicked external links. All project file operations remain custom Rust commands. The custom image scheme is allowed only in `img-src`; `connect-src` remains limited to Tauri IPC.

Register a navigation handler that permits the bundled application origin and rejects other WebView navigations. User-clicked external links go through the OS opener, not WebView navigation.

- [ ] **Step 3: Add static configuration check**

`check-tauri-security.sh` parses JSON rather than grepping and fails when forbidden capability prefixes or CSP sources appear. It then runs `cargo test --test security_boundaries`.

- [ ] **Step 4: Add security reporting policy**

`SECURITY.md` documents supported version 0.1, private vulnerability contact instructions without promising an unavailable SLA, what diagnostic data to omit, and that public GitHub issues should not contain undisclosed vulnerabilities or user project data.

- [ ] **Step 5: Verify and commit**

Run `./scripts/check-tauri-security.sh`. Expected: exit 0.

```bash
git add scripts/check-tauri-security.sh tests/security_boundaries.rs SECURITY.md src-tauri
git commit -m "security: freeze Viewer Tauri boundary"
```

### Task 4: Freeze cross-crate APIs and architecture decisions

**Files:**
- Create: `docs/architecture/viewer-0.1-api-baseline.md`
- Modify: `docs/superpowers/specs/2026-07-16-viewer-system-architecture-design.md`
- Modify: `docs/adr/0004-viewer-0.1-architecture-freeze.md`

**Interfaces:**
- Produces: reviewed public API list for Domain/Application ports used by M1–M4.

- [ ] **Step 1: Generate the public API inventory**

Run `cargo doc --workspace --no-deps` and list every public type/function in `viewer-domain` and every public port/use case in `viewer-application`. For each item record owner crate, caller, stability expectation and the gate that validated it.

The baseline must include IDs/paths, session states, generations, scan/search contracts, image contracts, operation state/plan, file mutation/image/trash/watcher ports and structured errors.

- [ ] **Step 2: Remove accidental public APIs**

Change implementation helpers not consumed across crates to `pub(crate)` or private. Run `cargo doc` again and verify the public surface matches the baseline exactly.

- [ ] **Step 3: Reconcile the architecture spec**

Update the system architecture only where G1–G3 produced evidence-backed changes. Each change links its ADR. Preserve product behavior unless the user has explicitly approved a product change.

- [ ] **Step 4: Mark ADR 0004 accepted and commit**

Set ADR 0004 to `Accepted` only when Tasks 1–4 pass. Record the selected image backend, file recovery protocol, search parameters, dependency locks and security boundary.

```bash
git add docs/architecture docs/superpowers/specs/2026-07-16-viewer-system-architecture-design.md docs/adr/0004-viewer-0.1-architecture-freeze.md
git commit -m "docs: freeze Viewer 0.1 architecture"
```

### Task 5: Create the milestone scope matrix and planning handoff

**Files:**
- Create: `docs/milestones/viewer-0.1-scope-matrix.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md`

**Interfaces:**
- Produces: complete mapping from product requirements to M1–M4 and the next planning entry point.

- [ ] **Step 1: Build a requirement-to-milestone matrix**

Create one row for every confirmed requirement in `docs/PRODUCT_SPEC.md` with columns: requirement ID, behavior summary, owning module, milestone, required tests, performance/security acceptance and prerequisite gate. No row may have an empty milestone or test column.

- [ ] **Step 2: Validate coverage mechanically**

Add stable requirement IDs to product-spec headings/items that lack them, then write a script that compares those IDs with the matrix and fails on missing or duplicate IDs.

- [ ] **Step 3: Update the roadmap with frozen outcomes**

Replace gate descriptions with links to accepted ADRs and mark G1–G4 complete. The next executable planning action is an M1 Browsing Core plan created with `superpowers:writing-plans`; M2–M4 plans are created only after the preceding milestone passes its internal acceptance checks.

- [ ] **Step 4: Verify and commit**

Run the scope coverage script and `./scripts/run-architecture-gates.sh`. Expected: both exit 0.

```bash
git add docs/milestones docs/PRODUCT_SPEC.md docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md scripts
git commit -m "docs: map Viewer 0.1 milestone scope"
```

## G4 Completion Check

Run:

```bash
./scripts/check-locked-dependencies.sh
./scripts/check-tauri-security.sh
./scripts/run-architecture-gates.sh
pnpm tauri build --target aarch64-apple-darwin
git status --short
```

Expected:

- All checks exit 0.
- `.app` and DMG are arm64 and declare macOS 13.0 minimum.
- ADR 0004 status is `Accepted`.
- Architecture and API baseline match implemented prototypes.
- Scope matrix covers every Viewer 0.1 requirement.
- Working tree is clean.
