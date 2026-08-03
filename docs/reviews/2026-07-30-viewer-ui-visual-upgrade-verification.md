# Viewer UI Visual Upgrade Verification

## Current evidence authority

- Visual specification: `docs/superpowers/specs/2026-07-30-viewer-complete-ui-visual-upgrade-design.md`
- Migration design: `docs/superpowers/specs/2026-08-02-viewer-atlas-to-product-complete-migration-design.md`
- Non-omission audit: `docs/reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md`
- Migration ledger: `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`
- Current product commit: `6f2d7b9bdc8f67d06566e249be1a54be0c2969e3`
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
| `pnpm test:policy` | 0 | 28 policy tests passed; 47 scope requirements mapped exactly once. |
| `pnpm security` | 0 | 8 security-boundary tests passed; Cargo bans/licenses/sources and npm license policy passed. Duplicate-crate output is warning-only. |
| `cargo fmt --check` / `cargo clippy --locked --workspace --all-targets -- -D warnings` / `cargo test --locked --workspace` | 0 | Rust formatting, warning-free linting, complete workspace unit/integration coverage and doc tests passed. |

## Current native acceptance

The current native target is the bare development executable launched only through `pnpm start:viewer`; no bundle is a valid development acceptance target. Each accepted screenshot is paired with its exact atlas state in one combined image before judgment.

| State | Current combined comparison | Verdict |
| --- | --- | --- |
| `LAU-01` — no project | current joint comparison pending | The first exact 1024 joint image found that the product body copy inherited `16 px` and the atlas retained old `22 / 12 / 32 px` entry rules. Product and atlas now share the approved `20 / 13 / 36 px` title/body/primary-action scale under focused tests; both exact viewport joint images must be recaptured before pass. |

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

The current Computer Use application selector resolves the registered `com.viewer.desktop` bundle, not the canonical bare `viewer-desktop` development process. Consequently, prior radial accessibility trees and null screenshots are not valid current-product visual evidence. Shell window capture can capture the correct bare process, but it cannot open the interaction state.

The radial rows therefore remain pending until a supported state-seeding or UI-control route can open the menu in the exact bare process; static component tests and atlas states do not substitute for that native comparison.

## Completion status

- Formal product code now covers the 17 audited groups and the 89-state ledger remains one-to-one guarded by tests.
- The complete automated gate passes on the current product commit.
- No state is currently closed: the only current exact joint candidate correctly failed the comparison and triggered the entry typography correction.
- The final Task 15 gate is **not complete**: every row still needs current-commit 1024×720 and 1440×900 native evidence before closure.
- No row is promoted to final pass from historical, browser-only, stretched, or accessibility-tree-only evidence.

The next acceptance work is exact 1024×720 capture for `LAU-01`, followed by a supported state-seeding/control route for the project-open and radial-menu states in the canonical bare development process.
