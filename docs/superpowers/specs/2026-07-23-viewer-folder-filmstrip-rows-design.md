# Viewer Folder Filmstrip Rows Design

Date: 2026-07-23
Status: Approved

## Problem

The category overview currently renders each numbered folder as a large card in
an auto-filling grid. The repeated card chrome and multiline statistics consume
most of the workspace, while only four representative images are visible for
each folder. Comparing B01, B02, and later folders requires scanning across and
down an irregular grid and entering each folder to see the rest of its images.

## Desired Experience

- Each numbered folder occupies one full-width horizontal row.
- The row has its own horizontal image scroller and shows every direct image in
  that folder.
- Clicking a thumbnail previews that image without leaving the category
  overview.
- Clicking the folder number or name enters that folder.
- Preview navigation stays within the images from the row that opened it.
- Rows remain fast when a category contains many folders and images.

## Layout

`FolderOverview` keeps the current category heading and “显示全部后代文件”
action. The card grid becomes a vertical list of filmstrip rows.

Each row uses two columns:

1. A fixed-width identity panel containing the folder name, relative path,
   image/text counts, folder marker, and compact reviewed/total progress.
2. A flexible horizontal filmstrip containing square or portrait-preserving
   thumbnails at a consistent height.

The identity panel is a button labelled `打开 <folder name>`. Rows use a subtle
bottom divider instead of a rounded card border. The filmstrip reserves a
stable scrollbar gutter and follows the system's native scrollbar visibility
preference. Empty image folders display `无图片`.

Long review-state breakdown text is removed from the default row surface. The
essential reviewed/total progress and marker remain visible; complete review
breakdown data remains available after entering the folder.

## Data Loading

The existing category projection remains the source of folder identity,
counts, markers, review progress, and representative images. It does not expand
to include every image.

`FolderOverview` receives a request callback that loads one child folder's
normal, non-aggregate workspace by entity id. A row starts this request only
when it approaches the viewport through `IntersectionObserver`. The returned
content workspace supplies the complete ordered image list for that row.

Row results are cached by folder entity id for the lifetime of the current
overview. Concurrent requests for the same folder share one promise. `App`
provides an overview identity derived from project session, generation, and
selected category id; changing that identity remounts `FolderOverview` and
discards the row cache. This prevents files from a previous category or project
session from leaking into the next view.

Thumbnail bytes continue through the existing thumbnail request and cache path.
Only visible thumbnail elements request image data.

## Preview Context

The application preview state gains an explicit ordered file context instead of
assuming the active content workspace is always the source. Opening a filmstrip
thumbnail stores both the chosen image and the row's image list. `ImagePreview`
uses that list for previous/next navigation while the category overview remains
behind the modal.

Existing previews opened from a normal folder continue to use the current
content workspace as their context. Closing a preview restores focus to the
thumbnail button that opened it.

## Interaction and Accessibility

- Folder identity is a keyboard-operable button.
- Every thumbnail is a button with the accessible name `预览 <file name>`.
- Enter or Space on a thumbnail opens preview.
- Arrow-key behavior inside the preview is unchanged and stays within the row.
- Each filmstrip is labelled `<folder name> 图片`.
- Vertical page scrolling and each row's horizontal scrolling are independent.
- Right-click file actions are not added to the overview in this change.

## Loading and Failure States

- Before a row enters the loading margin, it shows a lightweight placeholder.
- While loading, it shows thumbnail skeleton cells without blocking other rows.
- A failed row displays `无法加载图片` and a `重试` button.
- Retrying replaces only that row's failed cache entry.
- If the folder changes externally, the normal project-change projection
  refresh recreates the overview and reloads affected rows.

## Component Boundaries

- `FolderOverview` owns the vertical row list and per-overview request cache.
- `FolderFilmstripRow` owns visibility detection, row loading state, horizontal
  scrolling, folder navigation, and thumbnail preview buttons.
- The existing `FolderThumbnail` continues to own thumbnail visibility and
  image request state.
- `App` provides the child-folder content request and preview-session callbacks.
- Backend contracts and dependencies remain unchanged.

## Testing

Component tests cover:

- one full-width row per folder in source order;
- independent labelled horizontal filmstrips;
- lazy row requests and same-folder request deduplication;
- all returned images rendered in order;
- folder-button navigation;
- thumbnail preview with the complete row context;
- empty, loading, failed, and retry states;
- metadata remaining visible before images resolve;
- keyboard-accessible folder and image actions.

Application tests cover:

- filmstrip preview opening while category overview remains selected;
- previous/next preview navigation staying within the originating row;
- closing preview restoring focus;
- existing content-folder previews remaining unchanged.

The complete repository verification command must pass after implementation.
