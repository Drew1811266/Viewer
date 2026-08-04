# Viewer UI Visual Upgrade Verification

## Current evidence authority

- Visual specification: `docs/superpowers/specs/2026-07-30-viewer-complete-ui-visual-upgrade-design.md`
- Migration design: `docs/superpowers/specs/2026-08-02-viewer-atlas-to-product-complete-migration-design.md`
- Non-omission audit: `docs/reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md`
- Migration ledger: `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`
- Native controller design: `docs/superpowers/specs/2026-08-03-viewer-native-acceptance-controller-design.md`
- Native controller plan: `docs/superpowers/plans/2026-08-03-viewer-native-acceptance-controller-plan.md`
- Current product commit: `e69a85a7713da97871164aff997657ecb183e33b`
- Branch: `codex/viewer-atlas-product-migration`
- Platform: macOS `26.5.2` (`25F84`), built-in Retina display

Historical screenshots remain useful for diagnosis, but only evidence indexed for the current product commit can close the Task 15 visual gate.

## Automated gate

The complete gate was run on the current product commit after adding a canonical exact-viewport launch path.

| Command | Exit | Evidence |
| --- | ---: | --- |
| `pnpm --dir ui check` | 0 | Biome and TypeScript passed across 168 files; one existing Biome configuration deprecation notice remains informational. |
| `pnpm --dir ui test` | 0 | 67 test files; 649 passed and 1 skipped. |
| `pnpm --dir ui build` | 0 | TypeScript and Vite production build passed. |
| `node --test scripts/viewer-dev-launcher.test.mjs` | 0 | 20 launcher tests passed, including exact acceptance-config forwarding and one-process safeguards. |
| `pnpm test:native-acceptance` | 0 | 52 PID/process/window/path/protocol/fixture/manifest/PNG/non-shipping controller tests passed. |
| `pnpm test:policy` | 0 | 28 policy tests passed; 47 scope requirements mapped exactly once. |
| `pnpm security` | 0 | 8 security-boundary tests passed; Cargo bans/licenses/sources and npm license policy passed. Duplicate-crate output is warning-only. |
| `cargo fmt --check` / `cargo clippy --locked --workspace --all-targets -- -D warnings` / `cargo test --locked --workspace` | 0 | Rust formatting, warning-free linting, complete workspace unit/integration coverage and doc tests passed. |

## Current native acceptance

The current native target is the bare development executable launched only through `pnpm start:viewer`; no bundle is a valid development acceptance target. Each accepted screenshot is paired with its exact atlas state in one combined image before judgment.

| State | Current combined comparison | Verdict |
| --- | --- | --- |
| `LAU-01` — no project | `target/atlas-product-migration-acceptance/e69a85a7713da97871164aff997657ecb183e33b/1024x720/LAU-01/combined.png`; `target/atlas-product-migration-acceptance/e69a85a7713da97871164aff997657ecb183e33b/1440x900/LAU-01/combined.png` | **Pass.** Product and atlas share the approved `20 / 13 / 36 px` title/body/primary-action scale and 12 px vertical rhythm. At both exact viewports, the only remaining visible difference is the allowed macOS system title bar and its corresponding content-area centering offset; P0/P1/P2 are zero. |
| `LAU-01` — PID-controller smoke | `target/atlas-product-migration-acceptance/da1c9f833477a45cdd93bb11cdc22c89f54b84de/1024x720/LAU-01/combined.png` | **Pass.** Exact bare PID `4859`, window `194`, `1024 × 720`, clean controller commit and file hashes are recorded in `manifest.json`; the same-state combined review has P0/P1/P2 zero. This smoke proves the controller path and does not substitute for a missing second viewport on another row. |

The prior `PRE-01` and `DIA-02` 1024 comparisons remain useful ancestor-commit diagnostics, but they are not current-commit closure evidence.

The same audit run also exposed and corrected three real product mismatches before these captures:

1. Ordinary rename and close dialogs inherited the 760 px complex-flow shell.
2. Image metadata was hidden at 1024 px by a breakpoint intended for genuinely narrow layouts.
3. `适应窗口` did not upscale a smaller safe preview representation to use the available stage.

Focused red/green coverage was added for all three corrections before the complete automated gate was rerun.

## Native evidence status

### Exact 1440×900 capture

Resolved. `VIEWER_TAURI_CONFIG` is forwarded by the canonical launcher to `pnpm tauri dev`, while the launcher still rejects duplicate, bundle and wrong-worktree Viewer processes. The authoritative capture used the worktree's bare `target/debug/viewer-desktop` process at logical `1440 × 900`; the uncropped Retina source is `2880 × 1800` and is normalized by an exact 2× factor.

The previous `1291 × 768` capture came from selecting a registered bundle rather than the canonical development process. It is historical diagnostic evidence, not a display constraint and not an acceptance input.

This removes the environment-wide 1440 blocker; it does not automatically pass any uncaptured state.

### Native radial-menu screenshot

Resolved as a tooling precondition, not yet as row evidence. The approved non-shipping controller binds the exact bare PID and owned window, drives real Accessibility/CoreGraphics input, and captures that window with ScreenCaptureKit. Its 1024 LAU-01 smoke proves the identity, action, capture and hash chain. `RAD-01`–`RAD-07` remain pending until their declared real radial interactions and both joint comparisons are actually recorded; static component tests still do not substitute for native comparison.

## Completion status

- Formal product code now covers the 17 audited groups and the 89-state ledger remains one-to-one guarded by tests.
- The complete automated gate passes on the current product commit.
- One of 89 states is closed: `LAU-01` passed current-commit joint comparison at both exact viewports after the type-scale and vertical-rhythm corrections.
- The final Task 15 gate is **not complete**: the remaining 88 rows still need current-commit 1024×720 and 1440×900 native evidence before closure.
- No row is promoted to final pass from historical, browser-only, stretched, or accessibility-tree-only evidence.

The next acceptance work is executing the declared state recipes through the verified PID-bound controller at both exact viewports, beginning with Wave 1 and stopping for product fixes whenever a combined review exposes a P0/P1/P2 difference. Static component or browser-only states will not be promoted as native evidence.
