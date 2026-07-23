# G4 Architecture Freeze Stage Review

> **Historical governance note (2026-07-23):** References to M4 or Viewer 0.1 release acceptance below describe the plan at the time this evidence was recorded. ADR 0005 cancelled M4 and replaced release acceptance with continuous development governance. The measured evidence in this document is unchanged.

- Status: Passed
- Date: 2026-07-16
- Base: `0ba17dd` (accepted G3 on `main`)
- Reviewed implementation head: `784dd41`
- Review method: G4 plan traceability, full `0ba17dd..784dd41` diff inspection, negative security tests, dependency/license audits, aggregate architecture gates and Apple Silicon artifact inspection
- Decision: **Approved for local fast-forward merge to `main`**

## Exit-criteria traceability

| G4 exit criterion | Evidence | Result |
| --- | --- | --- |
| G1–G3 evidence remains reproducible | One aggregate script reruns all three accepted gates and requires ADR 0001–0003 | Pass |
| Dependencies and project license are frozen | Apache-2.0 text/hash and package SPDX checks; locked Cargo/pnpm resolution; direct version/license/purpose/upstream/distribution inventory | Pass |
| Rust and frontend dependency policies are executable | `cargo-deny` checks advisories, bans, licenses and sources; npm rejects every expression outside the reviewed allowlist and audits high-severity vulnerabilities | Pass |
| Tauri security boundary is minimal | Window-scoped empty capability, exact CSP/connect/image allowlists, local navigation guard, session/token image protocol and Rust-side Markdown sanitizer | Pass |
| Domain/Application surface is frozen | Generated docs, strict Clippy and the explicit cross-crate inventory agree; duplicate prototype-only APIs were removed | Pass |
| Product scope has no silent omissions | All 47 stable Product Spec requirement IDs occur exactly once in the M1–M4 matrix with owner, tests, acceptance and prerequisite gates | Pass |
| Apple Silicon distribution is reproducible | `pnpm build:macos` produces arm64 `.app`/DMG with `LC_BUILD_VERSION minos 13.0`; Info.plist and DMG integrity checks pass | Pass |

## Review findings

No Critical or Important findings remain open.

The full-diff review found one Important governance gap: `pnpm audit` checks vulnerabilities but not frontend licenses, so the repository did not yet enforce the Product Spec's automatic npm license scan. A failing test first reproduced the missing policy. Commit `784dd41` added a `pnpm licenses list --json` validator, rejects GPL/unknown/unlicensed or any other unreviewed expression, tests both allowed and forbidden cases, and wires the check into the immutable dependency gate. The current Apple Silicon install contains 118 npm packages across 10 reviewed expressions.

The review also corrected one documentation mismatch: `file-id` had remained in the foundations table as if it were a direct dependency even though G3 selected native Unix device/inode metadata and only receives `file-id` transitively. The frozen dependency inventory now matches the manifests and implemented adapter.

Release verification initially exposed an environment-specific packaging failure after the `.app` succeeded: Tauri's Finder AppleScript waited indefinitely while prettifying the DMG in the non-interactive Codex terminal. Process sampling isolated the failure to that AppleEvent path. Commit `1dc2a32` standardized Tauri's supported `CI=true`/`--skip-jenkins` path as `pnpm build:macos`, added a regression policy test, and changed the invalid `.app`-suffixed bundle identifier to `com.viewer.desktop`. Two subsequent builds produced both artifacts without warnings.

## Final verification evidence

```text
./scripts/check-locked-dependencies.sh  PASS; both lockfile hashes unchanged
  cargo advisories/bans/licenses/sources PASS
  npm license graph                    118 packages / 10 reviewed expressions
  pnpm audit                           no known vulnerabilities
./scripts/check-tauri-security.sh       PASS; 6/6 negative boundary tests
node scripts/check-scope-coverage.mjs   PASS; 47/47 requirements exactly once
./scripts/run-architecture-gates.sh     PASS
pnpm build:macos                        PASS; Viewer.app + Viewer_0.1.0_aarch64.dmg
lipo -archs viewer-desktop              arm64
otool LC_BUILD_VERSION                  minos 13.0
hdiutil verify Viewer_0.1.0_aarch64.dmg VALID
git diff --check                        PASS
```

The latest aggregate G1 run measured 112.4 MB peak RSS. G3 retained 20.08 ms first-folder p95, 39.17 ms base-scan p95, 0.86 ms search p95 and 6.67 MB peak RSS on its deterministic corpus.

## Accepted limitations

- G4 validates the architecture and prototype corpora, not the user's eventual approximately 10 MB real images. M4 must repeat integrated browse/thumbnail/preview/scroll acceptance on the user-provided project folder.
- The real macOS Trash smoke test remains explicit opt-in and was not invoked during unattended G4 verification; G2's fake-port, transaction and recovery contracts remain accepted evidence.
- The internal build is unsigned and unnotarized because no Developer ID credentials were provided. M4 performs signing/notarization only when credentials are available.
- Non-interactive DMG creation skips custom Finder icon positioning/background. The image still contains `Viewer.app` and the Applications link and passes integrity checks; visual DMG polish is not a Viewer 0.1 architecture requirement.
- `cargo-deny` reports informational duplicate-version groups inherited mainly through Tauri and one currently unmatched Cargo ISC allowance. Advisories, license compatibility, banned sources and npm license policy all pass.
- G4 adds no final browsing UI or product command wiring. Those 47 behaviors remain assigned to M1–M4 rather than being misrepresented as complete.

## Decision

G4 meets every roadmap exit criterion. ADR 0004 may be accepted, the branch may be fast-forward merged to `main`, and M1 planning may begin only after the merged main worktree passes verification.
