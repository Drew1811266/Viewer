# Viewer Visual Atlas Final Remediation Design

## Status

Approved for implementation by the user on 2026-07-31 after the final combined
visual and accessibility audit.

## Goal

Bring the standalone Viewer visual atlas into full agreement with
`2026-07-30-viewer-complete-ui-visual-upgrade-design.md`, then produce a fresh
1024×720 and 1440×900 visual acceptance set and a Figma audit board.

The remediation changes the atlas presentation and its prototype interactions.
It does not change Viewer file-operation semantics or the native macOS
application implementation.

## Root Causes

1. The completion plan instructed the atlas to keep one approved radial raster
   and explain alternate states in a text panel. That instruction conflicts
   with the complete specification, which requires the radial click, gesture,
   secondary, disabled, read-only, and keyboard states to be visible.
2. The generic toolbar handler routes the filter trigger to the search screen
   instead of opening the filter popover.
3. Filter counts are maintained in separate hard-coded strings instead of being
   derived from one condition model.
4. Collapsed-sidebar copy was placed into the same 52 px header cell as the
   compact project identity.
5. Disabled menu styling is scoped to the radial explanation panel instead of
   every disabled menu item.
6. Accessibility examples are explanatory cards rather than real Viewer shell
   states, and several prototype controls omit keyboard and ARIA behavior.

## Design Direction

### Real radial states

The atlas will render a real radial command surface in the Viewer workspace,
using the geometry and state language of the existing `RadialFileMenu`
component. The approved raster remains available as a compact reference, but it
is no longer the primary state visualization.

Every radial state must visibly change the command surface:

- click mode: one active primary sector;
- gesture mode: pointer path and active sector;
- mark: the mark secondary ring is open;
- organize: the organize secondary ring is open;
- disabled: unavailable primary and secondary commands stay in place and show
  both reduced emphasis and a lock/unavailable cue;
- read-only: write commands are disabled and the center explicitly says
  `只读`;
- keyboard: a visible focus ring identifies the current sector.

The radial surface uses the existing production component's sector geometry,
labels, and semantic order. It must not be substituted by a rectangular menu.

### Shell and toolbar corrections

- A collapsed sidebar uses one icon-only expand control with an accessible name;
  it never renders project name text and expand copy in the same 52 px cell.
- `筛选` opens and closes the filter popover without changing screens.
- `筛选 · N` and the panel condition badge derive from the same selected
  condition array, including advanced rules.
- Read-only menu items use the global disabled style and remain visibly
  different from enabled commands.
- Atlas-only state controls move into reserved atlas chrome and never cover
  Viewer controls.

### Accessibility and resilient states

- Viewer buttons and segmented controls expose at least a 32 px effective
  target.
- Tertiary text is darkened until small text reaches at least 4.5:1 on white
  and sidebar surfaces.
- Every visual field label is programmatically associated with its input.
- Focusable image cards select with Enter or Space.
- Progress surfaces expose `progressbar`, `aria-valuenow`, and a live status.
- Dialogs cover the complete Viewer shell with a modal scrim, expose dialog
  semantics, close with Escape, and restore the trigger focus.
- Accessibility pages render the real Viewer shell under keyboard, reduced
  motion, forced-colors, and 200% reflow demonstrations. Explanatory notes may
  accompany the shell but may not replace it.

### Layout corrections

- The information inspector is 334 px, within the approved 320–340 px range.
- Modal actions remain reachable at 1024×720.
- The state catalog remains usable at 1024×720 and 1440×900 without clipping or
  overlap.

## Interaction Model

The atlas keeps one state object. Toolbar popovers, scene state, selection,
dialog ownership, and focus return are stored separately so changing one does
not silently navigate another surface. Derived helpers calculate filter counts
and radial state data; renderers consume those helpers rather than duplicating
labels or counts.

## Verification

Automated JSDOM tests must fail before implementation and then prove:

- the filter trigger opens a filter popover and retains the current screen;
- filter badges agree with the visible selected conditions;
- collapsed sidebar uses an icon-only accessible expand control;
- every radial state renders distinct radial state markup;
- read-only menu rows are both semantically and visually disabled;
- image cards select from the keyboard;
- progress and dialog semantics exist;
- labels are associated with inputs;
- inspector and target-size contracts match the approved ranges.

Final visual acceptance must capture and inspect every important corrected state
at 1024×720 and representative corrected states at 1440×900. The audit board is
created only from screenshots accepted in that fresh run.
