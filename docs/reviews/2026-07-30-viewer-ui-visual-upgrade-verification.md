# Viewer UI Visual Upgrade Verification

## Current evidence authority

- Visual specification: `docs/superpowers/specs/2026-07-30-viewer-complete-ui-visual-upgrade-design.md`
- Migration design: `docs/superpowers/specs/2026-08-02-viewer-atlas-to-product-complete-migration-design.md`
- Non-omission audit: `docs/reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md`
- Migration ledger: `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`
- Current product commit: `676290fe200e353c3074da3209ce61d99ed5a565`
- Branch: `codex/viewer-atlas-product-migration`
- Platform: macOS `26.5.2` (`25F84`), built-in `2560 × 1664` Retina display

Historical screenshots remain useful for diagnosis, but only evidence indexed for the current product commit can close the Task 15 visual gate.

## Automated gate

The complete gate was run on the current product commit after the final preview and dialog corrections.

| Command | Exit | Evidence |
| --- | ---: | --- |
| `pnpm --dir ui check` | 0 | Biome and TypeScript passed across 168 files; one existing Biome configuration deprecation notice remains informational. |
| `pnpm --dir ui test` | 0 | 67 test files; 648 passed and 1 skipped. |
| `pnpm --dir ui build` | 0 | TypeScript and Vite production build passed. |
| `pnpm test:policy` | 0 | 28 policy tests passed; 47 scope requirements mapped exactly once. |
| `pnpm security` | 0 | 8 security-boundary tests passed; Cargo bans/licenses/sources and npm license policy passed. Duplicate-crate output is warning-only. |

## Current native acceptance

The current 1024×720 bundle is running as the only Viewer process from this worktree. Each accepted screenshot below was paired with its exact atlas state in one combined image before judgment.

| State | Current combined comparison | Verdict |
| --- | --- | --- |
| `PRE-01` — image preview, fit | `target/atlas-product-migration-acceptance/676290fe200e353c3074da3209ce61d99ed5a565/1024x720/PRE-01/combined.png` | 1024 pass. The 52 px three-part toolbar, visible filename/dimensions/file size, segmented transform controls, large fitted image stage, light surface, shadow, and floating navigation align with the atlas. Real fixture metadata and source dimensions intentionally differ from the atlas fixture. |
| `DIA-02` — single rename | `target/atlas-product-migration-acceptance/676290fe200e353c3074da3209ce61d99ed5a565/1024x720/DIA-02/combined.png` | 1024 pass. The ordinary dialog now uses the atlas 430 px width, 14 px radius, restrained shadow, field hierarchy, cancel-first footer, and primary rename action. The real extension-edit option is retained because it is an existing product capability. |

The same audit run also exposed and corrected three real product mismatches before these captures:

1. Ordinary rename and close dialogs inherited the 760 px complex-flow shell.
2. Image metadata was hidden at 1024 px by a breakpoint intended for genuinely narrow layouts.
3. `适应窗口` did not upscale a smaller safe preview representation to use the available stage.

Focused red/green coverage was added for all three corrections before the complete automated gate was rerun.

## Native evidence blockers

### Exact 1440×900 capture

The required 1440×900 native window cannot be produced on the current built-in display. A 1440×900 Tauri bundle was built and launched, but macOS clamped the captured application window to 1291×768. The unscaled constraint capture is:

`target/atlas-product-migration-acceptance/7b39607b995330601cbc0904acb1df441de1a441/actual-1291x768/LAU-01/native.jpg`

Resizing or stretching that image would fabricate evidence, so every ledger row still requiring native 1440×900 remains `pending` or `blocked`.

### Native radial-menu screenshot

The native accessibility tree confirms the visible radial commands and states, including Preview, Mark, Organize, Trash, disabled Compare with its reason, and Info. However, the current Computer Use capture surface returns a null screenshot while the native menu accessibility role is open. Existing diagnostic evidence is retained at:

- `target/atlas-product-migration-acceptance/7b39607b995330601cbc0904acb1df441de1a441/1024x720/RAD-01/native-accessibility-tree.txt`
- `target/atlas-product-migration-acceptance/7b39607b995330601cbc0904acb1df441de1a441/1024x720/RAD-04/native-accessibility-tree.txt`

Accessibility text is not a substitute for the required visual comparison, so the radial visual rows are not marked pass.

## Completion status

- Formal product code now covers the 17 audited groups and the 89-state ledger remains one-to-one guarded by tests.
- The complete automated gate passes on the current product commit.
- Two high-risk states have current exact 1024×720 joint visual evidence after the latest fixes.
- The final Task 15 gate is **not complete**: the full 89-state current-commit native matrix and every exact 1440×900 comparison are still required.
- No row is promoted to final pass from historical, browser-only, stretched, or accessibility-tree-only evidence.

The correct next acceptance environment is an external or virtual display that can expose a true 1440×900 application viewport, plus a native capture path that preserves the open radial-menu surface.
