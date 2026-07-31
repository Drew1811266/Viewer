# Viewer UI Visual Upgrade Verification

- Specification: `docs/superpowers/specs/2026-07-30-viewer-complete-ui-visual-upgrade-design.md`
- Platform: macOS development build
- Implementation commit: `c4b6fc7`
- Verification status: Passed

## Automated verification

| Command | Result |
| --- | --- |
| `pnpm --dir ui check` | Passed; one existing Biome deprecation information notice only. |
| `pnpm --dir ui test` | Passed; 60 files, 558 tests passed, 1 skipped. |
| `pnpm --dir ui build` | Passed. |
| `pnpm verify` | Passed: policy, clean-worktree contract, UI checks/tests/build, Rust format, Clippy, Rust unit/integration tests and security verification. |
| `pnpm tauri build --debug --bundles app` | Passed; ad-hoc signed local `Viewer.app` created only for native QA, not installed or deployed. |

The Task 10 obsolete-selector scan has no matches for
`prefers-color-scheme: dark`, `content-toolbar`, `content-view-menu`,
`settings-trigger`, `project-menu`, `SelectAllChoicePanel`, `#f3f4f6`,
`#2563eb`, or `#1d4ed8`. The shadow review found only tokenized floating,
preview, focus, selected and inspector uses plus explicit reset declarations.

## Visual and interaction evidence

Approved source captures for all 11 boards exist at 1440 × 900 and 1024 × 720
under `target/visual-qa/reference-*.png`. The locally bundled Viewer was
opened on `tests/fixtures/images` and captured in default, filter, search,
info, image-preview, unsupported-file, radial-menu and settings-dialog states.
The inspection inputs include the normalized no-project pair plus the four
main/search/info/preview contact sheets in `target/visual-qa/contact-*.png`.

The source documentation canvas and mock-window chrome were excluded before
judging the actual app regions. The same-state no-project pair is normalized to
1280 × 720. Native Computer Use supplied a 1229 × 768 Viewer window and cannot
programmatically resize it; compact layout behavior at 1024 × 720 is covered
by automated UI tests. This environment limitation was not treated as a
product defect.

The required fidelity surfaces were reviewed: typography, spacing/layout,
light color tokens, actual image/asset treatment, and Chinese product copy.
No P0/P1/P2 difference remains. Detailed evidence, state notes and iteration
history are in `design-qa.md`.

## Accessibility and state matrix

Native QA exercised Meta+F, Meta+I, Escape/focus restoration for filter/view/
more controls, task failure treatment, expanded other-file panel, image
preview, inspector, unsupported JSON recovery, search results/pagination,
radial action ring and settings dialog. An ignored extended QA project adds
Markdown, TXT and 20 comparison candidates without changing the tracked
fixture. The UI suite additionally covers text and Markdown previews, read-only behavior, comparison, loading/recovery,
keyboard menu navigation, focus visibility and reduced-motion semantics.

Computer Use's secondary-click injection opens a WebKit text menu before the
DOM event can be observed. The Viewer radial action ring is visible after that
overlay is dismissed; target-level capture-phase default prevention is covered
by a regression test for both image and text rows. This is recorded as an
automation limitation rather than an unresolved product defect.

## Fixes verified during final QA

- `6836bb3`: replaced the no-project browser-default action with the approved
  semantic primary-action treatment.
- `fad7a98`: added controlled filter Escape close/focus restoration.
- `c4b6fc7`: prevents native context-menu default during capture for image and
  text file targets, with a regression test.

No installation, deployment, or user project content modification occurred.
