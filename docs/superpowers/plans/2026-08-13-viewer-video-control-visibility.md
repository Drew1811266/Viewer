# Viewer Video Control Visibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove full-frame black gradient transitions and make every video playback control clearly visible through localized high-contrast surfaces.

**Architecture:** Keep the existing React component tree, playback state, native video surface, and semantic token system. Drive the refinement through the existing stylesheet contract test, then update only semantic video-preview tokens and CSS composition; render the existing playing-controls acceptance state for visual comparison before the complete repository gate.

**Tech Stack:** React 19, TypeScript, CSS custom properties, Vitest, Vite, Tauri 2, existing Viewer visual-acceptance runner.

## Global Constraints

- Remove the full-width top and bottom gradient scrims completely.
- Preserve contain-fit video geometry and native-surface transparency.
- Do not change playback commands, shortcuts, navigation, timeline requests, fullscreen behavior, or decoding.
- Do not add new icons, controls, settings, dependencies, or raw component color literals.
- Keep forced-colors, reduced-motion, responsive, hover, focus, active, and disabled states explicit.
- Keep the lower control dock inset from every window edge; it must not become a full-width black band.

---

## File Map

- `ui/src/styles/videoPreviewLayout.test.ts`: executable visual contract for scrim removal, localized metadata surfaces, navigation affordances, and the playback dock.
- `ui/src/styles/tokens.css`: semantic palette and shadow roles used by the refined player.
- `ui/src/styles/videoPreview.css`: stage, metadata capsule, page navigation, playback dock, button, timeline, responsive, and accessibility presentation.
- `design-qa.md`: source screenshot, implementation capture, comparison findings, and final result.

### Task 1: Replace Full-Frame Scrims With Localized High-Contrast Controls

**Files:**
- Modify: `ui/src/styles/videoPreviewLayout.test.ts`
- Modify: `ui/src/styles/tokens.css`
- Modify: `ui/src/styles/videoPreview.css`

**Interfaces:**
- Consumes: existing `.video-preview-stage`, `.video-preview-topbar`, `.video-preview-title`, `.preview-navigation-float`, `.video-controls`, `.video-controls__play`, and `.video-timeline` selectors.
- Produces: a style-only visual contract; no React props, bridge commands, or native interfaces change.

- [ ] **Step 1: Write the failing style contract**

Extend `ui/src/styles/videoPreviewLayout.test.ts` with behavior assertions that express the approved design:

```ts
it('keeps the video frame free of full-width scrims', () => {
  expect(rule('.video-preview-stage::before')).toMatchObject({ display: 'none' })
  expect(rule('.video-preview-stage::after')).toMatchObject({ display: 'none' })
})

it('uses localized high-contrast surfaces for every control group', () => {
  expect(rule('.video-preview-title')).toMatchObject({
    background: 'var(--video-preview-metadata-surface)',
    border: '1px solid var(--video-preview-surface-border)',
  })
  expect(rule('.video-controls')).toMatchObject({
    background: 'var(--video-preview-dock-surface)',
    border: '1px solid var(--video-preview-surface-border)',
    'border-radius': '18px',
  })
  expect(rule('.video-controls .video-controls__play')).toMatchObject({
    background: 'var(--viewer-accent)',
    color: 'var(--viewer-on-accent)',
  })
})
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
pnpm --dir ui exec vitest run src/styles/videoPreviewLayout.test.ts --reporter=verbose
```

Expected: FAIL because both pseudo-elements still render gradient scrims, the title has no localized surface, the control group has no dock surface, and play/pause is not solid accent.

- [ ] **Step 3: Add only the required semantic tokens**

In `ui/src/styles/tokens.css`, replace unused full-width scrim roles with localized surface roles and retain all palette literals in the token file:

```css
--video-preview-metadata-surface: rgb(17 20 27 / 82%);
--video-preview-dock-surface: rgb(13 16 22 / 88%);
--video-preview-surface-border: rgb(255 255 255 / 24%);
--video-preview-surface-shadow: 0 14px 36px rgb(0 0 0 / 30%);
--video-preview-button-surface: rgb(255 255 255 / 10%);
--video-preview-button-hover-surface: rgb(255 255 255 / 20%);
--video-preview-button-disabled: rgb(255 255 255 / 6%);
```

Remove `--video-preview-top-scrim` and `--video-preview-bottom-scrim` after the component stylesheet no longer consumes them.

- [ ] **Step 4: Implement the minimal CSS composition**

In `ui/src/styles/videoPreview.css`:

```css
.video-preview-stage::before,
.video-preview-stage::after {
  display: none;
}

.video-preview-title {
  background: var(--video-preview-metadata-surface);
  border: 1px solid var(--video-preview-surface-border);
  border-radius: 12px;
  box-shadow: var(--video-preview-surface-shadow);
  padding: 10px 14px;
  -webkit-backdrop-filter: blur(18px) saturate(130%);
  backdrop-filter: blur(18px) saturate(130%);
}

.video-controls {
  background: var(--video-preview-dock-surface);
  border: 1px solid var(--video-preview-surface-border);
  border-radius: 18px;
  box-shadow: var(--video-preview-surface-shadow);
  padding: 14px 16px;
  -webkit-backdrop-filter: blur(20px) saturate(130%);
  backdrop-filter: blur(20px) saturate(130%);
}

.video-controls .video-controls__play {
  background: var(--viewer-accent);
  border-color: var(--viewer-accent);
  color: var(--viewer-on-accent);
}
```

Increase localized button contrast without changing dimensions or behavior: use the new button surface for secondary actions, a stronger border, explicit hover and focus-visible states, and a recognizable disabled surface. Keep navigation as its own compact capsule and set enabled arrows to full-opacity high-contrast icons.

- [ ] **Step 5: Run focused GREEN and regression tests**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/styles/videoPreviewLayout.test.ts \
  src/styles/app.test.ts \
  src/styles/visualAccessibility.test.ts \
  src/components/VideoPreview.test.tsx \
  src/components/videoPreview/VideoControls.test.tsx \
  src/components/videoPreview/VideoTimeline.test.tsx \
  --reporter=dot
pnpm --dir ui check
git diff --check
```

Expected: all tests and checks pass with no component color-literal violations.

- [ ] **Step 6: Commit the executable UI refinement**

```bash
git add ui/src/styles/videoPreviewLayout.test.ts ui/src/styles/tokens.css ui/src/styles/videoPreview.css
git commit -m "feat: improve video control visibility"
```

### Task 2: Rendered Comparison And Complete Verification

**Files:**
- Modify: `design-qa.md`
- Evidence only: `target/viewer-visual-acceptance/video-control-visibility/`

**Interfaces:**
- Consumes: the existing `video-playing-controls` acceptance scene and supplied running-product screenshot.
- Produces: a same-state visual comparison and a final repository verification result.

- [ ] **Step 1: Capture the exact playing-controls state**

Run:

```bash
pnpm accept:visual -- \
  --id video-playing-controls \
  --viewport 1440x900 \
  --output-root target/viewer-visual-acceptance/video-control-visibility
```

Expected: a `1440 × 900` product screenshot exists even if the historical atlas comparison reports an unrelated missing reference.

- [ ] **Step 2: Compare the source and implementation together**

Open the supplied source screenshot and the new `product.png`, normalize both to the same `1440 × 900` viewport, and create a side-by-side comparison under `target/design-qa-video-control-visibility/`.

The comparison must confirm:

- no full-width top or bottom gradient remains;
- video brightness remains uniform outside localized controls;
- every enabled button has a visible icon and boundary;
- the play/pause action is visibly primary;
- the lower dock remains inset and does not read as a black border.

- [ ] **Step 3: Update the design QA record**

Append a dated section to `design-qa.md` that records the source image, exact product capture, viewport, comparison path, intentional differences, corrected findings, and `final result: passed`. If any P0/P1/P2 remains, write `final result: blocked`, return to Task 1, and do not hand off.

- [ ] **Step 4: Run the complete gate**

Run:

```bash
pnpm verify
git diff --check
```

Expected: policy, packaging, UI check/tests/build, locked Rust formatting/Clippy/workspace tests, Tauri security, dependency/license, and bundled-video license gates all pass.

- [ ] **Step 5: Commit verified visual evidence**

```bash
git add design-qa.md
git commit -m "docs: verify visible video controls"
```

## Plan Self-Review

- Spec coverage: scrim removal, localized surfaces, primary action emphasis, navigation visibility, accessibility, visual comparison, and full verification each map to an explicit step.
- Scope: CSS tokens, CSS composition, executable style contracts, and visual evidence only; playback and native APIs remain untouched.
- Type consistency: every selector and semantic token referenced in later steps is introduced or already exists in Task 1.
- Placeholder scan: no deferred implementation, unnamed tests, or undefined interfaces remain.
