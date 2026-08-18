# Rounded Video Rate Menu Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Replace the square native playback-rate selects in the production video preview with the approved rounded, keyboard-accessible Viewer listbox without changing playback behavior.

**Architecture:** Add one focused `VideoRateMenu` component that owns its open/focus state and emits a validated `VideoPlaybackRate`. Reuse it in the wide control strip and the compact More panel, then style the trigger, popover, and selected state with existing Viewer tokens so both layouts share one implementation.

**Tech Stack:** React 19, TypeScript, Vitest, Testing Library, Viewer CSS tokens, existing `ViewerPopover` and Lucide asset registry.

**Spec:** `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/video-ui-audit-2026-08-17/video-player-redesign.html`

## Global Constraints

- Preserve the exact supported rates: `0.5`, `0.75`, `1`, `1.25`, `1.5`, `2`.
- Preserve generation-bound `commands.setRate(rate)` behavior and existing wide/compact responsive boundaries.
- Use the existing Viewer icon assets and design tokens; add no dependency.
- The trigger must expose `aria-haspopup="listbox"` and `aria-expanded`; options must expose `role="option"` and `aria-selected`.
- Escape closes and returns focus; ArrowUp/ArrowDown/Home/End move option focus.

---

### Task 1: Shared rounded playback-rate menu

**Files:**
- Create: `ui/src/components/videoPreview/VideoRateMenu.tsx`
- Create: `ui/src/components/videoPreview/VideoRateMenu.test.tsx`

**Interfaces:**
- Consumes: `VideoPlaybackRate`, `ViewerPopover`, `ViewerIcon`, the current rate, supported rates, disabled state, and `onSelect(rate)`.
- Produces: `VideoRateMenu` with a rounded trigger and accessible listbox shared by wide and compact player layouts.

- [x] **Step 1: Write the failing behavior tests**

Test the real rendered component: the trigger has listbox semantics, opening reveals the six literal rates, selecting `1.5×` emits `1.5` and closes, ArrowDown moves from `1×` to `1.25×`, and Escape closes and restores trigger focus.

- [x] **Step 2: Run the focused test and confirm RED**

Run: `pnpm --dir ui exec vitest run src/components/videoPreview/VideoRateMenu.test.tsx`

Expected: FAIL because `VideoRateMenu` does not exist.

- [x] **Step 3: Implement the minimal component**

Create a controlled visual menu with internal open state, `ViewerPopover`, a button trigger containing the current rate and `chevron-down`, and one button per rate containing `check`. Close after selection, close on Escape through `ViewerPopover`, and implement ArrowUp/ArrowDown/Home/End focus movement.

- [x] **Step 4: Run the focused test and confirm GREEN**

Run: `pnpm --dir ui exec vitest run src/components/videoPreview/VideoRateMenu.test.tsx`

Expected: PASS.

### Task 2: Production integration in wide and compact layouts

**Files:**
- Modify: `ui/src/components/videoPreview/VideoControls.tsx`
- Modify: `ui/src/components/videoPreview/VideoControlsMoreMenu.tsx`
- Modify: `ui/src/components/videoPreview/VideoControls.test.tsx`
- Modify: `ui/src/components/videoPreview/VideoControlsMoreMenu.test.tsx`

**Interfaces:**
- Consumes: `VideoRateMenu` from Task 1 and the existing `commands.setRate(rate)` command.
- Produces: identical rounded speed-menu behavior in both player layouts with no remaining native speed combobox.

- [x] **Step 1: Change integration tests first**

Replace native combobox interactions with `button[name=/播放速度/]`, listbox option selection, and assertions that `commands.setRate(1.5)` fires once in wide and compact layouts.

- [x] **Step 2: Run the integration tests and confirm RED**

Run: `pnpm --dir ui exec vitest run src/components/videoPreview/VideoControls.test.tsx src/components/videoPreview/VideoControlsMoreMenu.test.tsx`

Expected: FAIL because production still renders native selects.

- [x] **Step 3: Replace both selects with the shared menu**

Wire `onSelect` to `onActivity()` plus `runCommand(commands.setRate(rate))`. In the wide layout, bind menu open state to `onAdjustingChange`; in the compact panel, keep the parent More popover's existing adjusting lifecycle.

- [x] **Step 4: Run focused integration tests and confirm GREEN**

Run the command from Step 2. Expected: PASS.

### Task 3: Viewer-token styling and product verification

**Files:**
- Modify: `ui/src/styles/videoPreview.css`
- Modify: `ui/src/styles/videoPreviewLayout.test.ts`
- Create: `target/video-ui-audit-2026-08-17/product-design-qa.md`

**Interfaces:**
- Consumes: `video-rate-menu*` class names from Task 1 and the approved prototype screenshot `07-rounded-speed-menu.png`.
- Produces: a 13px rounded menu, 9px rounded options, accent selected state, clear focus state, and verified production screenshots.

- [x] **Step 1: Add the failing rendered-style contract**

Assert the production stylesheet exposes a dedicated rounded rate-menu surface and option selected state while the player contains no native playback-rate select.

- [x] **Step 2: Run the style and component tests and confirm RED**

Run: `pnpm --dir ui exec vitest run src/styles/videoPreviewLayout.test.ts src/components/videoPreview/VideoRateMenu.test.tsx src/components/videoPreview/VideoControls.test.tsx src/components/videoPreview/VideoControlsMoreMenu.test.tsx`

Expected: FAIL until the new classes are styled.

- [x] **Step 3: Implement styles using Viewer tokens**

Apply the approved dimensions and states with `--viewer-surface`, `--viewer-border`, `--viewer-soft-surface`, `--viewer-accent-soft`, and `--viewer-accent`; include forced-colors handling.

- [x] **Step 4: Run UI verification**

Run the focused tests, `pnpm --dir ui check`, and `pnpm --dir ui build`. Then launch the development app, open a real video, exercise mouse and keyboard selection in both responsive layouts, inspect console errors, and capture a screenshot at the approved state.

- [x] **Step 5: Complete design QA**

Compare the prototype screenshot and production screenshot together. Fix any P0/P1/P2 mismatch; record the comparison in `product-design-qa.md` with final line `final result: passed`.
