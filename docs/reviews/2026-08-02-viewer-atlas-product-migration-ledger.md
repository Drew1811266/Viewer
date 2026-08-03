# Viewer Atlas-to-Product Migration Ledger

> Branch/commit, process PID, macOS version, display scale and final evidence paths are recorded during Task 15. A row remains `pending` until its automated evidence and required native comparisons are recorded.

## Acceptance fixture convention

Native recipes use one disposable real project outside the source tree at `target/atlas-product-migration-fixture/ViewerAcceptance`:

- `角色/B01` and `衣服/A01` contain copied JPG/PNG fixtures, including at least 30 images for density, paging, selection and 20-image comparison.
- `文档` contains `sample.md`, `plain.txt`, a valid GB18030 text file and a text file larger than 10 MiB.
- `其它` contains one arbitrary unsupported document and one unsupported image.
- `空目录/Empty` contains no files; `目标/Source` and `目标/Destination` contain same-named files for real copy/move conflicts.
- `corrupt.jpg` is copied from `tests/fixtures/images/corrupt.jpg`; an indexed file can be renamed in Finder after scanning to create a real unavailable state.
- A read-only copy of the project and an unreadable disposable file are used for permission, failure and recovery states. No source fixture in `tests/fixtures` is mutated.

`forced-colors` has no native macOS system mode. Its current-platform visual evidence uses macOS Increase Contrast and Reduce Transparency at both viewports, while the actual `forced-colors` behavior is an automated CSS contract; Windows-native forced-colors evidence is deferred to the future Windows phase and does not authorize different product styling.

| ID | Wave | Reference state | Product owner | Native entry recipe | Automated evidence | Native 1024 | Native 1440 | Result |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| LAU-01 | Wave 1 | `launch-no-project` | `EmptyProject` | Launch Viewer with no session, or choose `关闭项目` and wait for the empty entry screen. | `EmptyProject` (`EmptyProject.test.tsx`) | not-recorded | not-recorded | pending |
| LAU-02 | Wave 1 | `launch-drag` | `EmptyProject` drop layer | From Finder drag the `ViewerAcceptance` directory over the empty Viewer window without releasing. | `EmptyProject` (`EmptyProject.test.tsx`) | not-recorded | not-recorded | pending |
| LAU-03 | Wave 1 | `launch-invalid` | `EmptyProject` local feedback | From Finder drop `tests/fixtures/images/srgb.jpg` onto the empty Viewer window. | `EmptyProject` (`EmptyProject.test.tsx`) | not-recorded | not-recorded | pending |
| LAU-04 | Wave 1 | `launch-opening` | `EmptyProject` opening state | Click `选择项目文件夹`, choose `ViewerAcceptance`, and capture while project validation is active. | `EmptyProject` (`EmptyProject.test.tsx`) | not-recorded | not-recorded | pending |
| LAU-05 | Wave 1 | `launch-scanning` | `WorkspaceLoadingState` and `TaskBar` | Remove only the disposable fixture's `.viewer` data, open `ViewerAcceptance`, and capture during its first real scan. | `WorkspaceLoadingState` (`WorkspaceLoadingState.test.tsx`) | not-recorded | not-recorded | pending |
| LAU-06 | Wave 1 | `launch-thumbnails` | `AspectThumbnail` and `TaskBar` | During the fresh scan select `衣服/A01` before thumbnail generation has completed. | `AspectThumbnail` (`AspectThumbnail.test.tsx`) | not-recorded | not-recorded | pending |
| LAU-07 | Wave 1 | `launch-empty` | `ViewerEmptyState` in `App` | Open `ViewerAcceptance`, select `空目录/Empty`, and keep search closed. | `Viewer empty state` (`App.test.tsx`) | not-recorded | not-recorded | pending |
| LAU-08 | Wave 1 | `launch-error` | `ViewerLocalFeedback` in `EmptyProject` | Close the project and choose the disposable unreadable project copy so real open validation fails. | `EmptyProject` (`EmptyProject.test.tsx`) | not-recorded | not-recorded | pending |
| LAU-09 | Wave 1 | `launch-recovery` | `GlobalNoticeStack` | Start a real multi-file move in the disposable project, terminate Viewer during the operation, relaunch and reopen the same fixture. | `GlobalNoticeStack` (`GlobalNoticeStack.test.tsx`) | not-recorded | not-recorded | pending |
| SID-01 | Wave 1 | `sidebar-expanded` | `App` and `FolderTree` | Open `ViewerAcceptance` with the folder sidebar expanded and select the project root. | `App.test.tsx`; `FolderTree.test.tsx` | not-recorded | not-recorded | pending |
| SID-02 | Wave 1 | `sidebar-resized` | `useAppShellState` separator | Drag the sidebar separator to 320 px, then focus it and verify arrow-key resizing. | `App.test.tsx` | not-recorded | not-recorded | pending |
| SID-03 | Wave 1 | `sidebar-collapsed` | collapsed navigation rail | Click the sidebar collapse action after opening `ViewerAcceptance`. | `App.test.tsx` | not-recorded | not-recorded | pending |
| SID-04 | Wave 1 | `sidebar-drop` | `FolderTree` drop target | Select two files in `衣服/A01`, start organization drag, hover `目标/Destination`, then hover their current folder for the invalid variant. | `FolderTree.test.tsx`; `useOrganizationPointerDrag.test.tsx` | not-recorded | not-recorded | pending |
| STR-01 | Wave 1 | `structure-root` | `FolderOverview` | Select the project root and verify the root overview has no repeated structure title. | `FolderOverview.test.tsx` | not-recorded | not-recorded | pending |
| STR-02 | Wave 1 | `structure-category` | `FolderOverview` | Select the `衣服` category so its child folder bands are visible. | `FolderOverview.test.tsx`; `FolderFilmstripRow.test.tsx` | not-recorded | not-recorded | pending |
| STR-03 | Wave 1 | `structure-content` | `ContentBrowser` | Select `衣服/A01` and verify content begins directly with the image grid. | `ContentBrowser.test.tsx`; `App.test.tsx` | not-recorded | not-recorded | pending |
| STR-04 | Wave 1 | `structure-aggregate` | `ContentBrowser` aggregate tag | In `衣服/A01`, open `视图` and choose `显示全部后代文件`. | `App.test.tsx`; `WorkspaceViewMenu.test.tsx` | not-recorded | not-recorded | pending |
| STR-05 | Wave 1 | `structure-bands` | `FolderFilmstripRow` | Select `衣服` and scroll through at least three continuous child folder bands. | `FolderOverview.test.tsx`; `FolderFilmstripRow.test.tsx` | not-recorded | not-recorded | pending |
| THU-01 | Wave 1 | `density-compact` | `AspectVirtualGrid` | In Settings choose compact density, close Settings and open `衣服/A01`. | `ContentBrowser.test.tsx`; `aspectLayout.test.ts` | not-recorded | not-recorded | pending |
| THU-02 | Wave 1 | `density-standard` | `AspectVirtualGrid` | In Settings choose standard density, close Settings and open `衣服/A01`. | `ContentBrowser.test.tsx`; `aspectLayout.test.ts` | not-recorded | not-recorded | pending |
| THU-03 | Wave 1 | `density-large` | `AspectVirtualGrid` | In Settings choose large density, close Settings and open `衣服/A01`. | `ContentBrowser.test.tsx`; `aspectLayout.test.ts` | not-recorded | not-recorded | pending |
| THU-04 | Wave 1 | `selection-none` | `ImageCell` | Open `衣服/A01`, click the content background and leave every thumbnail unselected. | `ContentBrowser.test.tsx`; `contentSelection.test.ts` | not-recorded | not-recorded | pending |
| THU-05 | Wave 1 | `selection-single` | `ImageCell` | Click one image in `衣服/A01`. | `ContentBrowser.test.tsx`; `styles/app.test.ts` | not-recorded | not-recorded | pending |
| THU-06 | Wave 1 | `selection-multiple` | `ContentBrowser` summary | Click one image, then Command-click three additional images in `衣服/A01`. | `ContentBrowser.test.tsx`; `contentSelection.test.ts` | not-recorded | not-recorded | pending |
| THU-07 | Wave 1 | `selection-focus` | virtual grid active item | Select one image, focus the grid and use an arrow key so selection and keyboard focus appear together. | `AspectVirtualGrid.test.tsx`; `styles/app.test.ts` | not-recorded | not-recorded | pending |
| OTH-01 | Wave 1 | `other-collapsed` | `OtherFilePanel` | Open the `其它` folder with its other-file panel collapsed. | `OtherFilePanel.test.tsx`; `ContentBrowser.test.tsx` | not-recorded | not-recorded | pending |
| OTH-02 | Wave 1 | `other-expanded` | `OtherFilePanel` | Open the `其它` folder and expand its other-file panel. | `OtherFilePanel.test.tsx`; `ContentBrowser.test.tsx` | not-recorded | not-recorded | pending |
| OTH-03 | Wave 1 | `organization-drag` | `OrganizationDragHandle` and sidebar target | Select two items, drag from the organization handle and hold over `目标/Destination` while pressing and releasing Option to show copy/move. | `ContentBrowser.test.tsx`; `useOrganizationPointerDrag.test.tsx`; `FolderTree.test.tsx` | not-recorded | not-recorded | pending |
| SEA-01 | Wave 1 | `search-grouped` | `SearchResults` | Search `jpg` across `ViewerAcceptance`, then in `视图` choose grouped results. | not-recorded | not-recorded | not-recorded | pending |
| SEA-02 | Wave 1 | `search-flat` | `SearchResults` | Search `jpg`, then in `视图` choose flat results. | not-recorded | not-recorded | not-recorded | pending |
| SEA-03 | Wave 1 | `search-indexing` | search indexing feedback | Open a freshly scanned fixture and enter a text search before indexing completes. | not-recorded | not-recorded | not-recorded | pending |
| SEA-04 | Wave 1 | `search-paging` | `SearchResults` pagination | Search a term matching more than one page of the 30-image fixture, then navigate to page two. | not-recorded | not-recorded | not-recorded | pending |
| SEA-05 | Wave 1 | `search-empty` | `SearchResults` empty state | Search the unique string `viewer-no-match-20260802`. | not-recorded | not-recorded | not-recorded | pending |
| FIL-01 | Wave 1 | `filters-zero` | `SearchToolbar` | Open `筛选` with all filter conditions cleared. | not-recorded | not-recorded | not-recorded | pending |
| FIL-02 | Wave 1 | `filters-one` | `SearchToolbar` | Open `筛选` and enable only `JPEG`. | not-recorded | not-recorded | not-recorded | pending |
| FIL-03 | Wave 1 | `filters-multiple` | `SearchToolbar` | Enable `JPEG`, `PNG`, `保留` and `收藏`, then leave the popover open. | not-recorded | not-recorded | not-recorded | pending |
| FIL-04 | Wave 1 | `filters-advanced` | `SearchToolbar` | Open `高级条件` and enter a real minimum width and modification-time boundary. | not-recorded | not-recorded | not-recorded | pending |
| MEN-01 | Wave 1 | `menu-view` | `WorkspaceViewMenu` | Open `视图` while grouped search results are active. | not-recorded | not-recorded | not-recorded | pending |
| MEN-02 | Wave 1 | `menu-more` | `WorkspaceMoreMenu` | Open `更多` on a writable active project without hovering the close command. | not-recorded | not-recorded | not-recorded | pending |
| MEN-03 | Wave 1 | `menu-readonly` | `ReadOnlyBanner` and `WorkspaceMoreMenu` | Open the read-only fixture copy, then open `更多`. | not-recorded | not-recorded | not-recorded | pending |
| RAD-01 | Wave 2 | `radial-click` | `RadialFileMenu` | Select one image and secondary-click it without moving the pointer. | not-recorded | not-recorded | not-recorded | pending |
| RAD-02 | Wave 2 | `radial-gesture` | radial pointer session | Select one image, hold secondary click, move more than 8 px into a primary sector and keep the button held. | not-recorded | not-recorded | not-recorded | pending |
| RAD-03 | Wave 2 | `radial-mark` | mark secondary ring | Open the radial menu and dwell or navigate to `标记` until the secondary ring opens. | not-recorded | not-recorded | not-recorded | pending |
| RAD-04 | Wave 2 | `radial-organize` | organize secondary ring | Open the radial menu and dwell or navigate to `整理` until the secondary ring opens. | not-recorded | not-recorded | not-recorded | pending |
| RAD-05 | Wave 2 | `radial-disabled` | `RadialMenuModel` | Select one non-image or only one image so compare is disabled, then open the radial menu and focus the disabled command. | not-recorded | not-recorded | not-recorded | pending |
| RAD-06 | Wave 2 | `radial-readonly` | read-only radial model | In the read-only fixture select one image and open the radial menu. | not-recorded | not-recorded | not-recorded | pending |
| RAD-07 | Wave 2 | `radial-keyboard` | radial keyboard model | Focus a selected image, invoke the keyboard context menu, navigate with arrows and open a secondary ring without executing. | not-recorded | not-recorded | not-recorded | pending |
| PRE-01 | Wave 2 | `preview-fit` | `ImagePreview` | Open one image from `衣服/A01` and leave `适应窗口` active. | not-recorded | not-recorded | not-recorded | pending |
| PRE-02 | Wave 2 | `preview-100` | `ImagePreview` | In image preview choose `100%`. | not-recorded | not-recorded | not-recorded | pending |
| PRE-03 | Wave 2 | `preview-zoom` | `ImagePreview` | In image preview press zoom-in twice so the percentage differs from fit and 100%. | not-recorded | not-recorded | not-recorded | pending |
| PRE-04 | Wave 2 | `preview-rotate` | `ImagePreview` | In image preview activate clockwise rotation once. | not-recorded | not-recorded | not-recorded | pending |
| PRE-05 | Wave 2 | `preview-loading` | `ImagePreview` stage feedback | Clear the disposable preview cache and open the largest real image, capturing before decoding finishes. | not-recorded | not-recorded | not-recorded | pending |
| PRE-06 | Wave 2 | `preview-error` | `ImagePreview` local feedback | Index an image, rename it in Finder, then open its stale Viewer row so the real preview request fails. | not-recorded | not-recorded | not-recorded | pending |
| PRE-07 | Wave 2 | `preview-navigation` | image navigation control | Open the second image in a folder with at least three images so both previous and next actions are enabled. | not-recorded | not-recorded | not-recorded | pending |
| COM-01 | Wave 2 | `compare-2` | `CompareWorkspace` | Select exactly two images and choose `并排对比`. | not-recorded | not-recorded | not-recorded | pending |
| COM-02 | Wave 2 | `compare-3` | `CompareWorkspace` smart layout | Select exactly three images and choose `并排对比`. | not-recorded | not-recorded | not-recorded | pending |
| COM-03 | Wave 2 | `compare-4` | `CompareWorkspace` smart layout | Select exactly four images and choose `并排对比`. | not-recorded | not-recorded | not-recorded | pending |
| COM-04 | Wave 2 | `compare-many` | `CompareVirtualViewport` | Select exactly twenty images in `衣服/A01`, start comparison and scroll until virtualization changes the mounted panes. | not-recorded | not-recorded | not-recorded | pending |
| DOC-01 | Wave 2 | `document-markdown` | `TextPreviewPane` | Open `文档/sample.md`. | not-recorded | not-recorded | not-recorded | pending |
| DOC-02 | Wave 2 | `document-plain` | `TextPreviewPane` | Open `文档/plain.txt`. | not-recorded | not-recorded | not-recorded | pending |
| DOC-03 | Wave 2 | `document-encoding` | text encoding field | Open the GB18030 fixture first with the wrong encoding, then leave the encoding warning and selector visible. | not-recorded | not-recorded | not-recorded | pending |
| DOC-04 | Wave 2 | `document-truncated` | text truncation feedback | Open the text fixture larger than 10 MiB. | not-recorded | not-recorded | not-recorded | pending |
| DOC-05 | Wave 2 | `document-dual` | dual `TextPreviewPane` | Select `sample.md` and `plain.txt`, then invoke Preview. | not-recorded | not-recorded | not-recorded | pending |
| DOC-06 | Wave 2 | `document-unsupported` | `UnsupportedFilePreview` | Open the arbitrary unsupported document from `其它`. | not-recorded | not-recorded | not-recorded | pending |
| DOC-07 | Wave 2 | `document-unavailable` | `UnsupportedFileState` | Index an unsupported file, rename it in Finder, then open its stale Viewer row. | not-recorded | not-recorded | not-recorded | pending |
| INF-01 | Wave 2 | `info-single` | `InfoOverlay` | Select one image and invoke `信息` from the radial menu or Command-I. | not-recorded | not-recorded | not-recorded | pending |
| INF-02 | Wave 2 | `info-multiple` | `InfoOverlay` aggregate | Select an image, text file and folder, then invoke `信息`. | not-recorded | not-recorded | not-recorded | pending |
| DIA-01 | Wave 3 | `dialog-settings` | `SettingsDialog` | Open `更多`, choose `软件设置`, and keep `显示与外观` selected. | not-recorded | not-recorded | not-recorded | pending |
| DIA-02 | Wave 3 | `dialog-single-rename` | `RenameDialog` | Select one disposable file, open radial `整理`, and choose `重命名`. | not-recorded | not-recorded | not-recorded | pending |
| DIA-03 | Wave 3 | `dialog-batch-rename` | `BatchRenameDialog` | Select three disposable files, choose radial `整理`, then `批量重命名` and enter a valid prefix. | not-recorded | not-recorded | not-recorded | pending |
| DIA-04 | Wave 3 | `dialog-destination` | `DestinationDialog` | Select one file, choose radial `整理` then `复制到`, and select `目标/Destination`. | not-recorded | not-recorded | not-recorded | pending |
| DIA-05 | Wave 3 | `dialog-conflict` | `DestinationDialog` conflict stage | Copy the same-named fixture file from `目标/Source` to `目标/Destination` and keep the real conflict decisions open. | not-recorded | not-recorded | not-recorded | pending |
| DIA-06 | Wave 3 | `dialog-trash` | `TrashConfirmation` | Select one disposable file and choose radial `移到废纸篓` without confirming. | not-recorded | not-recorded | not-recorded | pending |
| DIA-07 | Wave 3 | `dialog-close` | `CloseOperationDialog` | Start a real multi-file copy, then choose `关闭项目` before the operation finishes. | not-recorded | not-recorded | not-recorded | pending |
| TAS-01 | Wave 3 | `task-running` | `TaskBar` | Open a fresh 30-image fixture or start a real multi-file copy and capture while running. | not-recorded | not-recorded | not-recorded | pending |
| TAS-02 | Wave 3 | `task-success` | `TaskBar` clean success | Complete a real small copy and capture its success state before the 2-second dismissal. | not-recorded | not-recorded | not-recorded | pending |
| TAS-03 | Wave 3 | `task-failure` | `TaskBar` failure detail | Include the disposable unreadable file in a batch copy so the real operation reports a failure. | not-recorded | not-recorded | not-recorded | pending |
| TAS-04 | Wave 3 | `task-cancelled` | `TaskBar` cancelled state | Start a real multi-file copy or fresh scan and activate `取消任务` before completion. | not-recorded | not-recorded | not-recorded | pending |
| TAS-05 | Wave 3 | `task-result` | `TaskBar` result action | Complete a real mixed-result operation and leave `查看结果` visible. | not-recorded | not-recorded | not-recorded | pending |
| RES-01 | Wave 3 | `results-operation` | `OperationResults` | From a completed real operation choose `查看结果`. | not-recorded | not-recorded | not-recorded | pending |
| RES-02 | Wave 3 | `results-notice` | `GlobalNoticeStack` | Trigger a real recoverable context repair by renaming a selected disposable file in Finder. | not-recorded | not-recorded | not-recorded | pending |
| RES-03 | Wave 3 | `results-local-error` | `ViewerLocalFeedback` | Rename a previewed disposable file in Finder and request it again so only the preview module reports the failure. | not-recorded | not-recorded | not-recorded | pending |
| RES-04 | Wave 3 | `results-readonly` | `ReadOnlyBanner` | Open the read-only project copy and leave the permission strip visible. | not-recorded | not-recorded | not-recorded | pending |
| RES-05 | Wave 3 | `results-recovery` | recovery notice and results | Relaunch after interrupting a real operation, then open the recovery result from its notice. | not-recorded | not-recorded | not-recorded | pending |
| A11Y-01 | Wave 4 | `accessibility-keyboard` | formal primitives and modules | Starting at the project root, use only Tab, arrows, Enter, Space and shortcuts to reach sidebar, grid, toolbar, menus, radial menu and a dialog. | not-recorded | not-recorded | not-recorded | pending |
| A11Y-02 | Wave 4 | `accessibility-restore` | overlay and dialog focus restoration | Record the trigger, open then Escape-close filter, view, more, preview, radial menu, information and Settings, verifying focus returns each time. | not-recorded | not-recorded | not-recorded | pending |
| A11Y-03 | Wave 4 | `accessibility-reduced` | reduced-motion CSS | Enable macOS Reduce Motion, relaunch Viewer and exercise popover, dialog, task and loading transitions. | not-recorded | not-recorded | not-recorded | pending |
| A11Y-04 | Wave 4 | `accessibility-forced` | forced-colors CSS and high-contrast native surface | Enable macOS Increase Contrast and Reduce Transparency for both native captures; pair them with the automated `forced-colors` contract until Windows-native validation exists. | not-recorded | not-recorded | not-recorded | pending |
| A11Y-05 | Wave 4 | `accessibility-zoom` | responsive module CSS | Set Viewer to 200% effective zoom and minimum window size, then open filter, radial menu, preview, Settings, task and inspector states. | not-recorded | not-recorded | not-recorded | pending |
