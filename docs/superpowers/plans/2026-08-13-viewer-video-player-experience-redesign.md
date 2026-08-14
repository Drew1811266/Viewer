# Viewer Video Player Experience Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:test-driven-development` for every behavior change and `superpowers:verification-before-completion` before claiming completion. Use `superpowers:subagent-driven-development` only if the user explicitly chooses delegated execution; otherwise execute inline task by task.

**Goal:** Rebuild the video preview chrome around one stable, responsive layout so playback feels deliberate at every window size and first-frame transitions no longer produce black-band artifacts.

**Architecture:** Preserve the generation-safe bridge and native fitted surface. Restructure only the React chrome around a bottom safe-region container, add one media-query-driven responsive branch for secondary controls, and use the existing visibility hook plus semantic CSS tokens for ambient reveal and accessibility states.

**Tech Stack:** React 19, TypeScript, CSS custom properties, Vitest, Testing Library, Vite, Tauri 2, existing Viewer primitives and visual-acceptance tooling.

## Global Constraints

- Keep `useVideoBridge` as the sole authority for generation, commands, fitted surface geometry, and matte insets.
- Do not change Rust/Tauri playback commands, libmpv/ffmpeg behavior, video cache, or media metadata.
- Do not crop/stretch video, introduce a second media rectangle, or expose desktop content below the native surface.
- Do not duplicate accessible volume/rate/frame-step controls between wide and compact layouts.
- Reuse `ViewerPopover`, `ViewerIconButton`, native range/select inputs, and the existing Lucide registry; add no dependency.
- Preserve the 2.5 second visibility timer, generation-safe command sequencing, first-frame gate, shortcuts, reduced motion, and forced colors.
- Use semantic `--video-preview-*` tokens for all new colors, shadows, and motion roles.
- All visible interactive targets must be at least 44×44 CSS pixels.

---

## File Map

- `ui/src/components/VideoPreview.tsx`: stage state attributes, bottom chrome grouping, metadata, and navigation placement.
- `ui/src/components/VideoPreview.test.tsx`: stage containment, first-frame reveal, navigation, and responsive shell regressions.
- `ui/src/components/videoPreview/VideoControls.tsx`: wide/compact control composition.
- `ui/src/components/videoPreview/VideoControls.test.tsx`: exact visible controls and command mapping in both layouts.
- `ui/src/components/videoPreview/VideoControlsMoreMenu.tsx`: compact-only frame-step, volume, and rate disclosure.
- `ui/src/components/videoPreview/VideoControlsMoreMenu.test.tsx`: popover, focus restoration, labels, and commands.
- `ui/src/components/videoPreview/useVideoResponsiveLayout.ts`: live `1100px` match-media boundary.
- `ui/src/components/videoPreview/useVideoResponsiveLayout.test.tsx`: initial and changing media-query behavior.
- `ui/src/components/videoPreview/useVideoControlsVisibility.test.tsx`: More/focus/adjustment keeps controls visible.
- `ui/src/styles/tokens.css`: ambient matte, reveal, dock, popover, and control roles.
- `ui/src/styles/videoPreview.css`: wide/compact/narrow layout, reveal veil, chrome hierarchy, and accessibility states.
- `ui/src/styles/videoPreviewLayout.test.ts`: executable geometry and responsive CSS contract.
- `ui/src/styles/visualAccessibility.test.ts`: forced-colors, focus, target size, and reduced-motion contract.
- `design-qa.md`: exact visual captures and final comparison record.

### Task 1: Establish One Stage And Bottom-Chrome Geometry

**Files:**
- Modify: `ui/src/components/VideoPreview.tsx`
- Modify: `ui/src/components/VideoPreview.test.tsx`
- Modify: `ui/src/styles/videoPreviewLayout.test.ts`
- Modify: `ui/src/styles/tokens.css`
- Modify: `ui/src/styles/videoPreview.css`

**Interfaces:**
- Consumes: `matteInsets`, `state.surfaceVisible`, `controlState.fullscreen`, and `controls.visible`.
- Produces: `.video-preview-bottom-chrome`, `data-chrome-visible`, and an ambient first-frame reveal veil; no bridge/native interface changes.

- [ ] **Step 1: Write failing component geometry tests**

Add regressions to `VideoPreview.test.tsx` that assert the toolbar, navigation,
and `VideoControls` all live inside the clipped stage; navigation and controls
share `.video-preview-bottom-chrome`; the stage exposes active
`data-surface-visible` and chrome visibility; and the existing four matte inset
variables remain unchanged.

- [ ] **Step 2: Write failing CSS behavior contracts**

Update `videoPreviewLayout.test.ts` to require:

```ts
expect(rule('.video-preview-bottom-chrome')).toMatchObject({
  bottom: 'var(--video-preview-safe-bottom)',
  display: 'grid',
  position: 'absolute',
})
expect(rule('.video-preview-stage::after')).toMatchObject({
  background: 'var(--video-preview-ambient-matte)',
  opacity: '1',
  'pointer-events': 'none',
})
expect(rule(".video-preview-stage[data-surface-visible='true']::after")).toMatchObject({
  opacity: '0',
})
```

Also require the letterbox border to use the same ambient matte token and remove
the old independent absolute positioning from `.video-controls` and
`.preview-navigation-float`.

- [ ] **Step 3: Run focused tests and verify RED**

```bash
pnpm --dir ui exec vitest run \
  src/components/VideoPreview.test.tsx \
  src/styles/videoPreviewLayout.test.ts \
  --reporter=verbose
```

Expected: FAIL because bottom chrome is not grouped, the stage reveal veil does
not exist, and the current matte/control selectors do not implement the new
geometry.

- [ ] **Step 4: Implement minimal structure and semantic tokens**

Wrap navigation plus `VideoControls` in `.video-preview-bottom-chrome`; expose
`data-chrome-visible={controls.visible}` on the preview and bottom container;
keep the existing `matteStyle` values untouched. Add semantic roles including:

```css
--video-preview-ambient-matte: #121722;
--video-preview-reveal-duration: 120ms;
--video-preview-safe-inline: clamp(16px, 2.2vw, 32px);
--video-preview-safe-bottom: max(18px, env(safe-area-inset-bottom));
--video-preview-dock-max-width: 1180px;
```

Use the ambient token for the stage and letterbox border. Implement
`.video-preview-stage::after` as an ambient reveal veil above the native surface
that transitions to opacity zero only when the active first frame is visible.
Keep the veil transition disabled under reduced motion.

- [ ] **Step 5: Implement the bottom safe region**

Make `.video-preview-bottom-chrome` the only absolute bottom anchor. Center it,
cap its width, and stack navigation above the dock without overlap. Convert
`.video-controls` and `.preview-navigation-float` to normal children inside this
grid. Keep fullscreen safe-area insets on the container rather than duplicating
them on child controls.

- [ ] **Step 6: Run focused GREEN**

```bash
pnpm --dir ui exec vitest run \
  src/components/VideoPreview.test.tsx \
  src/styles/videoPreviewLayout.test.ts \
  --reporter=dot
pnpm --dir ui check
git diff --check
```

- [ ] **Step 7: Commit the geometry foundation**

```bash
git add ui/src/components/VideoPreview.tsx ui/src/components/VideoPreview.test.tsx \
  ui/src/styles/videoPreviewLayout.test.ts ui/src/styles/tokens.css \
  ui/src/styles/videoPreview.css
git commit -m "refactor: unify video player chrome geometry"
```

### Task 2: Add Responsive Control Priority And Compact More Panel

**Files:**
- Create: `ui/src/components/videoPreview/useVideoResponsiveLayout.ts`
- Create: `ui/src/components/videoPreview/useVideoResponsiveLayout.test.tsx`
- Create: `ui/src/components/videoPreview/VideoControlsMoreMenu.tsx`
- Create: `ui/src/components/videoPreview/VideoControlsMoreMenu.test.tsx`
- Modify: `ui/src/components/videoPreview/VideoControls.tsx`
- Modify: `ui/src/components/videoPreview/VideoControls.test.tsx`
- Modify: `ui/src/styles/videoPreview.css`

**Interfaces:**

```ts
export const VIDEO_COMPACT_QUERY = '(max-width: 1099px)'
export function useVideoResponsiveLayout(): { compact: boolean }

export interface VideoControlsMoreMenuProps {
  open: boolean
  onOpenChange(open: boolean): void
  disabled: boolean
  view: Pick<VideoControlViewState, 'volumePercent' | 'rate'>
  commands: Pick<VideoControlCommands, 'step' | 'setVolume' | 'setRate'>
  onActivity(): void
  onAdjustingChange(adjusting: boolean): void
}
```

- [ ] **Step 1: Write the responsive-hook RED tests**

Stub `window.matchMedia`, then assert initial wide/compact state and live `change`
event updates. Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/videoPreview/useVideoResponsiveLayout.test.tsx \
  --reporter=verbose
```

Expected: FAIL because the hook and query constant do not exist.

- [ ] **Step 2: Implement the minimal live media-query hook**

Use one `MediaQueryList`, initialize from `matches`, subscribe with
`addEventListener('change', ...)`, and remove the listener on cleanup. Do not
read `window.innerWidth` independently.

- [ ] **Step 3: Write compact More-panel RED tests**

Require one `sliders-horizontal` trigger with `aria-expanded`, a named popover,
previous/next frame actions, one native volume slider, one native rate select,
outside/Escape dismissal through `ViewerPopover`, trigger focus restoration,
and exact command mapping. Assert opening/closing calls
`onAdjustingChange(true/false)` so idle controls cannot disappear mid-edit.

- [ ] **Step 4: Implement `VideoControlsMoreMenu`**

Reuse `ViewerPopover`, `ViewerIconButton`, and existing form labels. Keep the
trigger in the main settings row and render frame-step, volume, and rate only
inside this component when compact. Closing the panel must clear adjusting
state even on unmount or responsive transition.

- [ ] **Step 5: Write wide/compact composition RED tests**

In `VideoControls.test.tsx`, mock the responsive hook and assert:

- wide: frame-step, volume, and rate are inline; no More trigger exists;
- compact: play/pause, mute, fullscreen, and More remain visible; frame-step,
  volume, and rate appear only after opening More;
- every action still maps to exactly one existing generation-bound command;
- ended/failure/disabled semantics are identical in both layouts.

- [ ] **Step 6: Implement one accessible control branch**

Refactor shared volume/rate markup into small internal render helpers or the More
component without creating hidden duplicate inputs. Keep timeline rendered once
and play/pause visually primary. Add `data-layout="wide|compact"` for CSS and
acceptance diagnostics.

- [ ] **Step 7: Implement wide, compact, and narrow CSS**

- Wide: two dock rows, complete inline transport/settings.
- Compact: reduced gaps; compact More trigger visible.
- Narrow below 760px: timeline/time first row; action row second; popover stays
  within 16px viewport gutters and scrolls internally if necessary.

Keep 44×44 targets and avoid horizontal scrolling at 720px width.

- [ ] **Step 8: Run focused GREEN**

```bash
pnpm --dir ui exec vitest run \
  src/components/videoPreview/useVideoResponsiveLayout.test.tsx \
  src/components/videoPreview/VideoControlsMoreMenu.test.tsx \
  src/components/videoPreview/VideoControls.test.tsx \
  src/components/videoPreview/VideoTimeline.test.tsx \
  --reporter=dot
pnpm --dir ui check
git diff --check
```

- [ ] **Step 9: Commit responsive controls**

```bash
git add ui/src/components/videoPreview/useVideoResponsiveLayout.ts \
  ui/src/components/videoPreview/useVideoResponsiveLayout.test.tsx \
  ui/src/components/videoPreview/VideoControlsMoreMenu.tsx \
  ui/src/components/videoPreview/VideoControlsMoreMenu.test.tsx \
  ui/src/components/videoPreview/VideoControls.tsx \
  ui/src/components/videoPreview/VideoControls.test.tsx \
  ui/src/styles/videoPreview.css
git commit -m "feat: adapt video controls to window width"
```

### Task 3: Finish Player States, Idle Chrome, And Accessibility

**Files:**
- Modify: `ui/src/components/VideoPreview.tsx`
- Modify: `ui/src/components/VideoPreview.test.tsx`
- Modify: `ui/src/components/videoPreview/useVideoControlsVisibility.test.tsx`
- Modify: `ui/src/styles/videoPreview.css`
- Modify: `ui/src/styles/videoPreviewLayout.test.ts`
- Modify: `ui/src/styles/visualAccessibility.test.ts`

**Interfaces:**
- Consumes: existing `controls.visible`, `reducedMotion`, playback phase, More
  adjusting/focus state, and first-frame state.
- Produces: synchronized chrome/cursor visibility attributes and complete state
  styling; no playback command changes.

- [ ] **Step 1: Write state and idle-visibility RED tests**

Add tests that prove:

- playing idle hides the dock after exactly 2.5 seconds;
- pointer or owned shortcut activity restores it;
- paused, ended, failed, seeking, focused, and open-More states remain visible;
- fullscreen idle also hides the cursor, while pointer activity restores it;
- metadata/navigation are absent in fullscreen but present in normal preview;
- first-frame reveal never removes the matte insets.

- [ ] **Step 2: Write accessibility/style RED contracts**

Require 44×44 visible targets, focus-visible outlines, forced-colors Canvas
surfaces, reduced-motion transition removal, no pure `#000`/`black` player
surface, and no full-width opaque title/footer band.

- [ ] **Step 3: Run focused RED**

```bash
pnpm --dir ui exec vitest run \
  src/components/VideoPreview.test.tsx \
  src/components/videoPreview/useVideoControlsVisibility.test.tsx \
  src/styles/videoPreviewLayout.test.ts \
  src/styles/visualAccessibility.test.ts \
  --reporter=verbose
```

- [ ] **Step 4: Implement synchronized chrome presentation**

Use preview data attributes rather than a second timer. Apply opacity/transform
only to chrome, never to the native geometry. In fullscreen, set `cursor: none`
only when controls are idle; restore it with `controls.reveal`. Keep paused/end/
failure chrome stable. Ensure More focus/adjusting feeds the existing hook.

- [ ] **Step 5: Finish semantic visual states**

Implement localized metadata/dock/popover surfaces, clear enabled/hover/active/
disabled contrast, a strong play/pause action, and consistent navigation.
Forced-colors must replace translucent layers with system colors. Reduced motion
must remove reveal/auto-hide transforms but preserve immediate visibility state.

- [ ] **Step 6: Run focused GREEN and the complete UI suite**

```bash
pnpm --dir ui exec vitest run \
  src/components/VideoPreview.test.tsx \
  src/components/videoPreview/VideoControls.test.tsx \
  src/components/videoPreview/VideoControlsMoreMenu.test.tsx \
  src/components/videoPreview/VideoTimeline.test.tsx \
  src/components/videoPreview/useVideoControlsVisibility.test.tsx \
  src/components/videoPreview/useVideoResponsiveLayout.test.tsx \
  src/styles/videoPreviewLayout.test.ts \
  src/styles/visualAccessibility.test.ts \
  --reporter=dot
pnpm --dir ui test
pnpm --dir ui check
pnpm --dir ui build
git diff --check
```

- [ ] **Step 7: Commit player-state refinement**

```bash
git add ui/src/components/VideoPreview.tsx ui/src/components/VideoPreview.test.tsx \
  ui/src/components/videoPreview/useVideoControlsVisibility.test.tsx \
  ui/src/styles/videoPreview.css ui/src/styles/videoPreviewLayout.test.ts \
  ui/src/styles/visualAccessibility.test.ts
git commit -m "feat: refine video player states"
```

### Task 4: Visual Acceptance At Four Layouts And Final Gate

**Files:**
- Modify: `design-qa.md`
- Evidence only: `target/viewer-visual-acceptance/video-player-redesign/`

**Interfaces:**
- Consumes: existing `video-playing-controls`, `video-paused-controls`,
  `video-ended`, and `video-failed-retry` acceptance states.
- Produces: exact viewport captures and final repository verification evidence.

- [ ] **Step 1: Capture the approved layouts**

Capture the playing state at `720×720`, `1024×720`, and `1440×900`, plus one
fullscreen capture, using the existing visual acceptance runner. Capture paused,
ended, and failed states at `1440×900`. Store all artifacts under
`target/viewer-visual-acceptance/video-player-redesign/`.

- [ ] **Step 2: Compare against the approved Figma board**

Create a same-scale comparison for wide, compact, and narrow layouts and verify:

- no pure-black flash/band or desktop exposure;
- fitted video remains primary and uncropped;
- title, Done, navigation, timeline, transport, and popover do not collide;
- compact More owns secondary controls exactly once;
- time labels and 44×44 targets remain legible;
- fullscreen idle and revealed states follow the approved hierarchy.

If any overlap, clipping, unreachable control, P0, P1, or P2 issue remains,
return to the owning task and do not mark QA passed.

- [ ] **Step 3: Record design QA**

Append the Figma URL, viewport list, artifact paths, corrected findings,
intentional differences, accessibility observations, and `final result: passed`
to `design-qa.md` only after the comparison passes.

- [ ] **Step 4: Run final verification**

```bash
pnpm verify
git diff --check
git status --short
```

Expected: policy, UI check/tests/build, locked Rust formatting/Clippy/workspace
tests, security boundaries, dependency policy, and license gates all pass.

- [ ] **Step 5: Commit visual evidence**

```bash
git add design-qa.md
git commit -m "docs: verify redesigned video player"
```

## Plan Self-Review

- Spec coverage: ambient first-frame transition, stable geometry, responsive
  control priority, More disclosure, auto-hide, fullscreen, all playback states,
  accessibility, and visual acceptance each map to an executable task.
- Scope: React/CSS/test/QA only; native rendering and playback commands remain
  unchanged.
- Interface consistency: `VideoControlsMoreMenu` uses existing view/command
  types; `useVideoResponsiveLayout` is the single owner of the breakpoint;
  bottom chrome consumes existing visibility and geometry state.
- TDD order: every production change follows a named failing test and focused
  RED command before implementation.
- Placeholder scan: no TODO, TBD, unnamed test, undefined file, or deferred
  implementation step remains.
