# Viewer Aspect-Aware Thumbnail Density Design

Date: 2026-07-27
Status: Approved

## Problem

Viewer currently renders folder-filmstrip thumbnails in fixed `132 × 132` CSS
pixel cells with `object-fit: contain`. Portrait images therefore expose
container background on the left and right, while landscape images expose it
above and below. The normal content grid also assumes fixed square cells.

Those bands are layout waste rather than image content. They reduce the number
of useful pixels on screen and become especially visible when one folder
contains images with several aspect ratios.

The application must remove only container-generated bands. White pixels,
transparent pixels, and other background that belongs to the source image must
remain unchanged. Viewer must not crop, stretch, or content-analyze an image to
solve this problem.

## Goals

- Render image surfaces at their orientation-corrected source aspect ratio.
- Keep every image in a folder filmstrip at one common height while allowing
  each image to have its natural proportional width.
- Render the normal content image grid as fixed-height, variable-width rows in
  stable source order.
- Provide compact, standard, and large global thumbnail-density settings.
- Apply a density change immediately to both image surfaces and persist it
  outside every project.
- Preserve bounded rendering, selection, keyboard access, marquee selection,
  preview, drag, context-menu, and focus behavior.
- Keep the feature inside continuous development governance; it creates no new
  milestone, delivery target, or acceptance phase.

## Non-goals

- Cropping source pixels or detecting/removing white backgrounds.
- Restricting panoramic or extremely narrow image widths.
- Changing search-result, compare, or full-image-preview sizing.
- Remembering a different density per project or per folder.
- Adding settings to the no-project landing surface.
- Retaining a second, local content-grid size override.
- Adding a third-party layout or virtualization dependency.

## Approved User Experience

After a project opens, the workspace header adds a dedicated settings button
beside the existing `•••` project menu. The settings button has the accessible
name `软件设置`. It does not replace or absorb project-scoped actions. The
no-project landing surface does not show this button.

Activating the button opens a centered modal titled `软件设置`. The first
version is a single page with a `显示` section and one `缩略图密度` setting:

| Density | Image height |
| --- | ---: |
| `compact` / 紧凑 | 96 CSS px |
| `standard` / 标准 | 132 CSS px |
| `large` / 大图 | 168 CSS px |

`standard` is the first-run and recovery default. The three choices form one
keyboard-operable radio group.

Selecting a density immediately updates the folder filmstrips and normal image
grid behind the modal and starts persistence. There is no Save or Apply
button. Closing the modal does not undo a successfully selected value. The
modal has an explicit close button, closes with Escape, traps focus while open,
and returns focus to the settings button when it closes.

The normal content view's current local `视图 → 缩略图大小` control is removed.
Its other view actions remain available.

## Aspect-Ratio Source

Layout prefers `BrowserFile.imageMetadata`. The dimensions already represent
the displayed orientation after EXIF correction. A usable aspect ratio requires
finite, positive width and height values and a finite, positive quotient.

For a selected image height `H`, source width `W`, and source height `I`:

```text
displayWidth = H × W ÷ I
```

Geometry retains fractional CSS pixels. It must not round every width to an
integer before accumulating offsets. The image element and its visible
container use the same ratio, so `object-fit: contain` has no container area in
which to paint a top/bottom or left/right band.

The image remains complete for every valid ratio. Panoramas may become very
wide and extreme portrait images may become very narrow. The layout does not
introduce a minimum or maximum display width.

### Dimensions not yet available

An image with missing or invalid metadata first occupies a square skeleton
whose side is the selected height. Viewer does not paint the image into that
fallback cell.

When the thumbnail loads, the component reads its positive natural dimensions,
records the recovered ratio for that entity and modification identity, and
recomputes geometry. Only then does it reveal the image. A failed thumbnail
keeps an error placeholder. This permits one controlled reflow without ever
showing a loaded image inside the wrong aspect-ratio container.

## Folder Filmstrip Layout

Every filmstrip image uses the selected global image height. The row keeps its
existing horizontal scrolling behavior, source order, lazy folder request,
preview context, and focus restoration.

The fixed `THUMBNAIL_STRIDE` model is replaced by an aspect-aware geometry
table. For image `i`:

```text
width[i] = displayWidth(height, ratio[i])
offset[0] = 0
offset[i + 1] = offset[i] + width[i] + gap
```

The track width is the last occupied edge plus inline padding. Horizontal
visibility uses binary search over the monotonic offset/end arrays, then mounts
the visible interval plus a small cell and pixel overscan. A focused item
outside that interval remains mounted until focus moves, preserving the current
accessibility invariant.

Changing density or recovering a ratio rebuilds the table. The filmstrip
preserves the first visible entity as an anchor and adjusts `scrollLeft` to
keep that entity at the same visual offset where possible.

The row's image area is `H` high. Existing vertical padding remains outside the
image containers, and the folder identity panel stretches with the row.

## Normal Content Grid Layout

The normal image grid becomes a fixed-height, variable-width flow. It preserves
source order and uses one selected height for every image row.

Given the available content width:

1. Start a row at the inline origin.
2. Place the next proportional item if it fits or if the row is empty.
3. Otherwise start the next row and place the item there.
4. Keep the final row left-aligned.

Unused space at a row's end is workspace background, not part of an image
container. Rows are not justified by scaling and images are not stretched.

An image wider than the available content area begins a row by itself and keeps
its natural proportional width. The image viewport allows horizontal access
instead of cropping or adding letterbox bands.

The geometry engine returns a rectangle and row identity for every item, the
total scroll extent, monotonic row boundaries, and entity lookup maps.
Vertical virtualization binary-searches row boundaries and mounts only visible
rows plus bounded overscan.

The new proportional image grid owns variable geometry rather than forcing the
existing fixed-cell `VirtualGrid` to support two incompatible models. Shared
selection and marquee primitives remain reusable.

### Interaction preservation

- Selection and Shift ranges continue to use stable source order and entity
  IDs rather than mounted DOM order.
- Marquee hit testing intersects the selection rectangle with geometry-table
  item rectangles, including items outside the mounted DOM interval.
- Left and Right follow source order. Up and Down select the nearest horizontal
  center in the adjacent geometry row.
- A density or viewport change preserves selected IDs, active ID, preview
  context, and drag state. The first visible entity is used as the vertical
  scroll anchor.
- Focused off-window content remains mounted until focus moves.
- Preview, Finder export, organization drag, and radial-menu requests continue
  to carry entity IDs only.

Image cells retain their existing metadata/caption area below the proportional
image surface. The selected height controls the image surface; the caption
height remains constant.

## Density-Aware Thumbnail Requests

The requested representation uses the rendered long edge and
`window.devicePixelRatio` to avoid making the large density visibly soft on
Retina displays. Request dimensions are validated and capped by the existing
thumbnail/decode safety limits. A cap may reduce sampling sharpness for an
extreme panorama but never changes its display geometry.

The existing request cache identity continues to include entity identity,
modification identity, requested pixel bound, and scale. A density change may
reuse a sufficient representation or request a more appropriate one without
copying source images or sending bytes through JSON IPC.

## Settings Architecture

The setting is a presentation preference, not project metadata. The version-one
serialized form is:

```json
{
  "schemaVersion": 1,
  "thumbnailDensity": "standard"
}
```

Responsibilities are separated as follows:

- The Rust application layer defines the typed value, the three allowed
  densities, the default, and validation.
- The infrastructure layer implements a JSON-backed settings repository in the
  Tauri-resolved Viewer application configuration directory.
- The desktop layer owns the resolved directory and exposes narrow commands to
  read settings and update only thumbnail density.
- The UI owns modal state, the optimistic selected value, and presentation.
- A `ViewerSettingsProvider` loads the global value once and supplies it to the
  modal, `FolderFilmstripRow`, and the normal content image grid.
- A pure TypeScript aspect-layout module owns ratio validation, filmstrip
  offsets, flow rows, item rectangles, visibility windows, and scroll anchors.

The webview cannot supply a path, arbitrary JSON, or an arbitrary setting key.
No broad Tauri filesystem capability is added.

## Persistence and Concurrency

The repository writes a temporary file in the owned configuration directory,
flushes it, and atomically replaces the settings file. Writes are serialized.
The project directory and `.viewer` are never read or written for this setting.

The UI tracks the last confirmed value, the desired value, and an update
sequence:

1. A click updates the desired value immediately.
2. Layout consumers render that value.
3. The UI submits the typed update.
4. Only the result for the latest desired sequence may confirm or revert the
   visible value.

Rapid selections are processed in order and the last selection wins. An older
completion cannot overwrite a newer choice.

## Error Handling

- A missing settings file yields `standard`.
- Invalid JSON, invalid fields, invalid density, or an unknown schema version
  yields `standard` without blocking project opening.
- Invalid content is not exposed to the UI and is replaced the next time a
  valid user update succeeds.
- If the latest persistence attempt fails, the UI restores the last confirmed
  value and displays `设置未能保存` inside the modal.
- Error DTOs do not contain absolute configuration or project paths.
- Missing, zero, overflowing, or non-finite image geometry follows the
  placeholder/recovery path and cannot poison offsets or scroll extents.

## Accessibility and Appearance

- The settings trigger is a real button with an accessible name and visible
  light/dark focus treatment.
- The modal uses dialog semantics, an accessible title, focus trapping,
  Escape handling, an explicit close button, and trigger focus restoration.
- Density options use radio-group semantics and support arrow-key selection.
- Filmstrip thumbnails and normal image cells retain their current accessible
  names, roles, selection state, and preview actions.
- Container backgrounds may remain for skeletons and error states but are not
  visible around a successfully loaded image with known dimensions.
- Source white and transparent pixels remain visible and are never classified
  as removable layout bands.

## Testing

### Pure layout tests

- Exact heights for all three densities.
- Portrait, square, landscape, panorama, extreme portrait, and
  orientation-corrected dimensions.
- Fractional widths and cumulative offsets without progressive rounding drift.
- Fixed-height source-order wrapping and left-aligned final rows.
- A single item wider than the viewport.
- Missing, zero, overflowing, and recovered natural dimensions.
- Horizontal and vertical visible windows with bounded overscan.
- Scroll-anchor reconstruction after density, ratio, and viewport changes.

### Component and interaction tests

- Mixed-ratio filmstrips show proportional widths with no container bands.
- High-cardinality filmstrips preserve total width, source order, bounded
  mounting, focus retention, and preview context.
- The proportional content grid preserves click, Command, Shift, marquee,
  keyboard, preview, drag, Finder export, and radial-menu behavior.
- Density changes preserve selection, active entity, preview context, and
  scroll anchor.
- Extreme-width content remains reachable without cropping.
- Skeleton, recovered-ratio, thumbnail-failure, and retry states are isolated.

### Settings tests

- First-run default and successful reload after restart.
- Immediate propagation to both image surfaces.
- Rapid updates with last-choice-wins ordering.
- Missing, corrupt, invalid, unknown-version, and failed atomic-write cases.
- Latest-save failure rollback and safe error text.
- No settings artifacts are written below a project root.
- The local content-grid size control is absent.
- The trigger, dialog, radio group, Escape behavior, focus trap, and focus
  restoration are keyboard accessible.

### Repository verification

The complete repository verification command must pass. Layout and settings
checks remain continuous engineering tests and do not create an M4 stage,
Viewer 0.1 delivery goal, or other release gate.
