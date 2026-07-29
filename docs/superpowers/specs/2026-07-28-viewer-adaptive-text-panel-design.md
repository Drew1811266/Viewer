# Viewer Adaptive Text Panel Design

Date: 2026-07-28
Status: User-approved design; written review pending

## Problem

Viewer always renders a `文本文件` heading and text-file list after the image
grid, even when the current content folder contains no text files. The image
grid also receives a fixed `520px` viewport height from `ContentBrowser`.

Those two behaviors create the reported failure:

- an empty text section remains visible in image-only folders;
- the image grid cannot reclaim the unused window height;
- the empty area looks like a large reserved text pane;
- a user must scroll inside a comparatively small image viewport while much of
  the application window appears unused.

Hiding only the empty heading would not solve the problem. The content browser
must become a height-aware layout in which the image viewport receives all
space not currently needed by a real text panel.

## Goals

- Remove the complete text-panel surface when the current folder has no text
  files.
- Let the image grid use the full remaining workspace height instead of a
  fixed `520px` viewport.
- Keep text files discoverable without allowing them to dominate mixed
  image-and-text folders.
- Use a compact, collapsed text bar by default in mixed folders.
- Expand the text panel below the image grid without covering images.
- Size an expanded mixed-folder panel from its content, capped at 20% of the
  available content height.
- Give a text-only folder the full content area automatically.
- Preserve image scroll position, selection and loaded thumbnail cache across
  text-panel state changes.
- Preserve the existing text-row preview, selection, marker, radial-menu and
  drag behavior.
- Make the mixed-file `全选当前文件夹` command explicit about which file types
  will be selected.
- Keep resizing and virtualization work bounded.

## Non-goals

- Relating text files to images by filename.
- Changing Markdown or TXT preview behavior.
- Adding a separate text search, sort or filter system.
- Adding a user-resizable splitter.
- Persisting panel state across application launches or project sessions.
- Adding a new third-party layout, popover or virtualization dependency.
- Redesigning image cards, text rows, the radial menu or the main toolbar.
- Virtualizing the text list in this change; its rendering behavior remains as
  it is today.
- Changing selection semantics beyond what is required to prevent hidden text
  rows from owning keyboard focus and to support the new select-all choices.

## Chosen Direction

The selected direction is a smart bottom text shelf.

The rejected alternatives are:

1. **Always-visible text list.** Simple, but it preserves the reported space
   waste and continues to give secondary content excessive visual weight.
2. **Separate image and text views.** It maximizes each view's space, but adds
   mode switching and prevents users from seeing both file types together.
3. **Smart bottom shelf.** It hides when irrelevant, stays compact by default,
   and expands only when the user needs it. This is the approved direction.

## Content Modes

`ContentBrowser` derives one of four display modes from the current files and
the user's mixed-folder preference:

| Images | Text files | Mode | Result |
| --- | --- | --- | --- |
| yes | no | `image_only` | No text shelf; image grid fills the content body. |
| yes | yes, preference collapsed | `mixed_collapsed` | Image grid plus one compact bottom bar. |
| yes | yes, preference expanded | `mixed_expanded` | Image grid plus an adaptive expanded text shelf. |
| no | yes | `text_only` | Text shelf is forced open and fills the content body. |

The existing empty workspace remains responsible for the case in which both
collections are empty.

### Image-only mode

- The text heading, toggle and list do not mount.
- The image viewport receives every available pixel below the content toolbar.
- There is no decorative separator or empty placeholder for text files.

### Mixed collapsed mode

- A compact bar is pinned below the image viewport.
- The bar displays a disclosure indicator and `文本文件 · N`.
- If selected text files are hidden by the collapsed shelf, the bar also
  displays `已选 N`.
- The complete bar is an interactive disclosure button.
- The image grid receives the remaining height above the bar.

### Mixed expanded mode

- Activating the disclosure bar expands the shelf below the image grid.
- The shelf participates in layout and reduces the image viewport height. It
  never overlays or covers an image.
- Total shelf height is the smaller of:
  - the intrinsic height required by the shelf heading and all text rows;
  - 20% of the available content-body height.
- Rows beyond that height scroll inside the text list.
- The 20% limit is strict. At unusually short window heights, the disclosure
  remains operable and the list scrolls within whatever space remains; the
  panel does not exceed the cap to force a complete row.

### Text-only mode

- The shelf is forced open because text files are the only browsable content.
- The 20% mixed-folder cap does not apply.
- The text list uses all space below the content toolbar and scrolls
  internally.
- Entering this mode does not overwrite the user's mixed-folder preference.

## Expansion Preference

- A newly opened project session starts with the mixed-folder preference set
  to collapsed.
- A manual expand or collapse changes that preference for the current project
  session.
- The preference survives content-folder and category navigation within that
  project.
- A folder with no text files temporarily hides the shelf without changing the
  preference.
- A later mixed folder restores the stored preference.
- A text-only folder forces the visible state to expanded without changing the
  stored preference.
- Closing or replacing the project resets the preference to collapsed.

The preference therefore represents user intent, while the visible mode is a
pure derivation of preference plus current content.

## Layout Architecture

### Content browser shell

The content surface becomes a full-height layout with:

1. an intrinsic-height content toolbar;
2. a `minmax(0, 1fr)` image viewport when images exist;
3. an intrinsic or capped text shelf when text files exist.

The content surface and its parents must propagate a bounded height and
`min-height: 0`, so child scrollers can consume remaining space without making
the outer workspace reserve an artificial blank region.

### Measured image viewport

`AspectVirtualGrid` already requires a numeric viewport height for visible-row
calculation. `ContentBrowser` will measure the actual image slot rather than
supplying the current `520px` default.

- A `ResizeObserver` observes the image slot.
- Observations are coalesced to one update per animation frame.
- Non-finite, zero and unchanged measurements are ignored.
- The latest valid height is passed to `AspectVirtualGrid`.
- The image grid remains mounted while the shelf expands or collapses.
- A no-`ResizeObserver` fallback reads the slot's current bounding rectangle
  and uses the existing safe default only until a valid measurement exists.

Keeping the same grid instance preserves DOM scroll position, recovered image
dimensions, pending work and thumbnail cache. A height change only recalculates
the bounded visible window.

### Text shelf component

The text disclosure and list move into a focused component responsible for:

- collapsed and expanded presentation;
- `aria-expanded` and controlled-region relationships;
- the selected-text count in collapsed mode;
- the 20% mixed-folder height cap;
- internal text-list scrolling;
- disclosure focus and Escape behavior;
- the existing text-row rendering and events.

`ContentBrowser` continues to own file selection and supplies the text shelf
with selected state and existing callbacks.

### Session state owner

The mixed-folder expansion preference lives above `ContentBrowser` at the
active-project session level. This prevents category navigation or temporary
view changes from resetting it. A project/session identity change resets it.

The component receives:

- `textPanelExpanded`;
- `onTextPanelExpandedChange`.

The current content mode remains derived inside the content-browser boundary
from image count, text count and this preference.

## Disclosure Interaction And Accessibility

- The full collapsed bar is a native button.
- `Enter`, Space and pointer activation toggle mixed-folder expansion.
- The button exposes `aria-expanded` and `aria-controls`.
- Escape from the expanded text list collapses a mixed shelf and returns focus
  to the disclosure button.
- Text-only mode has a non-collapsible heading because its list is the primary
  view.
- When collapsed, image-grid arrow navigation cannot move active focus into an
  unmounted text row.
- If the previously active item is a text file when the shelf collapses, its
  selection remains, but no visible listbox exposes an `aria-activedescendant`
  that references the hidden option.
- Existing text-row click, Command-click, Shift selection, double-click
  preview, radial-menu request, Finder drag and organization drag behavior
  remains available when the shelf is expanded.

## Select-All Choice Panel

The existing `全选当前文件夹` action uses file-type-aware behavior.

### Single-type folders

- Images only: immediately select every image; no prompt appears.
- Text only: immediately select every text file; no prompt appears.
- Empty: the action remains disabled through the existing empty-workspace
  behavior.

### Mixed folders

Activating `全选当前文件夹` opens a compact, non-modal choice panel anchored to
the action. It offers three explicit commands:

1. `全选图片`
2. `全选文本文件`
3. `全部选择`

The panel:

- receives keyboard focus when it opens;
- supports pointer, Tab and arrow-key navigation;
- closes after a command;
- closes without changing selection on Escape, outside click, reactivating the
  source button, folder navigation, preview entry or comparison entry;
- returns focus to the source button after cancellation;
- never forces the text shelf open.

If a command selects hidden text files, the collapsed disclosure bar reports
their selected count.

## Data Flow

1. The folder workspace supplies independent `images` and `textFiles`
   collections.
2. Project-session state supplies the user's mixed-folder expansion
   preference.
3. `ContentBrowser` derives the visible content mode.
4. The layout assigns remaining height to the mounted image slot and text
   shelf.
5. The measured image-slot height drives the existing virtual-grid calculation.
6. Text disclosure changes only the session preference; it does not mutate
   workspace data.
7. Select-all choices reuse the existing selection commit path with one of
   three explicit file sets.

No backend, index, scan or IPC contract changes are required.

## Dynamic And Error States

- If a watcher refresh removes the last text file, the shelf unmounts
  immediately and the image slot reclaims its height.
- If text files later reappear, the mixed mode restores the session preference.
- If images disappear while text remains, text-only mode takes the full body.
- If images later return, mixed mode restores the session preference.
- A choice panel opened for mixed content closes if the file collections
  change before the user chooses; it never acts on a stale snapshot.
- Invalid or missing measurements retain the last valid image height and do
  not collapse the grid to zero.
- Rapid observer notifications and shelf transitions are coalesced and cannot
  create an unbounded render loop.
- Existing thumbnail failures, text-preview failures and file-operation errors
  keep their current reporting paths.

## Performance Requirements

- Shelf toggling must not remount `AspectVirtualGrid`.
- Existing cached thumbnail URLs and recovered image dimensions remain live.
- A height-only change must not restart already pending thumbnail requests.
- Resize observations publish at most once per animation frame and only when
  the effective pixel height changes.
- Visible image mounting stays within the existing virtual-grid overscan
  bounds.
- The select-all choice panel mounts only while open.
- No new third-party runtime dependency is added.

## Test And Acceptance Matrix

### Layout

- Image-only folder: no `文本文件` control or list exists; the image slot
  reaches the bottom content boundary.
- Mixed folder, initial project state: one collapsed disclosure bar is visible.
- Mixed folder, expanded: shelf content height is used until the 20% cap.
- Mixed folder with many text files: the shelf stays within 20% and its list
  scrolls internally.
- Text-only folder: no empty image slot is reserved; the text list fills the
  content body.
- Short window: the toolbar and disclosure remain usable, the text list stays
  contained, and the strict 20% cap is preserved.

### State transitions

- Expand in one mixed folder, navigate to another mixed folder, and remain
  expanded.
- Navigate through a no-text folder and restore the previous preference on the
  next mixed folder.
- Enter and leave text-only mode without mutating the mixed preference.
- Close or replace the project and return to the default collapsed preference.
- Dynamically add and remove the last text or image file without stale layout.

### Preservation

- Toggle the shelf without changing image scroll position.
- Toggle the shelf without remounting visible image cells.
- Toggle the shelf without duplicating pending thumbnail requests.
- Preserve image and text selection across valid shelf transitions.

### Select all

- Mixed folder opens the anchored three-choice panel.
- Each choice selects exactly its named file set.
- Escape, outside click and source-button reactivation preserve the prior
  selection.
- Image-only and text-only folders select directly without opening the panel.
- A collapsed shelf reports selected hidden text count and stays collapsed.
- Navigation or content refresh closes the panel before stale actions can run.

### Keyboard and accessibility

- Disclosure supports pointer, Enter and Space.
- Expanded mixed shelf supports Escape and focus return.
- Choice panel has a deterministic opening focus and keyboard order.
- Collapsed state has no hidden option referenced by `aria-activedescendant`.
- Text-only mode exposes a labelled text list without a misleading collapsible
  control.

### Regression

- Existing text preview, marker, radial-menu, Finder drag and organization drag
  tests remain green.
- Existing image-grid selection, marquee, thumbnail virtualization and
  keyboard tests remain green.
- UI type-check, lint, unit tests and production build pass.

## Documentation Impact

The browsing requirement in `docs/PRODUCT_SPEC.md` should change from an
unconditional independent text region to the adaptive behavior:

- text files remain an independent list;
- image-only folders render no text surface;
- mixed folders use the collapsed smart shelf;
- text-only folders use the full content body.
