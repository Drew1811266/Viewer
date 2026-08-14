# Viewer Final UI Visual Acceptance

Date: 2026-08-05

Prototype: `docs/prototypes/viewer-complete-ui-visual-atlas.html`

Result: **Accepted — formal product migration and macOS tiered visual verification complete**

## Current product acceptance

- Formal product evidence commit: `9114b78515baa069914a6f6dc083a05a9982e9cf`.
- Complete browser comparison: 89 states at `1024 × 720` and `1440 × 900`, 178/178 combined reference/product images passed, with zero console or page errors.
- Browser evidence: `target/viewer-visual-acceptance/final-all-9114b78/9114b78515baa069914a6f6dc083a05a9982e9cf/`.
- Native acceptance controller commit: `7fa4005f2d9baab6805264ecbef5d51a0b429de3`.
- macOS native representative journeys: 15/15 passed at each target size, 30/30 total.
- Native evidence: `target/atlas-product-migration-acceptance/7fa4005f2d9baab6805264ecbef5d51a0b429de3/`.
- Final visual severity: P0 `0`, P1 `0`, P2 `0`.

The browser layer exhaustively verifies the visual state matrix; the native layer verifies representative window, focus, input, popover, radial-menu and screenshot paths. The historical Figma board below remains the approved atlas handoff, while the current product evidence above is the final implementation authority.

## Historical atlas acceptance scope

This pass verifies the highest-priority visual-atlas corrections against the
approved Viewer visual-upgrade specification. The accepted screenshots were
captured after the corrections and are stored in:

`target/final-design-acceptance-2026-07-31/`

The compact captures use a 1024×720 Viewer viewport. The wide captures use the
atlas's 1440×900 Viewer viewport, fitted inside a 1280×720 outer browser
capture.

## Corrected findings

| Step | Surface | Evidence | Health | Acceptance note |
| --- | --- | --- | --- | --- |
| 01 | Collapsed sidebar | `01-collapsed-sidebar-1024x720.jpg` | Pass | The 52 px rail now exposes one centered, accessible expand control; project text no longer collides with it. |
| 02 | Advanced filter | `02-advanced-filter-1024x720.jpg` | Pass | Trigger and panel both derive `6` from the same active-condition model; the panel opens in place. |
| 03 | Read-only menu | `03-readonly-menu-1024x720.jpg` | Pass | Unavailable actions are visibly muted, carry a read-only label, and use disabled semantics. |
| 04 | Radial click | `04-radial-click-1024x720.jpg` | Pass | The primary radial menu is visible in the Viewer shell instead of a rectangular substitute. |
| 05 | Radial mark | `05-radial-mark-1024x720.jpg` | Pass | The mark secondary fan uses the approved radial geometry and remains attached to its primary sector. |
| 06 | Radial organize | `06-radial-organize-1024x720.jpg` | Pass | The organize secondary fan is visually distinct and preserves the same center and sector rhythm. |
| 07 | Radial disabled | `07-radial-disabled-1024x720.jpg` | Pass | Dashed boundaries, reduced emphasis, and the unavailable label make the state unambiguous. |
| 08 | Radial read-only | `08-radial-readonly-1024x720.jpg` | Pass | The center and affected actions clearly communicate read-only mode. |
| 09 | Radial keyboard | `09-radial-keyboard-1024x720.jpg` | Pass | Keyboard focus is visible on a sector without being confused with selection. |
| 10 | Keyboard selection | `10-keyboard-selection-1024x720.jpg` | Pass | Enter/Space selection is visible on the image card and reflected by `aria-selected`. |
| 11 | Scanning progress | `11-scanning-progress-1024x720.jpg` | Pass | Status and progress are visible and expose live-region/progressbar semantics. |
| 12 | Batch dialog | `12-batch-dialog-1024x720.jpg` | Pass | The modal scrim covers the entire Viewer shell, including toolbar and sidebar; dialog labelling is explicit. |
| 13 | Forced-colors | `13-forced-colors-1024x720.jpg` | Pass | System color tokens and explicit boundaries preserve hierarchy without relying on shadows. |
| 14 | 200% reflow | `14-zoom-200-1024x720.jpg` | Pass | The real Viewer shell reflows at 200%; the primary action remains reachable. |
| 15 | Wide main atlas | `wide-00-main-atlas-1280x720.jpg` | Pass | The atlas state switcher occupies reserved space and no longer overlays product controls. |
| 16 | Wide browser | `wide-01-browser-1280x720.jpg` | Pass | Main navigation, content rail, and image grid remain aligned at the 1440×900 Viewer viewport. |
| 17 | Wide filter | `wide-02-advanced-filter-1280x720.jpg` | Pass | The filter popover stays anchored and unclipped in the wide shell. |
| 18 | Wide radial mark | `wide-03-radial-mark-1280x720.jpg` | Pass | Primary and secondary radial geometry stays centered and legible at the wide viewport. |
| 19 | Wide preview | `wide-04-preview-zoom-1280x720.jpg` | Pass | Preview content, chrome, and zoom controls retain clear separation. |
| 20 | Wide dialog | `wide-05-batch-dialog-1280x720.jpg` | Pass | Modal scale, focus hierarchy, and full-shell scrim remain correct at the wide viewport. |
| 21 | Wide results | `wide-06-results-1280x720.jpg` | Pass | Result density and hierarchy remain stable without overlap or clipping. |
| 22 | Wide 200% reflow | `wide-07-zoom-200-1280x720.jpg` | Pass | The accessibility demonstration remains a real reflowed shell rather than an explanatory card. |

## Automated verification

- `pnpm --dir ui check` — pass.
- `pnpm --dir ui test` — 77 files pass; 727 tests pass; 1 expected test skipped.
- `pnpm --dir ui build` — production build pass.
- `pnpm test:visual-acceptance` — 16/16 pass.
- `pnpm test:native-acceptance` — 99/99 pass.
- `git diff --check` — pass.
- Complete browser acceptance — 178/178 combined images, zero console or page errors.
- macOS native acceptance — 30/30 representative journeys pass.

## Visual acceptance conclusion

No blocking overlap, clipping, state ambiguity, or atlas-control obstruction
was found in either the complete product evidence or the accepted historical
atlas board. Filter, radial-menu, read-only, keyboard, progress, dialog,
target-size, inspector-width, and contrast-risk contracts have visual,
automated and representative macOS-native evidence.

This acceptance does not claim full WCAG conformance. Native macOS/Windows
screen-reader output, OS high-contrast rendering, platform font rasterization,
and physical input-device behavior require platform-specific testing in
packaged application builds. Windows-native verification remains a future
platform task because the Windows version is not yet under development.

## Historical Figma atlas audit board

- File:
  [Viewer Final UI Visual Acceptance — 2026-07-31](https://www.figma.com/design/oCtWdesfu5wPx2m9QW1g6Y)
- Section: `Viewer Final UI Visual Acceptance — 2026-07-31`
- Structure: 22 accepted screenshot cards, each with step number, health, name,
  and finding-specific acceptance note.
- Layout verification: 15 cards in row one, 7 cards in row two, 200 px
  horizontal gaps, and a 600 px row gap.
- Visual verification:
  `target/final-design-acceptance-2026-07-31/figma-audit-board.png`
- Structural verification: all 22 cards contain exactly one accepted
  screenshot; the audit section is the only top-level canvas node.

## 2026-08-06 — Square Image Card Selection

### Inputs

- Approved design specification: `docs/superpowers/specs/2026-08-06-viewer-square-image-card-selection-design.md`.
- Authoritative visual source: `docs/prototypes/viewer-complete-ui-visual-atlas.html` at SHA-256 `723c542cff9d32ab63de571342e8abbbdf141dd491a3c56ba284dc313bab7ab5`.
- Supplemental user target: `/var/folders/hh/jj77kbxs0db1_j1kgh7hbd2c0000gn/T/codex-clipboard-d5bd1d52-517a-4846-900b-bf4128731924.png`; its red annotation defines the required complete-card boundary but is not used as a pixel-identical fixture.
- Implementation commit: `7ebf8496dea25ca8d104f634b322c2e74df87439` on `codex/square-image-card-selection`, captured from a clean worktree.
- Automated full-view evidence: `target/viewer-visual-acceptance/square-image-card-selection/7ebf8496dea25ca8d104f634b322c2e74df87439/{1024x720,1440x900}/{THU-04,THU-05,THU-06,THU-07}/combined.png`.
- Native full-view evidence: `target/atlas-product-migration-acceptance/7ebf8496dea25ca8d104f634b322c2e74df87439/1024x720/THU-05/combined.png`.
- Native focused-region evidence: `target/atlas-product-migration-acceptance/7ebf8496dea25ca8d104f634b322c2e74df87439/1024x720/THU-05/combined-focus.png`.

### Capture normalization

- Browser source and implementation captures use identical CSS viewports and 1× density: `1024 × 720` produces `1024 × 720` source/product images and a `2048 × 720` combined image; `1440 × 900` produces `1440 × 900` source/product images and a `2880 × 900` combined image.
- Native `native@2x.png` is `2048 × 1440` Retina output. The acceptance controller normalizes it to `1024 × 720` before combining it with the `1024 × 720` atlas reference, so geometry is judged at the same CSS size rather than by raw device pixels.
- The native `1440 × 900` window was not forced because the current display's safe application area caps it at `1440 × 847`. The accepted native state therefore uses the exact supported `1024 × 720` viewport without changing macOS display scaling, Dock settings, or global scrollbar settings; both approved sizes remain covered by browser component evidence.

### State and fidelity review

| State | Evidence | Result |
| --- | --- | --- |
| `THU-04` resting cards | Both browser viewports | Square card edges; no selection overlay or geometry shift. |
| `THU-05` single selection | Both browser viewports plus native `1024 × 720` | One 2 px accent boundary encloses the thumbnail and filename; no thumbnail-only ring remains. |
| `THU-06` multiple selection | Both browser viewports | Exactly one complete-card boundary appears on each of three selected cards. |
| `THU-07` keyboard focus | Both browser viewports | The complete-card selection boundary remains independent from the outer keyboard-focus treatment. |

- Typography: font family, size, weight, filename baseline, and selection-summary hierarchy remain unchanged; minor native rasterization differences are platform rendering, not token drift.
- Spacing and layout rhythm: moving the overlay does not change card measurement, image-stage height, filename row, grid gaps, or the selection-summary position.
- Color and tokens: the boundary uses `--viewer-accent`; the atlas uses its matching accent token. Forced-colors coverage moves to the same complete-card pseudo-element and uses `Highlight` at 2 px.
- Image quality and cropping: thumbnail assets retain `object-fit: contain`; no new crop, blur, scaling, or loading artifact is visible.
- Copy and content: source and native fixtures intentionally use different filenames and counts, while label hierarchy and the `已选择 1 项` guidance remain consistent.
- Interaction and layering: the organization handle stays above the non-interactive overlay, pointer events pass through the overlay, and focus remains separately visible.
- Scope protection: folder filmstrips, search-result rows, other-file rows, and non-image card families retain their existing geometry.

### Checklist

- [x] Resting content-grid cards have square corners.
- [x] The selection boundary encloses thumbnail and filename.
- [x] No inset thumbnail-only boundary remains.
- [x] Selection does not move card content or grid geometry.
- [x] Multi-selection draws one boundary per card.
- [x] Keyboard focus remains visually independent.
- [x] The organization handle remains above and operable.
- [x] Forced-colors styling follows the complete card.
- [x] Folder filmstrips and other card families are unchanged.

### Findings and comparison history

- Initial product finding: rounded image cards and a selection ring limited to the thumbnail stage did not match the approved complete-card intent.
- Implemented correction: the card radius is `0`, selection ownership remains on `aria-selected`, and the non-measuring overlay moved to `.image-cell::after` with `inset: 0`.
- Evidence correction during QA: the first `1024 × 720` reference export reused `THU-04` while changing URL fragments. Those stale images were rejected, each reference was reloaded in a fresh document, selected counts were asserted as `0/1/3/1`, and all eight combined states plus the native state were recaptured.
- Final severity: P0 `0`, P1 `0`, P2 `0`, P3 `0`.

Final result: passed

## 2026-08-13 — Responsive Video Player Experience Redesign

### Source truth and capture normalization

- Approved Figma board: [Viewer Video Player Redesign](https://www.figma.com/design/gWuePv7qwGnu32aRryxdyg/Untitled?node-id=9-2), especially `10:11` (wide player) and `11:17` (compact player).
- Figma source crops: `target/design-qa-video-player-redesign/figma-wide-source.png` (`1286 × 793`, SHA-256 `54f2537cc8030d190938b0273973f3923e372896b8fb173ed5ff45812ae65304`) and `target/design-qa-video-player-redesign/figma-compact-source.png` (`607 × 556`, SHA-256 `8c54afbbbf1eb93bc046bd042afbee1e78ae2ff11d3a58874175ae91f4f5a861`).
- Product captures: `target/viewer-visual-acceptance/video-player-redesign/cdd43657554d26da1ffcb5837c3dd5052be648ee/1440x900/video-playing-controls/product.png` (`1440 × 900`, SHA-256 `62f8bd0a849306d1ee8d4e9021afe2020d4eb6dbcfa8fa10774054ed03d56c9c`) and `target/viewer-visual-acceptance/video-player-redesign-720/cdd43657554d26da1ffcb5837c3dd5052be648ee/720x720/video-playing-controls/product.png` (`720 × 720`, SHA-256 `1ba577e85507815dc2fdc95588042d9ca707d6b3ce7984b1c1b465a80a046ca1`).
- Same-input comparison images: `target/design-qa-video-player-redesign/compare-wide.png` (SHA-256 `e5de4040ebdf1703c7eb74849e15498bc9bfad02716e3d3875b9bf95c1e7ddb7`) and `target/design-qa-video-player-redesign/compare-compact.png` (SHA-256 `86372ec6e78c3a8ead9e549aa4659ae0e60046272247aee31c9e5a0b70b05443`).
- Product screenshots use 1× CSS-pixel density. The Figma board crops preserve the design's natural aspect ratio and are compared as complete player compositions rather than as pixel-identical media content.
- Reviewed state: active playback, fitted first frame visible, controls revealed, two-video navigation, elapsed/total time, responsive settings, and Done.

### Fidelity and behavior review

| Surface | Result | Acceptance note |
| --- | --- | --- |
| Layout hierarchy | Pass | Video remains the dominant surface; metadata, Done, page navigation, timeline, transport, and settings form three localized layers instead of full-width opaque bands. |
| Wide controls | Pass | At `1440 × 900`, frame stepping, play/pause, volume, rate, and fullscreen remain inline with one timeline and one accessible instance of every control. |
| Narrow controls | Pass | At `720 × 720`, the dock is `688 px` wide inside `16 px` gutters with no horizontal overflow; frame step, volume, and rate move behind one More trigger. |
| Geometry | Pass | The React stage remains the single clipped container and retains the existing native `matteInsets`; CSS does not create a second media rectangle or crop the video. |
| First-frame transition | Pass | Ambient blue-gray matte and letterbox space remain stable until the active generation reveals the native first frame; the `120 ms` veil is removed under reduced motion. |
| Control priority | Pass | Play/pause is the only accent-filled primary action; secondary icon buttons retain visible boundaries, hover/focus states, and `44 × 44` minimum targets. |
| Idle and focus | Pass | Playing chrome shares one visibility transition; paused, ended, failed, adjusting, and focus-within states remain visible. Fullscreen cursor hiding follows the same idle state. |
| Accessibility | Pass | Chinese accessible names, native range/select controls, focus restoration from More, forced-colors system tokens, reduced motion, and keyboard ownership remain covered. |
| Typography and content | Pass | Viewer typography and dynamic filename/duration remain intact; hierarchy matches the Figma intent without copying its placeholder media or decorative geometry. |
| Icons and imagery | Pass | Existing Lucide registry icons are reused; the product video fixture is intentionally different from the abstract Figma placeholder and keeps `contain` framing. |

### Findings and iteration history

- P1 resolved: independent absolute navigation and control layers could collide or drift outside the clipped stage. They now share `.video-preview-bottom-chrome`, one bottom anchor, one width cap, and one visibility state.
- P1 resolved: narrow layouts compressed every control into one row. One live media-query hook now selects a compact branch and More disclosure without hidden duplicate form controls.
- P1 resolved: the old near-black first-frame transition read as a flash. The stage, loading shell, and letterbox now use one ambient matte with a first-frame veil.
- P2 resolved: the original visual-acceptance fixture supported only `1024 × 720` and `1440 × 900`; forcing a `720 px` browser around the `1024 px` frame falsely reported overflow. `720 × 720` is now an explicit, non-default acceptance viewport, and fresh browser metrics report frame/body/player width `720`, dock width `688`, and horizontal overflow `false` with no console warnings or errors.
- Intentional adaptation: production uses real fitted video imagery, existing Viewer primitives, and the native surface contract rather than Figma's abstract placeholder shapes. Navigation and dock are grouped into the approved shared safe region so they cannot overlap even when the media aspect ratio changes.
- The visual-acceptance runner produced all requested product images. Its comparison wrapper returned nonzero only because no legacy atlas `reference.png` exists for these new video states and the newly added `720 × 720` viewport; the authoritative reference for this redesign is the Figma source above.

### Verification

- RED→GREEN focused contracts covered bottom-chrome containment, ambient reveal, responsive hook updates, compact More behavior, exact control composition, idle/focus restoration, forced colors, reduced motion, and `720 × 720` acceptance parsing.
- Focused player and style suite: 8 files / 48 tests passed before the viewport addition; the viewport parser and CLI regression add 2 focused passing suites.
- Fresh `pnpm verify` passed end to end: policy `30/30`, packaging `30/30`, clean-wrapper `11/11`, UI check, `108` UI files / `959` passed + `1` skipped, Vite build (`190` modules), locked Rust formatting/Clippy/workspace tests, Tauri security boundaries, cargo-deny, npm licenses, and bundled runtime license verification.
- `git diff --check` passed after the visual evidence and documentation index were updated.
- Final visual severity: P0 `0`, P1 `0`, P2 `0`.

final result: passed

## 2026-08-13 — Native Video Letterbox Leak Correction

### Current-run evidence

- User-reported native window: `/var/folders/hh/jj77kbxs0db1_j1kgh7hbd2c0000gn/T/codex-clipboard-d5298069-8ae2-43ee-9ef6-b7c8e5a483d2.png` (`1968 × 1584`).
- Corrected native Viewer window: `target/design-qa-video-matte/runtime-refined.png` (`1848 × 1574`, SHA-256 `1aa1263b6d2297c527ace8f022d0f1535167f0cbb93b6a14dc3b4806ae6e0d3a`).
- Same-viewport comparison: `target/design-qa-video-matte/comparison.png` (`3696 × 1574`, SHA-256 `376236e3fa8313066344e54c6f06634fe1a55d3a7e9c6e573746c81a5a5f2782`). The reported screenshot was normalized to the corrected native capture size before comparison.

### Audit and correction

| Region | Reported defect | Corrected result |
| --- | --- | --- |
| Native letterbox | The Web document became transparent across the whole window while the native video occupied only its fitted center rectangle. The underlying browser screen leaked through as a blue top strip and unrelated imagery below the video. | The fitted native rectangle now publishes exact top/right/bottom/left insets. A solid theater matte covers only the transparent area outside that rectangle, while the center stays transparent for the native video. |
| Metadata | The filename and duration sat inside another dark card over the leaked background, adding unnecessary visual weight. | Passive metadata is flat on the theater matte; the actionable Done button retains a clear boundary. |
| Playback controls | A second large floating card was stacked over the bottom leak, making the lower third appear oversized and detached from the player. | Timeline and controls sit directly on the stable matte. Individual buttons retain high-contrast borders, white icons, and an accent primary action. |
| Video framing | Removing the original gradients exposed implementation details outside the native surface. | The full video remains visible at its original aspect ratio; no crop, stretch, or gradient transition is introduced. |

### Verification

- RED: three focused failures proved the missing matte geometry, missing CSS inset publication, and absent solid matte boundary.
- GREEN: geometry, layout, native-surface shell, controls, timeline, and accessibility suite passed 6 files / 45 tests, including fractional window geometry that cannot emit a negative matte width.
- `pnpm --dir ui check` and `git diff --check` passed; only the existing Biome configuration deprecation information remains.
- The corrected screenshot was captured from the running native Viewer window after the implementation update, not from a browser-only acceptance scene.
- Fresh `pnpm verify` passed end to end: policy 30/30, packaging 30/30, clean verifier 11/11, UI check, 106 UI files / 952 passed + 1 skipped, production build, locked Rust formatting/Clippy/workspace tests, Tauri security boundaries, cargo-deny, npm license policy, and bundled runtime license verification.

Final result: passed

## 2026-08-13 — Video Control Visibility Refinement

### Source and implementation evidence

- User running-product source: `/var/folders/hh/jj77kbxs0db1_j1kgh7hbd2c0000gn/T/codex-clipboard-5fc02ea7-c4d6-4453-9b19-0e88eb3c6240.png` (`2558 × 1600`, SHA-256 `922dd29d60225c8f9e1c131856da4f2ec04ec46ffd02932f8cfc82206f742812`).
- Rendered playing-controls state: `target/viewer-visual-acceptance/video-control-visibility/43b6d30938b9e6844829a2ad45de98846bf7c1d6/1440x900/video-playing-controls/product.png` (`1440 × 900`, SHA-256 `fca77223acd4498e15adab83d0a9aa1b4358acf125a62a062e2547bf0c0cc120`).
- Same-viewport comparison: `target/design-qa-video-control-visibility/comparison.png` (source normalized to `1440 × 900`; comparison SHA-256 `c571ff07ce0ed5f1010d6deb85d8f3d0379e97ac69c49e497c2a4a6982f905b1`).
- The acceptance command generated the complete product screenshot but returned nonzero only because the repository's unrelated historical atlas reference file is absent. The supplied user screenshot is the explicit visual source for this refinement.

### Comparison findings

| Region | Result | Acceptance note |
| --- | --- | --- |
| Video frame | Pass | The full-width top and bottom gradient scrims are absent; frame brightness remains unchanged outside localized controls. |
| Metadata and Done | Pass | Filename, duration, and Done use compact high-contrast capsules instead of a darkened top edge. |
| Page navigation | Pass | The page capsule remains separate; enabled arrows are white with visible button boundaries and disabled arrows remain recognizable. |
| Timeline | Pass | The track, progress, thumb, and elapsed/total time are visibly stronger without shading the surrounding frame. |
| Transport and settings | Pass | The inset rounded dock defines the control region; all external SVG icons render white, and play/pause uses a solid accent surface. |
| Window edge | Pass | The dock remains inset and rounded; it does not form a full-width black band or black transition edge. |

### Correction history

- P1: top and bottom pseudo-element gradients altered the apparent exposure of the video. Correction: both stage scrims are removed from rendering.
- P1: setting CSS text color did not recolor the externally loaded SVG icon images, leaving transport and navigation glyphs dark. Correction: preview icons use an explicit high-contrast filter, with forced-colors mode restoring system rendering.
- P1: secondary buttons had low-opacity surfaces and borders directly over arbitrary footage. Correction: every control group now owns a localized semantic surface, stronger border, hover state, and recognizable disabled state.
- P2: the 4-pixel timeline and 12-pixel thumb were visually weak at the native window size. Correction: the track is 6 pixels, the thumb 16 pixels, and the progress contrast is preserved.
- Intentional difference: the acceptance state uses the existing Viewer product fixture rather than the user's private video frame. Video content differs, while viewport, interaction state, control hierarchy, and contrast behavior are the reviewed targets.

### Focused verification

- RED contract: three expected failures for stage scrims, missing localized surfaces, and dark external icon assets.
- GREEN contract: 5/5 immersive layout tests.
- Focused player, timeline, semantic-color, and accessibility suite: 6 files / 73 tests passed.
- `pnpm --dir ui check` and `git diff --check` passed; only the existing Biome configuration deprecation information remains.
- The first full verification attempt stopped at the documentation policy because the new specification was not yet indexed. After adding the exact Active specification and Historical plan rows, the policy gate passed 30/30 with scope coverage 48/48.
- Fresh final `pnpm verify` passed end to end: UI check, 106 UI files / 949 passed + 1 skipped, production build, locked Rust formatting/Clippy/workspace tests, Tauri security boundaries, cargo-deny, npm license policy, and bundled video-runtime license verification.

Final result: passed

## 2026-08-13 — Immersive Video Playback Interface

### Visual source and capture normalization

- Selected design direction: `/Users/abc/.codex/generated_images/019fdab7-cec2-7cd3-9349-914f807e858f/exec-0918ca01-d216-45dc-a3d3-affc3b299f27.png` (`1586 × 992`, SHA-256 `2e8d491b3e67cc2c8bf7e8b6965cea284db2adb81137fd7902d77df4aae09e14`).
- Rendered product state: `target/viewer-visual-acceptance/video-ui-redesign-4/313bb31fcf38671580496456366b83c42d9719a4/1440x900/video-playing-controls/product.png` (`1440 × 900`, device scale factor `1`, SHA-256 `e8ed88e286fbb5f54725377f48cd3d3b861a5b1ea559775490e70cc69ca6072b`).
- Side-by-side comparison: `target/design-qa-video-ui/comparison.png` (SHA-256 `e946367e8628fd30043fd44c6c671fc9426a8b403265fedc5a8e41adb7f4c1d1`).
- Reviewed state: active video playback with first frame visible, controls revealed, two-item navigation, duration, volume, speed, and fullscreen controls.
- The visual source was normalized to the same `1440 × 900` review area before comparison. The product intentionally uses `object-fit: contain` for native playback, so portrait or nonmatching media may retain black letterboxing instead of cropping.

### Fidelity review

| Region | Result | Acceptance note |
| --- | --- | --- |
| Full-window theater stage | Pass | Playback fills one clipped dark stage; the former white content canvas and exposed lower strip are gone. |
| Top title bar | Pass | Filename and duration sit over a dark gradient at the upper-left; the Done action remains reachable at the upper-right. |
| Timeline and transport | Pass | Timeline spans the lower stage, playback is the emphasized primary control, and secondary controls use low-contrast translucent surfaces. |
| Page navigation | Pass | The page pill is centered above the bottom transport area and remains within the clipped video stage. |
| Native-video layering | Pass | The visible player root and stage remain transparent so the macOS native surface is not covered; DOM scrims and controls render above it. |
| Responsive and accessibility states | Pass | Compact layout, reduced motion, forced colors, disabled navigation, and keyboard-owned controls retain explicit styling and focused tests. |

### Findings and correction history

- P1: the original playback view split the interface into a white header, image-like content area, detached control card, and page navigation below the video. Correction: title, navigation, timeline, transport, and settings now share one immersive stage with top and bottom scrims.
- P1: placing the dark surface directly over the native macOS video hid the decoded frame. Correction: the preview root and stage become transparent after first-frame reveal while their pseudo-element scrims remain layered above the native surface.
- P1: acceptance rendering initially placed the synthetic native frame behind the underlying workspace. Correction: the acceptance-only frame and preview received explicit sibling z-order without changing production native layering.
- P1: the lower page-navigation area could appear outside the video boundary and expose unrelated content. Correction: all player chrome is nested inside the absolutely positioned, overflow-clipped stage; the page pill is anchored `154 px` above the lower edge and controls `24 px` above it.
- Intentional differences: the generated direction uses a neutral child-video placeholder, while product acceptance uses the repository's existing video-state asset; Viewer keeps its own icon set and typography, and contain-fit playback preserves the complete frame instead of imitating the source crop.

### Verification

- Focused player and accessibility suite: 7 files / 49 tests passed.
- `pnpm --dir ui check` — pass.
- `pnpm --dir ui build` — pass, 188 modules.
- Final `pnpm verify` retry — pass: policy and packaging gates, 106 UI files / 946 tests passed with 1 expected skip, production build, locked Rust formatting/Clippy/workspace tests, Tauri security, dependency sources/licenses, npm licenses, and bundled-video licenses.
- `git diff --check` — pass.
- The first full verification run correctly rejected raw color literals in the component stylesheet. The exact visual values were moved unchanged into semantic `--video-preview-*` tokens; the focused semantic-color contract passed before the successful full retry.
- The visual-acceptance capture produced the reviewed product image. Its comparison command reported a missing historical atlas reference, so the selected generated direction above was used as the explicit visual source rather than treating that unrelated missing file as a product failure.

Final result: passed

## 2026-08-06 — Five-Level Thumbnail Size Slider

### Accepted behavior

- Settings uses one accessible range slider with visible numeric stops `1` through `5`; the former `紧凑 / 标准 / 大图` radio cards are removed.
- The exact level-to-card-size contract is `1 = 96 px`, `2 = 132 px`, `3 = 168 px`, `4 = 204 px`, and `5 = 240 px`.
- Level 2 remains the default and invalid-value recovery level. The existing settings schema remains version 1, while all five values round-trip through the TypeScript model, Tauri DTO, Rust persistence, and JSON storage.
- Pointer input, Home/End and arrow-key changes share the same optimistic save behavior; stale completions cannot overwrite the latest user choice, and a failed latest write rolls the UI back to the last persisted value.

### Browser visual evidence

- Product/atlas evidence commit: `fa99c5d75e6c6cc04143b930f127fb8334774987` (later commits change only the native acceptance controller).
- Final browser run: 8 states at both `1024 × 720` and `1440 × 900`, 16/16 reference/product comparisons completed with no failed or unrun states.
- Evidence root: `target/viewer-visual-acceptance/five-level-thumbnail-slider-final-v2/fa99c5d75e6c6cc04143b930f127fb8334774987/`.
- Reviewed states: settings dialog (`DIA-01`), keyboard focus (`A11Y-01`), levels 1–5 (`THU-01`, `THU-02`, `THU-03`, `THU-08`, `THU-09`), and complete-card selection at maximum size (`THU-05`).
- Level 4 and 5 cards retain complete borders, filename rows, grid gaps, and selection geometry without clipping, overflow, or incomplete edge rendering at either viewport.

### macOS native evidence

- Level 5 grid (`THU-09`) passed at `1024 × 720`: `target/atlas-product-migration-acceptance/96d9a8c57d3da3335133d127a82f5ec728c6a687/1024x720/THU-09/combined.png`.
- Focused settings slider (`DIA-01`) passed at `1024 × 720`: `target/atlas-product-migration-acceptance/aea8957f451cbbdfe89a99747440a696d410738d/1024x720/DIA-01/combined.png`.
- The current display safe area limits the native Viewer window to `1280 × 800`, so a native `1440 × 900` capture was not forced. Browser component evidence covers that approved viewport. No macOS display scaling, resolution, Dock, or global scrollbar setting was changed.

### Final review

- Settings hierarchy, slider focus, accent thumb, filled track, numeric alignment, and close action match the approved visual atlas.
- Levels 4 and 5 increase only thumbnail/card scale; existing interaction logic, square card geometry, complete-card selection outline, filename treatment, and virtualization behavior remain intact.
- macOS current-stage severity: P0 `0`, P1 `0`, P2 `0`, P3 `0`.
- Windows-native rendering and input validation remain explicitly deferred until Windows development begins.

Final result: passed
