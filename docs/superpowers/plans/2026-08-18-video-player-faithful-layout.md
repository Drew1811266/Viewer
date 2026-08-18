# Faithful Video Player Layout Implementation Plan

> **For Codex:** Execute this plan inline with test-driven development. The approved screenshot is the sole visual source of truth.

**Goal:** Replace the incorrect full-width video chrome with the approved three-chip top layout and compact centered bottom controller.

**Architecture:** Preserve the existing `VideoPreview`, `ViewerToolbar`, `VideoControls`, command wiring, accessibility, auto-hide, fullscreen, and rounded rate menu behavior. Use video-preview-scoped CSS to turn the toolbar's three semantic regions into independent floating surfaces and constrain the bottom chrome to the approved geometry.

**Tech Stack:** React, TypeScript, CSS, Vitest, Vite visual acceptance.

---

### Task 1: Freeze the approved visual geometry

- [x] Update `videoPreviewLayout.test.ts` to require three independent top surfaces.
- [x] Require 44px top chips at 22px/20px safe-area offsets.
- [x] Require a centered bottom controller with `min(920px, calc(100% - 44px))`, 62px height, and 17px radius.
- [x] Run the focused layout test and observe the expected RED failures.

### Task 2: Implement the production layout

- [x] Make the toolbar container transparent and borderless.
- [x] Style leading, center navigation, and actions as three independent rounded surfaces.
- [x] Center and constrain the bottom chrome while preserving auto-hide transforms.
- [x] Match the approved control spacing, button sizing, and responsive rules.
- [x] Run focused component and layout tests to GREEN.

### Task 3: Visual verification

- [x] Build and open the production visual-acceptance scene at 1440x900.
- [x] Compare the production screenshot with the approved screenshot.
- [x] Fix all material geometry, overlap, clipping, and visual hierarchy mismatches.
- [x] Save the comparison evidence and a design-QA report ending in `final result: passed`.

### Task 4: Final verification

- [x] Run UI static checks, focused tests, full UI tests, and production build.
- [x] Audit the final diff and commit only the corrected implementation.
