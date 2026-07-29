# Viewer Other Files And Split Text Preview Design

Date: 2026-07-29
Status: Approved

## Problem

Viewer currently treats only Markdown and TXT files as non-image content. The
scanner recognizes `.jpg`, `.jpeg`, `.png`, `.md`, `.markdown`, and `.txt`;
every other regular file is silently omitted from the project.

This causes three product problems:

- a folder can contain files that Viewer never reveals to the user;
- the label `文本文件` no longer describes the desired scope of the secondary
  file area;
- two related text files cannot be inspected side by side even though
  comparison is a core Viewer workflow.

The desired behavior is to classify every visible regular file without reading
its contents. Known images remain in the image area, known videos remain
ignored for this iteration, and everything else appears in an `其它文件` area.
Existing text preview remains available within that broader area, including a
new two-file split preview.

## Goals

- Rename the user-facing `文本文件` area to `其它文件`.
- Index every visible, non-video regular file.
- Keep all recognized image formats in the image area, including formats that
  Viewer cannot currently decode.
- Show a clear `暂不支持预览` state for recognized but unsupported images.
- Keep known videos out of the workspace until video support is designed.
- Place extensionless and unknown-format files in `其它文件`.
- Preserve Markdown and plain-text preview inside the broader other-file
  collection.
- Let a user select one or two previewable text files and open them from the
  radial menu.
- Render two selected text files as an equal-width left/right split preview.
- Preserve the adaptive secondary shelf, scoped select-all behavior, file
  management actions, and the 20-image comparison limit.
- Avoid file-content reads during classification and keep large folders
  responsive.

## Non-goals

- Adding video browsing, thumbnails, playback, selection, or preview.
- Adding image decoders beyond the currently supported JPEG and PNG pipeline.
- Previewing PDF, Office, archive, design-project, executable, or arbitrary
  binary formats.
- Opening unsupported files with macOS Quick Look or their default
  applications.
- Detecting formats from file signatures, MIME inspection, or file contents.
- Allowing more than two simultaneous text previews.
- Adding a resizable text-preview divider.
- Adding a user-editable format registry.
- Indexing arbitrary other-file contents for full-text search.
- Redesigning the radial menu or the existing image comparison experience.

## Current Preview Support

This change broadens classification, not decoding. After the change, the
formats whose contents Viewer can actually preview remain:

| Content | Extensions | Behavior |
| --- | --- | --- |
| JPEG image | `.jpg`, `.jpeg` | Existing image thumbnail and preview pipeline |
| PNG image | `.png` | Existing image thumbnail and preview pipeline |
| Plain text | `.txt` | Existing text preview |
| Markdown | `.md`, `.markdown` | Existing sanitized Markdown preview |

Text preview continues to support UTF-8, UTF-16 LE, UTF-16 BE, and GB18030.
Each preview reads at most the first 10 MiB and reports when the content is
truncated.

## Chosen Direction

Classification uses centralized, case-insensitive extension registries in the
backend.

The rejected alternatives are:

1. **System MIME classification.** It can recognize more formats dynamically,
   but results vary by platform and the extra system work makes large scans
   less predictable.
2. **Extension plus content sniffing.** It improves accuracy, but adds file I/O,
   latency, error paths, and potentially expensive reads during a directory
   scan.
3. **Extension registries.** It reads only directory metadata and filenames,
   has deterministic behavior, and can be extended in one place as Viewer adds
   support. This is the approved direction.

An unknown extension intentionally falls back to `其它文件`. The registry
therefore favors complete visibility and predictable performance over claiming
perfect format detection.

## Classification Policy

### Visibility exclusions

The scanner skips:

- files and directories whose names begin with `.`;
- Viewer-owned hidden data such as `.viewer`;
- common system noise including `.DS_Store`, `Thumbs.db`, and `desktop.ini`;
- symbolic links, preserving the existing no-follow policy and preventing
  directory cycles.

Name checks are case-insensitive where the operating-system filename permits
it. Excluded directories are not traversed.

### Previewable images

`.jpg`, `.jpeg`, and `.png` produce the existing previewable image kinds.

### Recognized but unsupported images

The initial unsupported-image registry covers common raster, vector, editing,
and camera RAW extensions:

- general image formats: `.gif`, `.webp`, `.avif`, `.heic`, `.heif`, `.bmp`,
  `.tif`, `.tiff`, `.svg`, `.svgz`, `.ico`, `.jxl`, `.jfif`, and `.apng`;
- editing formats: `.psd`;
- camera RAW formats: `.dng`, `.cr2`, `.cr3`, `.nef`, `.nrw`, `.arw`, `.srf`,
  `.sr2`, `.raf`, `.orf`, `.rw2`, `.pef`, `.srw`, and `.x3f`.

These files enter the image collection with an
`unsupported_image` classification. Viewer does not request thumbnails or
decoded representations for them.

The registry is deliberately centralized so an implementation plan can add
missing common extensions without changing UI logic. Recognition does not
claim that Viewer can decode the file.

### Ignored videos

The initial video registry covers `.mp4`, `.mov`, `.m4v`, `.avi`, `.mkv`,
`.webm`, `.wmv`, `.flv`, `.mpeg`, and `.mpg`.

These files are recognized only so they are not incorrectly placed in
`其它文件`. They do not produce indexed file nodes, appear in scan totals, or
enter any UI collection in this iteration.

### Other files

Every remaining visible regular file produces an indexed other-file node.
This includes:

- `.txt`, `.md`, and `.markdown`, which retain their previewable subtypes;
- unsupported documents, archives, audio, executables, and project files;
- files with unknown extensions;
- files with no extension.

The word `其它` describes placement, not preview capability. A file may be an
other file and still support text preview.

## Domain And API Model

The domain keeps distinct kinds for behavior rather than creating one enum
variant per extension:

- `directory`;
- `jpeg`;
- `png`;
- `unsupported_image`;
- `markdown`;
- `text`;
- `other`.

Shared predicates express policy at call sites:

- image: `jpeg`, `png`, or `unsupported_image`;
- previewable image: `jpeg` or `png`;
- other-file collection: `markdown`, `text`, or `other`;
- previewable text: `markdown` or `text`.

The scanner remains the canonical owner of extension classification. The UI
receives the domain kind through the existing serialized file contract and
does not maintain a competing extension registry.

The content workspace changes from:

```text
images + textFiles
```

to:

```text
images + otherFiles
```

`otherFiles` contains Markdown, TXT, and generic other files. The internal
desktop application does not need a compatibility alias because all domain,
application, IPC, and UI consumers are updated atomically in the same
development version.

Image metadata and text-index status remain meaningful only for their
respective supported kinds. Generic other files do not enter the text index.

## Scanner Data Flow

1. `WalkDir` applies the visibility and no-symbolic-link filters.
2. The walker normalizes the filename extension to lowercase without opening
   the file.
3. A previewable image extension creates its existing image kind.
4. A recognized unsupported image extension creates an
   `unsupported_image` node.
5. A recognized video extension returns no node.
6. Markdown and TXT retain their existing kinds.
7. Every remaining regular file creates an `other` node.
8. Existing batches publish the resulting nodes and folders.
9. Folder queries partition file nodes into `images` and `otherFiles` by the
   shared kind predicates.

Unsupported images and generic other files can be found by filename and path.
Only Markdown and TXT continue to participate in text-content indexing and
content search.

## Other-File Shelf

The approved adaptive text shelf becomes an adaptive other-file shelf.
Its existing layout policy remains intact:

| Images | Other files | Mode | Result |
| --- | --- | --- | --- |
| yes | no | `image_only` | No other-file shelf; image grid fills the body |
| yes | yes, collapsed | `mixed_collapsed` | Compact `其它文件 · N` disclosure |
| yes | yes, expanded | `mixed_expanded` | Adaptive shelf below the image grid |
| no | yes | `other_only` | Other-file shelf fills the content body |

In mixed expanded mode, the complete shelf height is the smaller of its
intrinsic content height and 20% of the available content-body height. Overflow
scrolls inside the list. In other-only mode, the list uses the complete
available body.

The component and public prop names change from text-specific names to
other-file names. Previewable text rows retain their current icons and
interactions. Generic other rows use a lightweight file icon plus their
filename and extension; extensionless files use a generic file label.

Because the collection can now be much larger than a text-only list, row
rendering uses the existing virtual-list primitive. Expanding or collapsing
the shelf does not discard selection.

All other files remain eligible for the existing selection, marker, rename,
copy, move, trash, Finder drag, organization drag, and information actions.
Preview capability is evaluated separately from management capability.

## Select-All Behavior

The existing type-aware select-all behavior is renamed and preserved.

### Single-type folders

- Images only: directly select all images without prompting.
- Other files only: directly select all other files without prompting.
- Empty: keep the action disabled.

### Mixed folders

The choice panel offers:

1. `全选图片`
2. `全选其它文件`
3. `全部都选`

The panel keeps its existing focus, Escape, outside-click, navigation, and
stale-data protections. Choosing hidden rows does not force the shelf open;
the collapsed disclosure reports the number of selected other files.

Recognized unsupported images count as images for every select-all rule.
Videos are absent and therefore never included.

## Unsupported Preview Experience

### Unsupported images

An `unsupported_image` card:

- appears in normal image ordering and selection;
- displays a neutral file placeholder, the filename, the file extension, and
  `暂不支持预览`;
- never invokes the image thumbnail command.

Double-clicking the card opens the light preview surface with the same filename
and unsupported message. In a multi-image comparison, the file receives a
normal comparison panel whose stage contains the placeholder rather than an
image. It remains part of the comparison count and selection order.

The existing maximum of 20 selected images applies to both supported and
unsupported image kinds. More than 20 selected images keeps the multi-image
preview action disabled.

### Unsupported other files

Generic other files remain selectable and manageable. Double-clicking one
opens a lightweight, light-themed preview surface with its filename, extension,
and `暂不支持预览`. The surface does not invoke a backend content-read command.

Viewer does not offer Quick Look or default-application opening from this
state.

## Text Preview Entry Rules

Text preview capability is restricted to `text` and `markdown` kinds.

### Double-click

Double-clicking a previewable text row always opens that one file, regardless
of the wider selection. This avoids unexpectedly opening a second selected
file.

### Radial-menu preview

The existing single-file preview rule remains available for every indexed
non-directory file:

- one supported image opens image preview;
- one unsupported image opens its image placeholder preview;
- one previewable text file opens text preview;
- one generic other file opens its unsupported placeholder preview.

Exactly two selected files can use the same radial preview action only when
both are previewable text files. That case opens split preview.

The action is disabled when:

- more than two files are selected;
- exactly two selected files include an image or generic other file;
- exactly two selected files mix previewable text with an unsupported file.

The disabled state explains that preview supports either one file or two
previewable text files. A selection containing too many text files also
exposes the approved limit `文本预览最多支持 2 个可预览文件`.

This restriction affects preview only. Users may still select any number of
other files for select-all and management actions.

## Split Text Preview

The split view is one light-themed modal preview surface with two equal-width
panes separated by a visible divider.

Each pane independently owns:

- the filename;
- its encoding selector;
- loading state;
- decoded or rendered content;
- unsupported-encoding guidance;
- error state;
- truncation notice;
- content scrolling;
- Markdown link handling.

Both preview requests start concurrently. A slow or failed file does not block
the other pane. Each request independently applies the 10 MiB limit and
encoding override.

The dialog owns focus trapping, Escape handling, and one close command. Closing
exits the complete split preview; individual panes are not removed. Focus
returns to the radial-menu source or prior connected element using the existing
preview restoration behavior.

The task bar exposes one aggregate text-preview task:

- `requested` equals the number of preview panes;
- `completed` and `failed` reflect the independent request results;
- the task completes when both requests settle.

The component boundary separates the reusable per-file preview pane from the
one- or two-pane dialog shell. This prevents duplicating encoding, Markdown,
loading, and error logic.

## State And Routing

Application preview state distinguishes:

- one supported image preview session;
- one image-comparison session;
- one unsupported-file placeholder preview;
- one text-preview session containing one or two previewable text files.

Entering any preview closes an incompatible preview or comparison session using
the existing single-overlay rule. Folder or project identity changes close the
active preview and discard stale requests.

Text preview requests remain keyed by entity ID and modified timestamp. Pane
state resets when its file identity changes. Late responses from a closed or
replaced session are ignored.

## Performance

- Classification performs no file-content reads.
- Video and image recognition is a lowercase extension lookup.
- Unsupported images never request thumbnails or image decoding.
- Generic other files never enter the text indexing pipeline.
- Existing scan batching remains unchanged.
- The image area retains its aspect-aware virtual grid.
- The expanded other-file list uses bounded, virtualized row rendering.
- Single and split text preview read content only on explicit user action.
- Split preview starts at most two bounded reads.

Indexing previously ignored files increases the number of stored file nodes by
design. The work per additional file remains directory metadata, a lowercase
extension lookup, and normal node publication.

## Dynamic And Error States

- If a file disappears after scanning, only its preview pane or placeholder
  reports that it is unavailable.
- A read or encoding failure in one text pane does not replace content in the
  other pane.
- Closing or replacing a split preview invalidates both pending UI updates.
- A watcher refresh reclassifies renamed files using the same canonical
  backend registry.
- Changing an extension can move a file between image, ignored-video, and
  other-file collections on the next reconciliation.
- If the last other file disappears, the shelf unmounts and the image grid
  reclaims its space.
- If the last image disappears, the other-file shelf becomes the full-body
  `other_only` view.
- Unsupported files never generate misleading decode or text-read failures;
  they use the explicit unsupported state.

## Accessibility

- Unsupported placeholders expose the filename, format, and unsupported status
  through one accessible label.
- The other-file disclosure retains `aria-expanded` and `aria-controls`.
- The split preview is one dialog with an accessible name describing both
  files.
- Each pane has a unique heading and uniquely associated encoding control.
- Keyboard focus enters the dialog, remains within it, and returns to the
  previous connected target after close.
- Each pane scrolls independently with keyboard and assistive-technology
  semantics.
- Disabled preview actions expose the reason through the existing radial-menu
  description/status mechanism.

## Verification

### Domain and infrastructure

- Test the new kind predicates and serialized values.
- Test case-insensitive classification of every registry family.
- Test previewable images, unsupported images, videos, previewable text,
  unknown extensions, and extensionless files.
- Test hidden files, hidden directories, Viewer data, system-noise files, and
  symbolic links.
- Test full scans and subtree snapshots with the same classification policy.
- Verify ignored videos do not enter file totals or published batches.

### Application and API

- Verify folder queries partition nodes into `images` and `otherFiles`.
- Verify only Markdown and TXT can call text preview.
- Verify unsupported images cannot call the image representation pipeline.
- Verify search indexes names and paths for all visible nodes but text content
  only for supported text kinds.
- Update serialized-contract and architecture-baseline tests for the new kinds
  and workspace field.

### UI

- Verify the `其它文件` labels in collapsed, expanded, and other-only modes.
- Verify the shelf is absent when empty, capped at 20% in mixed mode, and
  internally scrollable and virtualized when large.
- Verify mixed and single-type select-all behavior with unsupported images and
  generic other files.
- Verify unsupported image cards make no thumbnail requests.
- Verify unsupported placeholders in the image grid, single preview, and
  multi-image comparison.
- Verify unsupported other-file double-click behavior.
- Verify one selected text file opens single preview from the radial menu.
- Verify two selected text files open equal-width split preview.
- Verify double-click always opens only its target text file.
- Verify more than two files and mixed unsupported selections disable preview
  with the approved reason.
- Verify pane-local loading, encoding, scrolling, Markdown links, errors, and
  truncation.
- Verify concurrent requests, aggregate task progress, Escape, focus return,
  and stale-response protection.

### Regression gates

- Preserve JPEG, PNG, Markdown, and TXT behavior.
- Preserve the 20-image comparison limit and aspect-aware image layout.
- Preserve marker and file-management actions for all indexed files.
- Run the existing UI unit suite, Rust workspace suite, policy checks, lint,
  formatting, type checking, and production build.

## Acceptance Criteria

The change is complete when:

1. A visible regular file is either an indexed image, an ignored known video,
   or an indexed other file.
2. No empty `其它文件` area reserves workspace height.
3. Unsupported image formats appear in the image area without decode attempts
   and clearly report `暂不支持预览`.
4. Unknown and extensionless files appear in `其它文件`.
5. Markdown and TXT retain their current single-file preview.
6. Every indexed non-directory file retains its single-file radial preview
   entry, using a content preview or unsupported placeholder as appropriate.
7. Exactly two selected previewable text files can be opened from the radial
   menu in an independent left/right split.
8. More than two selected files or a mixed two-file selection cannot enter
   split preview and explains why.
9. Select-all uses `图片`, `其它文件`, and `全部都选` scopes correctly.
10. Large collections use bounded rendering and classification does not read
   file contents.
11. All project quality gates pass without modifying installed or packaged
    Viewer applications.
