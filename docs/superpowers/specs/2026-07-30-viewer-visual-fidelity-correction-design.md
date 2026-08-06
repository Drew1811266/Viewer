# Viewer Visual Fidelity Correction Design

## Status

Approved on 2026-07-30.

This document is a corrective addendum to
`2026-07-30-viewer-complete-ui-visual-upgrade-design.md`. Where the two
documents conflict, this addendum is authoritative.

## Why this correction exists

The first implementation pass drifted from the approved interaction character
in four visible ways:

1. ordinary secondary click opened a conventional rectangular menu while a
   held secondary button opened the radial menu;
2. the selection boundary wrapped the image, filename, and complete item card;
3. toolbar popovers rendered as stacks of bordered buttons;
4. native review sometimes targeted a stale bundled application instead of
   the current development process.

Passing structural and unit tests did not prove visual fidelity. The
corrective pass therefore treats the approved reference images and native
same-state captures as release-blocking evidence.

## Authoritative visual references

The following rendered references are the visual source of truth:

- `target/visual-qa/reference-integrated-content-browser-17-1440x900.png`
- `target/visual-qa/reference-integrated-radial-reference-21-1440x900.png`
- `target/visual-qa/reference-integrated-menus-dialogs-20-1440x900.png`
- `target/visual-qa/reference-integrated-preview-compare-14-1440x900.png`
- `target/visual-qa/reference-integrated-search-tasks-18-1440x900.png`
- the corresponding `1024x720` variants

The current product thumbnails and fixtures remain the content assets. No new
decorative raster assets are introduced. UI symbols use the existing Viewer
symbol model and platform text rendering.

## Product constraints

- Preserve the white-first, quiet, cross-platform visual language approved for
  both macOS and Windows.
- Preserve file-operation semantics, safety confirmation, shortcuts, selection
  rules, preview behavior, compare limits, markers, and project access rules.
- Do not add a permanent command bar or restore removed toolbar buttons.
- Do not create a second command taxonomy for alternate input methods.
- Keep every command keyboard reachable and screen-reader named.
- Treat reduced motion, high zoom, narrow windows, and viewport edge fitting as
  first-class states.

## Unified radial command surface

### One visible menu

Every file-action invocation opens `RadialFileMenu`:

- ordinary right click;
- macOS Control-click;
- press-and-release of the secondary pointer button;
- held or moved secondary pointer gesture;
- the keyboard Context Menu key or `Shift+F10`.

`FileContextMenu` is removed from the rendered application. The radial menu is
the only visible file-action menu.

### Click and gesture modes

- A normal secondary click opens the radial menu in click mode at the pointer
  location. It remains open until a command, center cancel, Escape, or an
  outside click closes it.
- Holding the secondary button for 180 ms or moving at least 8 px promotes the
  same session to gesture mode.
- Gesture mode highlights the sector under the pointer and executes a valid
  leaf command on release.
- Releasing without crossing the movement threshold converts the already-open
  radial menu to click mode. It never substitutes a rectangular menu.
- Native WebKit context menus are prevented on Viewer-owned file cells.

### Keyboard mode

The Context Menu key and `Shift+F10` open the radial menu at the active file's
visible center. Arrow keys cycle enabled sectors, Enter or Space activates,
Escape closes, and focus returns to the invoking file.

### Visual treatment

- The menu retains six independent 60-degree annular primary sectors, the
  established clockwise order, and outward 30-degree secondary sectors.
- Primary geometry remains 42 px inner radius and 108 px outer radius.
- Secondary geometry remains 112 px inner radius and 168 px outer radius.
- A viewport-filling quiet scrim separates the command layer from the
  workspace without hiding file context.
- The complete radial assembly is fitted inside an 8 px viewport safe area.
- Resting sectors are warm white with a one-pixel neutral divider.
- Hover, pointer, and keyboard-active sectors use the approved indigo-soft
  fill and indigo boundary.
- The center is a distinct circle containing the selected file count and a
  concise `中心取消` hint.
- Trash remains the only destructive-colored primary label.
- Disabled commands remain readable, expose their reason, and never look
  selected.

## Thumbnail selection and focus

> 2026-08-06 update: the selection-boundary rules below are replaced by
> `2026-08-06-viewer-square-image-card-selection-design.md`. The radial-menu,
> focus, toolbar, launch, and all other corrections in this document remain
> authoritative.

### Selection boundary

- Content-grid file cards and their selection boundary use square corners.
- The boundary is 2 px solid `var(--viewer-accent)`, inset zero from the
  complete file-card edge, with zero radius.
- The boundary is an overlay and does not affect measurement, virtualization,
  filename layout, or scrolling.
- The boundary encloses the thumbnail and filename area; the thumbnail region
  does not receive a second selection boundary.
- The complete card receives no selection shadow.
- No opaque tint covers the product image.

### Focus boundary

Selection and keyboard focus are separate states:

- keyboard focus uses the shared focus outline outside the item with the
  standard focus offset;
- focus does not change `aria-selected`;
- a focused selected item may display both the outer focus outline and the
  complete-card selection boundary.

### Organization handle and summary

- The organization handle appears on hover, keyboard focus, or active pointer
  organization. It is not permanently visible only because an item is
  selected.
- The floating summary remains a compact count capsule.
- Supporting copy becomes `右键打开圆盘菜单 · Esc 取消选择`.
- The summary must not overlap the bottom row or other-files disclosure.

## Toolbar popovers

`筛选`, `视图`, and `更多` remain the only persistent controls after search.

### View and More menus

- The popover is one white surface with one border, one shadow, and 6 px inner
  padding.
- Commands are full-width, left-aligned rows with no resting border and a
  minimum height of 32 px.
- Hover and keyboard focus use the soft surface; active choices use the
  indigo-soft surface and a small semantic state mark.
- Related rows are separated by a 1 px divider with 6 px vertical margin.
- The divider is never rendered as an unstyled dot.
- Destructive project close uses danger text only on hover/focus or when the
  action is imminent; it is not styled as a primary button.
- Escape closes and returns focus to the trigger.

The filter popover keeps its more complex field layout, but its command rows
consume the same menu-row tokens.

## Task feedback

- Clean successful tasks remain visible for no more than 2 seconds.
- Multiple running tasks occupy one compact bottom-right surface, not multiple
  detached cards.
- The collapsed surface shows the most relevant task label, aggregate count,
  and one thin progress line.
- Expanding reveals individual task rows and existing failure details.
- Failed, cancelled, and result-bearing tasks persist until dismissed.
- Successful task rows do not display a permanent `关闭任务` button during the
  short success interval.

## Preview and compare chrome

- Preview and compare keep the approved three-zone toolbar: identity on the
  left, transforms in the center, completion/actions on the right.
- Related transform controls share one segmented control surface instead of
  appearing as unrelated bordered buttons.
- The closing action is visibly labeled `完成`; it is not an `×` glyph.
- Rotate uses the existing accessible action but receives the same quiet
  icon-button treatment as the transform group.
- Navigation remains a compact floating capsule.
- Preview, compare, text preview, unsupported preview, and information overlay
  share the same border, type, spacing, and focus tokens.

## Development preview integrity

The only supported way to launch a review build is `pnpm start:viewer`.

The launcher must:

- stop prior repository development Viewer processes;
- stop any running `Viewer.app/.../viewer-desktop` process before spawning;
- start from the current worktree;
- report branch, commit, and dirty state;
- wait for the exact worktree development executable;
- never launch `target/debug/bundle/macos/Viewer.app`.

Native QA must confirm that exactly one Viewer executable is running and that
its path is the current worktree's `target/debug/viewer-desktop`.

## Visual acceptance matrix

Each state is captured at `1440x900` and `1024x720` where the state is
available:

1. no project;
2. launch/loading;
3. content browser, no selection;
4. single selection and keyboard-focused selection;
5. multi-selection;
6. radial click mode;
7. radial gesture mode and secondary sectors;
8. filter, view, and more popovers;
9. settings;
10. running, successful, failed, and result-bearing tasks;
11. image preview;
12. compare;
13. text and unsupported preview;
14. information overlay;
15. rename, destination, trash, close-blocked, and other destructive dialogs;
16. operation results and global notices.

For each state, the reference and current capture must be opened in one
comparison input. P0, P1, and P2 mismatches block completion. Remaining P3
polish is recorded but does not block.

## Automated acceptance

- A real secondary-click sequence produces `.radial-file-menu` with no
  `.file-context-menu`.
- Keyboard invocation opens the radial at the active item and restores focus.
- Pointer dwell and movement preserve gesture behavior.
- Selected image cells expose `aria-selected="true"` and the complete card owns
  the visual selection overlay.
- Workspace popover command rows have no resting border.
- The More divider has a named class and correct geometry.
- Clean completed tasks auto-hide and do not offer a close button.
- Preview and compare completion controls contain visible `完成`.
- Existing accessibility, controller, build, Rust, and security gates remain
  green.
