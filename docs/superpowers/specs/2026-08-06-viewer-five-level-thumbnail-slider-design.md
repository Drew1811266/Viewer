# Viewer Five-Level Thumbnail Slider Design

Date: 2026-08-06
Status: Approved

## Relationship to the Existing Thumbnail Design

This specification amends only the global thumbnail-size setting defined by
`2026-07-27-viewer-aspect-aware-thumbnail-density-design.md`. Its aspect-ratio
layout, virtualization, selection, preview, drag, caching, safety and project
isolation decisions remain active.

## Problem

Viewer currently exposes three global thumbnail choices as radio cards labelled
`紧凑`, `标准` and `大图`. The existing maximum image-surface height of 168 CSS
pixels is not large enough for users who want to inspect more detail while
browsing. The three wide radio cards also consume unnecessary settings space
and describe density indirectly when the user is actually choosing image size.

Viewer needs two larger sizes and one compact, cross-platform control that makes
the five ordered levels obvious without retaining the Chinese size names.

## Goals

- Preserve the current minimum and the three existing image-surface heights.
- Add two larger thumbnail levels, ending at 240 CSS pixels.
- Replace the three radio cards with one discrete five-level slider.
- Present the choices as the visible numbers `1` through `5` only.
- Apply each selected level immediately and persist it globally.
- Load every existing version-one settings file without changing its selected
  compact, standard or large value.
- Keep the control visually consistent in the macOS WebView and the future
  Windows WebView2 build.
- Preserve the existing bounded geometry, request and cache behavior at the two
  new sizes.

## Non-goals

- Continuous, pixel-by-pixel thumbnail resizing.
- Per-project or per-folder thumbnail sizes.
- Changing caption height, file metadata layout or image aspect-ratio behavior.
- Changing search-result, compare or full-image-preview sizing.
- Adding a sixth level or allowing custom numeric input.
- Redesigning other settings categories or dialog navigation.
- Changing the Viewer settings file location or granting broader filesystem
  access.

## Approved Level Mapping

The slider uses five fixed stops:

| Visible level | Internal value | Image-surface height |
| ---: | --- | ---: |
| 1 | `compact` | 96 CSS px |
| 2 | `standard` | 132 CSS px |
| 3 | `large` | 168 CSS px |
| 4 | `extra_large` | 204 CSS px |
| 5 | `maximum` | 240 CSS px |

The increments remain 36 CSS pixels. `standard` remains the first-run and
recovery default, so a new or invalid settings file opens at visible level 2.

The UI owns pure level-to-density and density-to-level mappings. Layout and IPC
consumers continue using the typed density value rather than an unchecked
number.

## Settings Interaction

The settings field is renamed from `缩略图密度` to `缩略图大小`.

The three radio cards are removed. Their replacement is one semantic range
input with:

- minimum `1`;
- maximum `5`;
- step `1`;
- the current level as its value;
- five evenly spaced visible labels: `1 2 3 4 5`.

No user-facing `紧凑`, `标准` or `大图` label remains in this setting. The
numbers stay visible at every level; the current number receives the accent
color and stronger weight. The slider thumb aligns exactly with the five number
centres.

The control supports pointer dragging, track clicking, Left/Right and Down/Up
arrow keys, and Home/End through native range-input semantics. Moving to a new
stop immediately updates both folder filmstrips and the normal content grid
behind the modal. There is no Save or Apply button, and closing the modal does
not undo a successful selection.

The slider receives initial focus when the settings dialog opens. The existing
close button, Escape behavior, focus trap and trigger-focus restoration remain
unchanged.

## Visual Contract

The input remains a real `input[type="range"]`; custom styling must not replace
it with a pointer-only simulated slider.

- The track is 4 CSS pixels high with quiet neutral unselected color and Viewer
  accent color through the selected stop.
- The visible thumb is circular, 18 CSS pixels in diameter and uses the Viewer
  accent color with a surface-coloured inner separation line.
- The slider row provides at least a 32 CSS pixel interaction height even though
  the visible track is thinner.
- The five numeric labels share the slider width and align with its stops.
- Hover, active and keyboard-focus states use the existing Viewer accent and
  focus tokens.
- Disabled appearance is not introduced because this setting is always
  available when the settings dialog is available.
- Reduced-motion mode removes any optional thumb or fill transition.
- Forced-colours mode delegates track and thumb contrast to system colours and
  retains a visible focus indicator.

The CSS styles both WebKit/Chromium slider pseudo-elements needed by the macOS
and future Windows shells. Platform-default shape and accent differences must
not cause the two builds to look like unrelated controls.

## Accessibility

The slider has the accessible name `缩略图大小`. Its numeric value remains
`1` through `5`, and `aria-valuetext` reports `档位 N，H 像素`, using the exact
height for the current stop.

The visible number row is presentational and does not create five additional
tab stops. The semantic slider is the only control. Pointer and keyboard changes
follow the same update path. Focus remains visible at every stop and is not
obscured by the thumb.

## Settings Compatibility and Persistence

The version-one JSON structure stays unchanged:

```json
{
  "schemaVersion": 1,
  "thumbnailDensity": "standard"
}
```

The Rust application enum and desktop DTO retain `Compact`, `Standard` and
`Large`, and add typed variants that serialize as `extra_large` and `maximum`.
Consequently:

- existing `compact`, `standard` and `large` files load at levels 1, 2 and 3;
- levels 4 and 5 persist as `extra_large` and `maximum`;
- no settings migration or project write occurs;
- malformed values still recover to level 2;
- an older Viewer binary may safely treat a later level-4/5 value as invalid and
  recover to its standard default after a downgrade.

The additive accepted-value change does not alter the JSON shape, so the schema
version remains 1. IPC continues exposing one narrow typed
`update_thumbnail_density` command; the WebView cannot submit an arbitrary
setting name or JSON object.

## Update Ordering and Error Handling

The existing optimistic settings flow remains:

1. The slider moves to a discrete stop.
2. The UI maps the level to a typed density and updates layout immediately.
3. The provider serializes the typed persistence request.
4. Only the latest desired sequence may confirm or revert the visible value.

A full drag can cross at most four intermediate stops. Those typed writes may be
serialized by the existing queue; the last selected stop wins. An older
completion cannot overwrite a later choice.

If the latest write fails, Viewer restores the last confirmed level, moves the
slider and content layout back together, and displays the existing safe
`无法保存设置` feedback. Error payloads expose no configuration or project path.

## Layout, Requests and Performance

`THUMBNAIL_HEIGHT` expands to all five typed densities. Level 5 uses a 240 CSS
pixel image height while the caption height remains unchanged. Existing
aspect-aware geometry, row wrapping, scroll anchoring, virtualization and
selection identities consume that value without a second sizing model.

At the existing maximum device scale of 4, a level-5 ordinary long edge requests
at most 960 physical pixels before panoramic geometry and the existing 4096-pixel
safety cap are considered. The two new levels therefore require no larger decode
cap or new cache key. Larger thumbnails naturally produce fewer items per row;
mounted rows and overscan remain bounded.

## Product and Visual-Atlas Coverage

The formal visual atlas and the product must show the same five-level slider in
the settings state. Existing density acceptance states retain their stable IDs
and map to levels 1, 2 and 3. Two additional acceptance states cover levels 4
and 5 without reusing selection-state identifiers.

The settings screenshot/state must prove the slider itself, all five visible
numbers, current-stop emphasis and keyboard focus. Content screenshots must
prove the actual 204- and 240-pixel image surfaces at both supported acceptance
viewports.

README and current product documentation describe five numbered global
thumbnail sizes instead of the former three named choices. Historical review
evidence keeps its original wording and is not rewritten.

## Testing

### Mapping and UI tests

- Exact bidirectional mapping for all five levels and typed densities.
- Exact heights `96`, `132`, `168`, `204` and `240`.
- Slider min, max, step, current value, visible number labels and
  `aria-valuetext`.
- Pointer/keyboard changes call the same typed update path.
- Initial focus, Escape close, focus trap and focus restoration remain intact.
- No radio inputs or user-facing `紧凑`, `标准`, `大图` labels remain in the
  settings control.
- Latest-save failure restores both slider and content layout to the last
  confirmed level.

### Persistence and IPC tests

- Existing version-one `compact`, `standard` and `large` JSON loads unchanged.
- `extra_large` and `maximum` round-trip through application, infrastructure,
  desktop DTO and TypeScript bridge boundaries.
- Missing, malformed, unknown and failed-write behavior remains safe.
- The settings command accepts only the five public values.

### Layout and request tests

- Filmstrip and content-grid geometry at levels 4 and 5.
- Density changes preserve selected IDs, active ID and scroll anchor.
- DPR-aware request bounds for 204- and 240-pixel surfaces.
- Mounted content remains bounded at the two new sizes.

### Visual acceptance

- Settings slider at 1024 × 720 and 1440 × 900.
- Resting content at levels 1 through 5, with dedicated evidence for levels 4
  and 5.
- Single selection at the maximum level retains the complete square card
  boundary and full neutral outline.
- Keyboard focus and forced-colours states remain distinct from selection.

The complete repository verification command must pass. This feature remains a
continuous-development change and creates no milestone or release gate.
