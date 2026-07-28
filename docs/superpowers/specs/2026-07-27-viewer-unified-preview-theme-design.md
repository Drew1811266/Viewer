# Viewer Unified Light Preview Theme Design

Date: 2026-07-27
Status: Approved

## Problem

Viewer's default workspace and its single-image and text previews now use a
light visual system, but the two-to-four-image comparison workspace still
ships with an independent dark palette. Entering comparison therefore changes
the interface from light gray and white surfaces to nearly black workspace,
toolbar, pane, stage, footer, and control surfaces.

The mismatch is not caused by the operating-system appearance. The comparison
feature has hard-coded dark colors in the `.compare-workspace`,
`.compare-toolbar`, `.compare-pane`, `.compare-pane-header`,
`.compare-pane-stage`, and `.compare-pane > footer` rules. These rules were
introduced with the comparison feature and are independent from the shared
single-image and text preview rules.

The earlier light-preview design explicitly listed the comparison workspace as
a non-goal. This design supersedes that exclusion and unifies every preview
mode under one light preview theme.

## Goals

- Make single-image, text, and multi-image previews use one coherent light
  visual system.
- Replace duplicated preview color literals with shared CSS custom properties.
- Remove every legacy dark surface from the multi-image comparison workspace.
- Preserve clear hierarchy between workspace, chrome, image stage, active pane,
  controls, and errors.
- Preserve all current preview and comparison interactions, layouts, focus
  behavior, and data flow.
- Protect the unified theme with cascade-aware CSS contract tests.

## Non-goals

- Adding a user-selectable light/dark theme.
- Restyling the folder tree, search, results view, popovers, task panels, or
  other non-preview surfaces.
- Changing comparison layouts for two, three, or four images.
- Changing image loading, proxy selection, caching, memory budgets, zoom, pan,
  rotation, synchronization, markers, favorites, pane removal, or completion.
- Refactoring React component structure solely for CSS reuse.

## Shared Preview Theme Variables

Define a focused set of CSS custom properties in the root application
stylesheet:

```css
:root {
  --preview-surface: #f5f6f8;
  --preview-chrome: #fbfcfd;
  --preview-stage: #edf0f3;
  --preview-document-surface: #f7f7f8;
  --preview-panel-surface: #fff;
  --preview-text: #1f2328;
  --preview-muted: #68717d;
  --preview-border: #d8dce2;
  --preview-control-border: #8a94a3;
  --preview-control-hover-border: #747f8e;
  --preview-control-surface: #fff;
  --preview-control-hover-surface: #f3f5f7;
  --preview-accent: #2477d4;
  --preview-accent-surface: #d9e8ff;
  --preview-accent-text: #174f8f;
  --preview-danger: #9d1c13;
  --preview-danger-surface: #fff3f0;
  --preview-image-shadow: 0 8px 28px rgb(34 42 53 / 14%);
}
```

These variables are globally available but intentionally consumed only by the
preview and comparison rules in this change. Their names describe preview
semantics rather than a general application theme. This keeps the work focused
while giving all preview modes one source of truth.

## Approved User Experience

### Single-image preview

The existing light presentation remains visually unchanged. Its overlay,
toolbar, controls, image stage, image boundary, image shadow, navigation footer,
focus outline, and error color are rewritten to consume the shared variables
instead of literal colors.

### Text preview

The document body remains the `--preview-document-surface` light reading
surface. Its sticky toolbar, encoding selector, close control, foreground, and
error treatment consume the same variables as the single-image preview.

### Multi-image comparison

The comparison workspace retains its embedded two-, three-, and four-pane
layouts but changes completely to light surfaces:

- the outer workspace uses `--preview-surface` and `--preview-text`;
- the main toolbar, each pane header, and each marker footer use
  `--preview-chrome` with `--preview-border` separators;
- each pane uses `--preview-panel-surface` with a light border;
- every image stage uses `--preview-stage`;
- compared images use the shared boundary and restrained image shadow without
  changing their measured geometry;
- the active pane keeps a visible `--preview-accent` border;
- the synchronized-mode button uses `--preview-accent-surface`,
  `--preview-accent`, and `--preview-accent-text` instead of a dark-blue fill;
- ordinary toolbar, pane-close, and marker buttons use the shared white control
  surface, gray border, light hover surface, and blue keyboard-focus outline;
- comparison alerts use `--preview-danger-surface` and `--preview-danger`.

Marker and favorite states retain their existing semantic meaning. The shared
comparison button base must not erase active review or favorite state
presentation.

## Architecture and Data Flow

This is a presentation-only refactor in `ui/src/styles/app.css`. Existing
component boundaries already provide the required styling hooks:

- `.preview-overlay`, `.preview-toolbar`, `.image-preview-stage`,
  `.image-preview > footer`, and `.text-preview` consume the variables for
  single-image and text previews;
- `.compare-workspace` owns the shared comparison surface;
- `.compare-toolbar` owns global comparison controls;
- `.compare-pane` owns pane shell and active-pane state;
- `.compare-pane-header` and `.compare-pane > footer` own pane chrome;
- `.compare-pane-stage` owns the neutral image canvas;
- `.compare-pane-stage img` owns the visual image boundary and shadow;
- comparison alert selectors own light error treatment.

No React props, state, reducers, hooks, backend requests, IPC calls, persistence,
or image representation logic changes. The runtime data flow remains:

1. selection opens a `CompareWorkspace`;
2. the existing compare model chooses the two-, three-, or four-pane layout;
3. `ComparePane` loads and transforms each image;
4. existing callbacks update active panes, synchronized transforms, markers,
   favorites, and removals;
5. CSS variables control presentation only.

## Control and Accessibility States

Preview and comparison controls share:

- white default surfaces with visible gray borders;
- light-gray hover surfaces and stronger hover borders;
- blue `:focus-visible` outlines with a two-pixel offset;
- existing disabled semantics;
- a minimum target size appropriate to each compact toolbar or pane footer.

The active comparison pane and synchronized-mode button use accent colors as
state indicators. Color is not the only active-pane signal because the existing
pane border remains structural. Text and non-text contrast targets remain at
least 4.5:1 and 3:1 respectively.

## Error Handling

Loading, invalid selection, original-image budget errors, and image-load errors
retain their existing messages and recovery behavior. Only their visual
treatment changes:

- light danger surface instead of dark brown;
- dark danger foreground instead of pale red;
- the same padding and placement as today.

The invalid comparison workspace also inherits the shared light surface and
control styling.

## Testing

CSS contract tests will verify:

- the shared custom properties resolve to the approved values;
- single-image and text preview selectors consume the variables;
- comparison workspace, toolbar, pane, header, stage, footer, buttons,
  active-pane state, synchronized state, focus state, and alerts consume the
  variables;
- final comparison selectors do not resolve to any legacy comparison palette
  values: `#171a1f`, `#24282f`, `#0e1013`, `#20242a`, `#303640`,
  `#4d5663`, `#275e9d`, `#4e8ed8`, `#4e94df`, `#4b2c22`, `#ffd5c7`, or
  `#b8c0ca`;
- control, active, and error colors satisfy the relevant contrast thresholds.

Existing component tests will continue to verify:

- single-image fit, original size, zoom, rotation, navigation, and closing;
- text decoding, Markdown rendering, links, scrolling, focus, and closing;
- two-, three-, and four-image layouts;
- synchronized and independent transforms;
- active-pane selection, zoom, pan, rotation, proxy/original switching;
- markers, favorites, pane removal, keyboard exit, and read-only behavior.

Before completion, run the full UI static check, test suite, and production
build. Start the latest development binary and visually inspect single-image,
text, and two-, three-, and four-image previews. Confirm light surfaces,
readable controls, active states, no clipping, and unchanged interactions.

## Acceptance Criteria

- Every preview mode uses the shared light preview variables.
- Multi-image comparison contains no black or dark-gray workspace, toolbar,
  pane, stage, header, footer, or ordinary control surfaces.
- Single-image and text previews remain visually consistent with their approved
  light design.
- White-background images remain distinguishable from their stages.
- Active pane, synchronized mode, review, favorite, hover, disabled, and
  keyboard-focus states remain clear.
- Two-, three-, and four-image layouts and all preview interactions behave as
  before.
- CSS contract tests, existing component tests, the complete UI suite, static
  checks, production build, and visual inspection all pass.
