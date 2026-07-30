# Viewer Complete UI Visual Upgrade Design

Date: 2026-07-30  
Status: Approved

## Summary

Viewer will receive a complete presentation-layer redesign that makes the
desktop application feel quiet, precise, compact, and intentionally designed
without materially changing its established interaction model.

The approved direction is a light, predominantly white interface with
slightly warm neutral grays, restrained indigo accents, compact information
density, and very limited use of shadow. The same internal visual system will
be used by the current macOS application and the future Windows application.
Only operating-system-owned chrome and workflows retain platform-specific
appearance.

This design was approved through five visual review sections:

1. application shell, workspace density, grouping, and toolbar;
2. search and filtering;
3. single-image preview and multi-image comparison;
4. dialogs, empty states, errors, and read-only feedback;
5. visual tokens, accessibility states, and motion.

## Relationship to Earlier Designs

This design preserves the functional behavior defined by earlier Viewer
designs, including proportional thumbnails, folder filmstrips, preview modes,
comparison layouts, selection, search, file operations, radial menus, settings,
and read-only behavior.

Where an earlier specification defines a conflicting color, border, radius,
shadow, control arrangement, toolbar label, or other presentation detail, this
document supersedes that visual detail. It does not supersede the earlier
functional requirements, data flow, safety behavior, or accessibility
semantics.

Viewer remains intentionally light in this redesign even when the operating
system uses a dark appearance. A separate dark theme requires its own future
design and is not inferred by inverting these tokens.

## Problem

Viewer's current interface was assembled incrementally while its core
features and workflows were being established. The result is functionally
capable but visually uneven:

- project identity, search, settings, view controls, and project actions compete
  in one toolbar without a clear hierarchy;
- some screens repeat the current path or title even though the folder sidebar
  already communicates location;
- large panels and numerous bordered controls make the interface feel heavier
  than its content;
- folder rows, preview modes, comparison panes, dialogs, and feedback states
  do not yet share one complete visual system;
- shadows, borders, grays, active colors, and spacing are not governed by one
  set of application-wide tokens;
- macOS-specific styling would risk creating a visibly different future
  Windows version.

The redesign must improve perceived quality without disrupting users who
already understand Viewer.

## Goals

- Make the interface feel deliberate, minimal, and premium while keeping it
  efficient for large image projects.
- Use one visual system across browsing, search, preview, comparison, settings,
  dialogs, and feedback states.
- Keep the main application white and neutral, with content as the strongest
  visual element.
- Preserve current workflows, keyboard behavior, selection, preview,
  comparison, file operations, and state recovery.
- Reduce persistent toolbar controls to the fewest useful groups.
- Keep compact information density without sacrificing focus visibility or
  click targets.
- Establish concrete reusable tokens for implementation and future Windows
  work.
- Make local errors and low-frequency states visually calm and recoverable.

## Non-goals

- Redesigning Viewer workflows, navigation structure, or project data model.
- Changing search semantics, filter capability, sort behavior, result paging,
  selection rules, or comparison limits.
- Changing proportional thumbnail geometry, virtualization, image loading,
  image safety budgets, or file-operation semantics.
- Adding a user-selectable theme or a dark theme.
- Replacing native window controls, file pickers, permission settings, macOS
  Trash, or Windows Recycle Bin interactions.
- Making macOS and Windows window chrome look identical.
- Adding decorative illustrations, gradients, glass effects, or a new icon
  style solely for visual novelty.
- Building the future Windows application in this iteration.

## Design Principles

### Content is visually dominant

Large neutral surfaces stay quiet. Product images, filenames, selection, and
review state carry the meaning. Chrome does not compete through dark fills,
heavy borders, or repeated headings.

### Compact does not mean cramped

Toolbar controls use compact visual heights, information rows remain dense,
and whitespace is spent at structural boundaries rather than around every
item. Interactive targets and keyboard focus remain comfortably operable.

### Depth is earned

Background changes and one-pixel separators establish most hierarchy.
Shadows are reserved for interfaces that physically float above another
surface: popovers, menus, dialogs, and the image navigation control.

### Indigo is a state color

The approved accent `#5869CF` is used for selection, keyboard focus, active
controls, and low-frequency brand recognition. Review outcomes, warnings,
successes, and destructive actions retain their own semantic colors.

### Cross-platform consistency stops at system ownership

Viewer-owned surfaces, spacing, type scale, controls, and feedback are shared
between macOS and Windows. Window buttons, system dialogs, filesystem
terminology, permission UI, and platform shortcuts follow each operating
system.

## Visual Foundation

### Global color tokens

The implementation will introduce application-wide semantic variables and
derive preview-specific variables from them instead of maintaining a separate
preview palette.

| Token role | Value | Use |
| --- | --- | --- |
| Canvas | `#F3F3F1` | Outer preview canvases and neutral framing |
| Application | `#FAFAF8` | Main application background |
| Surface | `#FFFFFF` | Toolbars, content surfaces, menus, dialogs |
| Sidebar | `#EFF0EF` | Folder navigation and identity column |
| Soft surface | `#F5F5F3` | Search field, quiet controls, chips |
| Border | `#DEDFDD` | One-pixel separators and ordinary control borders |
| Primary text | `#23262C` | Titles and primary content |
| Secondary text | `#6B7077` | Descriptions and metadata |
| Tertiary text | `#8A8E94` | Placeholders and low-priority labels |
| Accent | `#5869CF` | Selection, focus, active state |
| Accent soft | `#ECEEFF` | Selected navigation and active controls |
| Success | `#397158` | Completed or successful states |
| Success soft | `#EFF7F2` | Success background |
| Warning | `#76591D` | Read-only and warning text |
| Warning soft | `#FAF6EB` | Read-only and warning background |
| Danger | `#B7463F` | Destructive action and error emphasis |
| Danger soft | `#FFF7F6` | Error background |

Review-state colors remain semantically distinct from the indigo accent.
Unmarked, keep, pending, reject, and favorite states must not be flattened into
one generic selected color.

### Typography

The interface font stack is:

```css
font-family:
  Inter,
  "SF Pro Text",
  "Segoe UI Variable Text",
  "PingFang SC",
  "Microsoft YaHei UI",
  sans-serif;
```

Inter provides consistent Latin letters and numeric metrics. Chinese text uses
the high-quality platform font so glyph rendering remains native and clear.
Viewer must not depend on the macOS-only San Francisco font for geometry.

The approved type scale is:

| Role | Size | Weight |
| --- | ---: | ---: |
| Page or major view title | 20 px | 700 |
| Section title | 14 px | 650 |
| Body and primary row content | 13 px | 400–500 |
| Caption, metadata, and secondary control text | 11 px | 500 |

Large folder identifiers may use a stronger weight but stay within the same
family. Letter spacing remains neutral except for small uppercase system labels.

### Spacing, radii, and sizing

All new spacing uses a 4 px base grid:

```text
4, 8, 12, 16, 24, 32
```

Approved radii are:

- 6 px for rows and small labels;
- 8 px for buttons, inputs, and compact controls;
- 12 px for popovers and floating panels;
- 14 px for dialogs;
- full pill radius only for compact semantic tags or counts.

Compact toolbar controls render at 28–32 px visual height. The effective pointer
target is at least 32 px. Larger primary actions may use 34–36 px height.

### Borders and shadows

- Ordinary structure uses a 1 px neutral border or separator.
- Selected content uses a one-pixel indigo border plus, where necessary, a soft
  two-pixel outer ring.
- Folder rows, image bands, sidebar rows, and ordinary content cards do not use
  shadows.
- Popovers use one restrained shadow approximately equivalent to
  `0 18px 48px rgb(31 35 42 / 17%)`.
- Dialogs may use a slightly deeper shadow approximately equivalent to
  `0 22px 60px rgb(31 35 42 / 19%)`.
- Preview images may use a restrained image-only shadow to separate white
  source pixels from the canvas.

## Application Shell

### Column-aligned header

The top header is divided according to the same two columns as the body:

- the project name anchors the folder-sidebar column;
- search and global workspace actions begin at the content-column boundary.

When the user resizes the existing sidebar between its current supported
limits, the project identity column follows that width. When the sidebar is
collapsed, the header follows the collapsed width. This alignment is visual;
sidebar resize, collapse, selection, and project behavior do not change.

The project name is shown once. It is not repeated as a large workspace title.

### Sidebar

The sidebar uses `#EFF0EF`, one right separator, and compact 24–28 px rows.
Selected navigation uses `#ECEEFF`, indigo text, and a two-pixel inset leading
indicator. Group labels use tertiary text rather than stronger borders.

Folder hierarchy, drag targets, resizing, collapse, keyboard navigation,
markers, and selection semantics remain unchanged.

### Removed duplicate workspace heading

The independent content heading and blank band previously shown above folder
overview rows are removed. The sidebar is the source of current-location
context.

The existing `显示全部后代文件` capability is not removed. It moves into the
contextual `视图` menu and appears only where descendant aggregation is
available. When aggregation is active, a small in-content state label may
remain; it must not recreate the removed full-width heading band.

### Folder overview rows

Folder overview uses the approved “quiet bands” treatment:

- each folder occupies one contiguous horizontal band;
- the identity and metadata area uses a pale neutral surface;
- image content flows directly beside it;
- rows are divided by hairline separators;
- rows do not float as cards and do not use individual shadows;
- proportional image geometry and current horizontal scrolling remain
  unchanged.

This treatment preserves high scanning density while clearly separating folder
identity from image content.

## Top Toolbar

### Persistent controls

After the search field, only three controls remain persistently visible:

1. `筛选`
2. `视图`
3. `更多`

The separate settings button is removed from the persistent header and its
content moves into `更多`. The separate `结果视图` control is folded into
`视图`. The project menu and project-scoped actions also live under `更多`.

The menu destinations preserve their existing functions:

- `筛选` owns filter and sort configuration;
- `视图` owns grouped/flat search results, contextual descendant aggregation,
  and other context-specific display choices;
- `更多` owns software settings, including thumbnail density, plus project
  access information, permission actions, directory reselection where
  applicable, and closing the project.

The exact grouping must avoid duplicating the same command in two menus.

### Search field

The search field expands to consume available toolbar width. It uses the soft
surface, an eight-to-ten-pixel radius, no strong resting border, and tertiary
placeholder text. Keyboard shortcut feedback remains visible but quiet.

Focused search receives the standard focus ring. Existing text, path, and
content search behavior remains unchanged.

### Filter button state

The filter label is shortened from `筛选与排序` to `筛选`. When filters are
active, a compact indigo count badge appears inside the control. The badge
reports the number of active filters, not sort or scope.

## Search and Filtering

The filter interaction remains an anchored popover. It does not become a
sidebar, full-width inspector, or new page.

The approved popover is approximately 560–590 px wide on a normal desktop
window, constrained to the available viewport. Its information hierarchy is:

1. title and close action;
2. search scope and sort;
3. common filters;
4. collapsed advanced conditions;
5. active-filter summary and clear-all action.

File type and review state are always visible because they are common.
Orientation, pixel dimensions, file size, and modification time move into the
collapsed `高级条件` group. The group remains one action away and retains all
existing filter capability.

Active conditions are repeated as removable compact chips at the bottom of the
popover. The top toolbar shows only the count.

Search result grouping and flat mode belong to `视图`, not the filter popover.
The filter popover must not cover the full workspace at ordinary window sizes.

## Single-image Preview

Single-image preview remains a modal, application-covering view. It does not
become an embedded content pane.

The preview uses:

- a white 52 px toolbar;
- a light neutral stage based on `#F0F1EF`;
- the filename and image metadata at the leading edge;
- display controls grouped in the center;
- rotation and `完成` grouped at the trailing edge;
- a bottom-centered floating navigation control containing previous, count,
  and next.

Fit, 100%, zoom out, zoom percentage, zoom in, rotate, navigation, keyboard
shortcuts, pan, image budgets, repair behavior, and closing semantics remain
unchanged.

The full-width navigation footer is removed visually. Navigation remains in
the same logical order and uses the same accessible names.

## Multi-image Comparison

Comparison continues to replace the file grid inside the content workspace.
It does not become an overlay and is not merged with single-image preview.

The comparison toolbar uses the same white chrome and neutral stage as the
single-image preview:

- image count and current focus appear at the leading edge;
- fit, 100%, zoom, and rotation controls form the central transform group;
- synchronized/independent mode and `完成` appear at the trailing edge.

Comparison panes:

- use a 10–12 px gutter;
- use one-pixel neutral borders and a light canvas;
- show one thin indigo border and soft ring only on the active pane;
- retain filename and remove action in the pane header;
- retain review and favorite controls in the pane footer;
- do not use heavy card shadows.

The existing intelligent two-to-twenty-image layout, virtualization,
synchronized and independent transforms, active-pane selection, original-image
loading, marker editing, pane removal, read-only behavior, and escape behavior
remain unchanged.

## Dialogs

Existing modal focus management and keyboard behavior remain the contract.
Dialogs receive a consistent visual shell:

- white surface;
- 14 px radius;
- one neutral border;
- one restrained modal shadow;
- 18–20 px internal padding;
- direct action-oriented title;
- explanatory copy limited to the information needed for the decision;
- actions aligned to the trailing edge.

Destructive color appears only on the final destructive action. The entire
dialog must not become red. Cancel remains neutral and appears before the
primary or destructive action in logical keyboard order.

Long-form dialogs such as destination selection and batch rename may be wider
and scroll internally, but use the same title, field, error, separator, and
action treatment.

## Empty, Error, and Read-only States

### Empty states

Empty states do not use large illustrations. They contain:

- a short title;
- one concise explanation of why content is absent;
- one primary recovery action;
- at most one secondary action when it represents a meaningfully different
  recovery path.

For an empty search result, clearing filters is primary when filters are
active. Expanding scope to the whole project is secondary.

### Local errors

Failures that affect one row, preview pane, or operation remain at the point of
failure. A local error uses the danger-soft surface, danger text, a one-pixel
semantic border, and a local retry or recovery action.

A failed folder row retains its layout height so surrounding content does not
jump. Other rows remain usable. Local failures must not be promoted to a
blocking application dialog.

Global failures that prevent the application from continuing may use a centered
state, but follow the same concise language and recovery structure.

### Read-only mode

The current large read-only banner becomes a 38 px information strip directly
below the workspace toolbar. It uses the warning-soft surface and includes:

- `只读模式`;
- a concise statement that browse, search, and preview remain available while
  modification and marking are disabled;
- `权限设置`;
- `重新选择目录`.

Read-only restrictions, disabled controls, native permission opening, and
directory reselection behavior remain unchanged.

## Interaction and Motion

Motion explains state change and never becomes decoration.

| Interaction | Duration | Treatment |
| --- | ---: | --- |
| Hover and press | 120 ms | Color, border, and surface only |
| Popover and menu | 160 ms | Opacity plus up to 4 px translation |
| Dialog and preview | 200 ms | Opacity, with stable content geometry |
| Shared easing | — | `cubic-bezier(.2, 0, 0, 1)` |

Menus and dialogs do not bounce, spring, or scale. Image grids, proportional
rows, large comparison layouts, scrolling, zooming, and panning do not receive
decorative layout animation.

`prefers-reduced-motion: reduce` removes nonessential transitions. Functional
progress indication may remain, but continuous shimmer is not required.

## Accessibility

- Text contrast targets at least 4.5:1 for ordinary text.
- Non-text control and active boundaries target at least 3:1.
- Every keyboard-interactive element uses a visible two-pixel indigo
  `:focus-visible` ring with separation from the component edge.
- Color is not the only indicator for active panes, selected navigation,
  filters, errors, or review state; borders, labels, counts, or structure also
  communicate state.
- Compact controls retain at least a 32 px effective target.
- Existing roles, accessible names, focus restoration, focus traps, Escape
  handling, keyboard shortcuts, and logical tab order remain intact.
- Platform shortcut labels may differ (`⌘` on macOS and `Ctrl` on Windows), but
  control geometry remains stable.
- Forced-colors and increased-contrast modes must retain visible borders,
  focus, and selected state.

## Implementation Architecture

The redesign is primarily a CSS and presentation-structure change.

### Token layer

Global Viewer tokens will live in the root application stylesheet or a
dedicated token stylesheet imported before component styles. Existing
preview-only variables will either be replaced by global semantic variables or
mapped to them temporarily during migration.

Component styles consume semantic roles rather than copying color literals.
For example, folder selection, filter selection, preview focus, and keyboard
focus all consume the accent tokens, while danger and warning remain separate.

### Component boundaries

Existing React state and controller boundaries remain. Markup may change where
required to support the approved alignment and grouping:

- `App` aligns project identity with the sidebar and moves persistent settings
  into `更多`;
- `SearchToolbar` exposes `筛选`, `视图`, and `更多` groupings without changing
  search state;
- `FolderOverview` removes the repeated heading band and routes descendant
  aggregation through `视图`;
- preview and comparison components regroup existing controls;
- `ModalSheet`, feedback states, and `ReadOnlyBanner` consume shared visual
  primitives.

No backend commands, persisted project state, filesystem permissions, image
requests, or controller transitions change as a result of visual regrouping.

### Migration order

Implementation should proceed in dependency order:

1. introduce tokens, typography, focus, motion, and primitive control rules;
2. align the application shell and simplify the top toolbar;
3. restyle sidebar, folder bands, content surfaces, and search;
4. restyle preview and comparison;
5. restyle dialogs and feedback states;
6. remove superseded literals and conflicting dark-scheme rules;
7. perform complete visual and interaction regression verification.

Each step should keep the application runnable and should not temporarily
delete a command before its new menu destination works.

## Testing and Visual Verification

### Contract and component tests

Tests should verify:

- approved global token values and semantic mappings;
- persistent toolbar labels and absence of the removed standalone controls;
- active filter count and retained filter behavior;
- contextual availability of descendant aggregation;
- removal of the repeated folder-overview heading band;
- keyboard focus selectors and reduced-motion handling;
- preview and comparison control functionality after regrouping;
- dialog focus trapping, Escape, initial focus, and focus restoration;
- local error, empty state, and read-only content and actions;
- no interaction regression in existing component tests.

### Visual verification matrix

Manual verification must cover at least:

- macOS at the minimum supported window width and a large desktop width;
- compact, standard, and large thumbnail density;
- expanded, resized, and collapsed sidebar;
- project root, category overview, content folder, descendant aggregate, and
  search results;
- filter popover with zero, one, and many active filters;
- grouped and flat results;
- single-image preview at fit, 100%, zoomed, rotated, loading, and error states;
- two-, three-, four-, and large multi-image comparison;
- rename, settings, destination, batch rename, destructive confirmation, and
  close-operation dialogs;
- empty project, empty folder, empty search, row error, global error, and
  read-only mode;
- keyboard-only navigation;
- reduced motion;
- future Windows verification using the same internal tokens when that build
  begins.

Screenshots must be compared at the same viewport and state. Visual review
checks alignment, clipped content, inconsistent padding, unexpected shadow,
wrong border radius, unreadable contrast, and platform-specific font geometry.

## Acceptance Criteria

- Viewer presents one coherent light visual system across every approved
  workspace, preview, comparison, dialog, and feedback state.
- The header is column-aligned: project identity matches the sidebar and
  search/actions match the content workspace.
- Search is followed only by `筛选`, `视图`, and `更多`.
- Settings, result layout, descendant aggregation, and project actions remain
  available in their approved menu destinations.
- The repeated folder-overview heading and blank band are gone.
- Folder overview rows read as continuous quiet bands rather than floating
  cards.
- The filter popover retains all existing capabilities without dominating the
  workspace.
- Single preview and multi-image comparison share the approved white toolbar,
  light stage, grouped controls, and restrained active state.
- Dialogs, empty states, local errors, and read-only feedback follow the
  approved hierarchy and recovery rules.
- Approved color, type, spacing, radius, border, focus, and motion tokens are
  used consistently.
- macOS-owned appearance remains native while Viewer-owned appearance is ready
  to be shared with Windows.
- Existing workflows, keyboard behavior, data flow, file safety, and project
  behavior remain unchanged.
- Static checks, component tests, production build, accessibility checks, and
  the complete visual verification matrix pass before the redesign is declared
  complete.
