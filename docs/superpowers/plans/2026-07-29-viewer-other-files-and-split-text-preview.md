# Viewer Other Files And Split Text Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Index every visible non-video file, present non-image content as `其它文件`, show explicit placeholders for unsupported formats, and preview up to two supported text files in an independent left/right split.

**Architecture:** The Rust scanner owns a centralized, extension-only classification registry and publishes behavior-oriented `FileKind` values. Browse and IPC contracts expose `images + otherFiles`, while the React UI uses kind predicates rather than duplicating extension rules. Unsupported files use request-free placeholder components, and text preview is decomposed into a one-dialog shell plus one independently loading pane per file.

**Tech Stack:** Rust 2024 workspace, Tokio, WalkDir, rusqlite, serde/Tauri 2 IPC, React 19, TypeScript 6, Vitest/Testing Library, Biome, CSS.

## Global Constraints

- Implement against `/Users/abc/Project/Viewer/.worktrees/adaptive-text-panel`, which contains the latest development source and uncommitted changes.
- Do not edit or stage the pre-existing dirty files `docs/PRODUCT_SPEC.md`, `ui/src/components/FolderTree.test.tsx`, `ui/src/components/FolderTree.tsx`, `ui/src/components/VirtualList.tsx`, `ui/src/styles/app.css`, or `ui/src/styles/app.test.ts`.
- Before every commit, run `git diff --cached --name-only` and confirm that none of the protected paths above are staged.
- Do not add a third-party dependency.
- Classification must use only a case-insensitive filename-extension lookup; scanning must not read file contents or probe MIME.
- Previewable images remain `.jpg`, `.jpeg`, and `.png`.
- Recognized unsupported images use the exact baseline registry in the approved design and must not request thumbnails or decoded image representations.
- Recognized videos `.mp4`, `.mov`, `.m4v`, `.avi`, `.mkv`, `.webm`, `.wmv`, `.flv`, `.mpeg`, and `.mpg` remain completely absent from indexed nodes and UI collections.
- Hidden dotfiles/directories, `.viewer`, `.DS_Store`, `Thumbs.db`, `desktop.ini`, symbolic links, and macOS aliases remain excluded.
- Generic other files are searchable by name and path only; only `.txt`, `.md`, and `.markdown` enter text-content indexing.
- Text preview retains UTF-8, UTF-16 LE/BE, and GB18030 decoding and the 10 MiB per-file limit.
- A radial preview accepts any one indexed file or exactly two previewable text files; no other multi-file preview selection is valid.
- Image comparison accepts 2–20 recognized images, including unsupported-image placeholders.
- Every new preview surface must use the existing light theme.
- Do not install, package, or launch `/Applications/Viewer.app`; visual verification uses `pnpm start:viewer` from this worktree.

---

## File Structure

### New files

- `crates/viewer-infrastructure/src/scan/file_classifier.rs` — centralized image/video/other extension registry and system-noise filename policy.
- `ui/src/fileKinds.ts` — kind-only UI capability predicates; it contains no extension registry.
- `ui/src/components/contentBrowser/OtherFilePanel.tsx` — adaptive, virtualized other-file shelf.
- `ui/src/components/contentBrowser/OtherFilePanel.test.tsx` — shelf behavior and interaction tests.
- `ui/src/components/contentBrowser/adaptiveOtherFilePanelModel.ts` — other-file content modes and select-all scopes.
- `ui/src/components/contentBrowser/adaptiveOtherFilePanelModel.test.ts` — pure layout/select-all policy tests.
- `ui/src/components/UnsupportedFileState.tsx` — reusable request-free unsupported message for cards, previews, and compare panes.
- `ui/src/components/UnsupportedFilePreview.tsx` — light single-file placeholder dialog.
- `ui/src/components/UnsupportedFilePreview.test.tsx` — generic unsupported preview tests.
- `ui/src/components/TextPreviewPane.tsx` — one text file's decoding, encoding, Markdown, scroll, and error state.
- `ui/src/state/previewPolicy.ts` — pure radial preview selection validation.
- `ui/src/state/previewPolicy.test.ts` — single-file and two-text policy tests.
- `ui/src/styles/adaptiveOtherFilePanel.css` — adaptive other-file shelf styles.
- `ui/src/styles/adaptiveOtherFilePanel.test.ts` — focused shelf layout contract tests.
- `ui/src/styles/filePreviewExtensions.css` — unsupported and split-text preview styles.

### Files replaced by clearer names

- Delete `ui/src/components/contentBrowser/TextFilePanel.tsx` after `OtherFilePanel.tsx` is wired.
- Delete `ui/src/components/contentBrowser/TextFilePanel.test.tsx` after its coverage moves.
- Delete `ui/src/components/contentBrowser/adaptiveTextPanelModel.ts` after the other-file model is wired.
- Delete `ui/src/components/contentBrowser/adaptiveTextPanelModel.test.ts` after its coverage moves.
- Delete `ui/src/styles/adaptiveTextPanel.css` and `ui/src/styles/adaptiveTextPanel.test.ts` after their other-file replacements are imported and tested.

### Principal modified files

- `crates/viewer-domain/src/file.rs` — new kinds and capability predicates.
- `crates/viewer-infrastructure/src/scan/mod.rs` and `walker.rs` — classifier integration for full scan and subtree reconciliation.
- `crates/viewer-infrastructure/src/search/index/mod.rs` — append-only persisted kind codes.
- `crates/viewer-infrastructure/src/portable/markers.rs` — append-only portable marker kind codes.
- `crates/viewer-infrastructure/src/operation/service/preflight.rs` — allow indexed unsupported/other regular files in file operations.
- `src-tauri/src/state/scan_index.rs` — derived indexing only for previewable images/text.
- `crates/viewer-application/src/browse.rs` — `other_files`, counts, partitioning, and selection summaries.
- `src-tauri/src/dto/browse.rs` — `otherFiles`, `otherFileCount`, and `otherFiles` selection counts.
- `ui/src/api/types.ts` and `ui/src/state/reducers/workspaceReducer.ts` — synchronized frontend contract.
- `ui/src/components/ContentBrowser.tsx` — other-file shelf, select-all, and image placeholder routing.
- `ui/src/components/ImagePreview.tsx` — unsupported-image single preview without image requests.
- `ui/src/components/ComparePane.tsx`, `CompareWorkspace.tsx`, and `ui/src/state/comparePolicy.ts` — unsupported-image compare placeholders and 20-image policy.
- `ui/src/components/TextPreview.tsx` — one- or two-pane dialog shell with aggregate task progress.
- `ui/src/components/radialMenuModel.ts` and `ui/src/App.tsx` — preview policy and split-text session routing.
- `ui/src/components/SearchToolbar.tsx`, `InfoOverlay.tsx`, and `FolderFilmstripRow.tsx` — updated labels, counts, and unsupported representative images.
- `ui/src/main.tsx` — import the two new focused style sheets.
- Existing Rust and UI test files listed in the tasks below — contract, regression, and interaction coverage.

---

### Task 1: Domain Kinds And Canonical Scanner Classification

**Files:**
- Create: `crates/viewer-infrastructure/src/scan/file_classifier.rs`
- Modify: `crates/viewer-domain/src/file.rs`
- Modify: `crates/viewer-infrastructure/src/scan/mod.rs`
- Modify: `crates/viewer-infrastructure/src/scan/walker.rs`
- Test: `tests/progressive_scan.rs`
- Test: `tests/m3_watcher_runtime.rs`

**Interfaces:**
- Produces: `FileKind::{UnsupportedImage, Other}`.
- Produces: `FileKind::{is_image,is_previewable_image,is_other_file,is_previewable_text}() -> bool`.
- Produces: `classify_regular_file(path: &Path) -> Option<FileKind>`, where `None` means recognized video.
- Produces: `is_ignored_entry_name(name: &OsStr) -> bool`.
- Consumes: Existing `ProjectWalker`, `snapshot_subtrees`, and no-follow safety checks.

- [ ] **Step 1: Write failing kind and scanner tests**

Add domain tests that lock the capability matrix:

```rust
#[test]
fn file_kinds_expose_behavior_without_extension_logic() {
    assert!(FileKind::Jpeg.is_image());
    assert!(FileKind::Png.is_previewable_image());
    assert!(FileKind::UnsupportedImage.is_image());
    assert!(!FileKind::UnsupportedImage.is_previewable_image());
    assert!(FileKind::Markdown.is_other_file());
    assert!(FileKind::Text.is_previewable_text());
    assert!(FileKind::Other.is_other_file());
    assert!(!FileKind::Other.is_previewable_text());
    assert!(!FileKind::Directory.is_other_file());
}
```

Expand `progressive_scan_publishes_folders_first_and_excludes_unsafe_entries` with:

```rust
project.create_file("poster.WEBP", b"unsupported image");
project.create_file("archive.zip", b"archive");
project.create_file("README", b"extensionless");
project.create_file("ignored.MOV", b"video");
project.create_file("Thumbs.db", b"system");
project.create_file("desktop.ini", b"system");
```

Collect `(relative_path, kind)` pairs and require:

```rust
assert!(files.contains(&("poster.WEBP".into(), FileKind::UnsupportedImage)));
assert!(files.contains(&("archive.zip".into(), FileKind::Other)));
assert!(files.contains(&("README".into(), FileKind::Other)));
assert!(!files.iter().any(|(path, _)| path == "ignored.MOV"));
assert!(!files.iter().any(|(path, _)| path == "Thumbs.db"));
assert!(!files.iter().any(|(path, _)| path == "desktop.ini"));
```

Update
`subtree_snapshot_excludes_hidden_reserved_unsupported_and_symlink_entries`
so `work/ignored.pdf` is expected as `FileKind::Other`; add
`work/poster.webp` as `FileKind::UnsupportedImage` and `work/clip.mov` as an
absent video. Rename the indexed PDF to `work/ignored.webp`, reconcile `work`,
and assert the same entity is reclassified from `Other` to
`UnsupportedImage`. Rename it again to `work/ignored.mov`, reconcile, and
assert that entity is removed from the session index.

- [ ] **Step 2: Run the focused tests and verify failure**

Run:

```bash
cargo test --locked -p viewer-domain file_kinds_expose_behavior_without_extension_logic
cargo test --locked -p viewer-infrastructure --test progressive_scan progressive_scan_publishes_folders_first_and_excludes_unsafe_entries
```

Expected: compilation fails because `UnsupportedImage`, `Other`, and the predicate methods do not exist; after only the enum is added, the scan assertion still fails because unknown files are omitted.

- [ ] **Step 3: Implement the enum predicates and extension registry**

Append enum variants without changing existing serialized names:

```rust
pub enum FileKind {
    Directory,
    Jpeg,
    Png,
    Markdown,
    Text,
    UnsupportedImage,
    Other,
}

impl FileKind {
    pub const fn is_image(self) -> bool {
        matches!(self, Self::Jpeg | Self::Png | Self::UnsupportedImage)
    }

    pub const fn is_previewable_image(self) -> bool {
        matches!(self, Self::Jpeg | Self::Png)
    }

    pub const fn is_other_file(self) -> bool {
        matches!(self, Self::Markdown | Self::Text | Self::Other)
    }

    pub const fn is_previewable_text(self) -> bool {
        matches!(self, Self::Markdown | Self::Text)
    }
}
```

Implement the classifier with the approved registries:

```rust
const UNSUPPORTED_IMAGE_EXTENSIONS: &[&str] = &[
    "gif", "webp", "avif", "heic", "heif", "bmp", "tif", "tiff", "svg", "svgz",
    "ico", "jxl", "jfif", "apng", "psd", "dng", "cr2", "cr3", "nef", "nrw",
    "arw", "srf", "sr2", "raf", "orf", "rw2", "pef", "srw", "x3f",
];

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mov", "m4v", "avi", "mkv", "webm", "wmv", "flv", "mpeg", "mpg",
];

pub(crate) fn classify_regular_file(path: &Path) -> Option<FileKind> {
    let extension = path
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("jpg" | "jpeg") => Some(FileKind::Jpeg),
        Some("png") => Some(FileKind::Png),
        Some("md" | "markdown") => Some(FileKind::Markdown),
        Some("txt") => Some(FileKind::Text),
        Some(value) if UNSUPPORTED_IMAGE_EXTENSIONS.contains(&value) => {
            Some(FileKind::UnsupportedImage)
        }
        Some(value) if VIDEO_EXTENSIONS.contains(&value) => None,
        _ => Some(FileKind::Other),
    }
}

pub(crate) fn is_ignored_entry_name(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    let lowercase = name.to_ascii_lowercase();
    name.starts_with('.')
        || matches!(lowercase.as_str(), "thumbs.db" | "desktop.ini")
}
```

Put the complete approved unsupported-image and video lists in private constant slices. Wire both full scan and `snapshot_subtrees` through the same classifier by changing only `is_visible_entry` and the regular-file branch of `node_from_entry`.

- [ ] **Step 4: Run classification and reconciliation coverage**

Run:

```bash
cargo fmt --all -- --check
cargo test --locked -p viewer-domain
cargo test --locked -p viewer-infrastructure --test progressive_scan
cargo test --locked -p viewer-infrastructure --test m3_watcher_runtime
```

Expected: all tests pass; ignored videos and system noise are absent, while unsupported images and arbitrary files are published.

- [ ] **Step 5: Commit the canonical classification**

```bash
git add crates/viewer-domain/src/file.rs \
  crates/viewer-infrastructure/src/scan/file_classifier.rs \
  crates/viewer-infrastructure/src/scan/mod.rs \
  crates/viewer-infrastructure/src/scan/walker.rs \
  tests/progressive_scan.rs \
  tests/m3_watcher_runtime.rs
git diff --cached --name-only
git commit -m "feat: classify images videos and other files"
```

---

### Task 2: Persist And Operate On The New File Kinds Safely

**Files:**
- Modify: `crates/viewer-infrastructure/src/search/index/mod.rs`
- Modify: `crates/viewer-infrastructure/src/portable/markers.rs`
- Modify: `crates/viewer-infrastructure/src/operation/service/preflight.rs`
- Modify: `src-tauri/src/state/scan_index.rs`
- Modify: `src-tauri/src/state/markers.rs`
- Test: `tests/progressive_scan.rs`
- Test: `tests/m2_portable_metadata.rs`
- Test: `tests/m2_derived_indexing.rs`
- Test: `tests/m3_file_commands.rs`

**Interfaces:**
- Consumes: Task 1 `FileKind` predicates and variants.
- Produces: stable database codes `UnsupportedImage = 5`, `Other = 6`.
- Produces: marker, copy, move, rename, trash, and watcher projection support for both new indexed kinds.
- Preserves: derived indexing only for `is_previewable_image()` and `is_previewable_text()`.

- [ ] **Step 1: Add failing round-trip and derived-work tests**

Extend the session-index reopen test with:

```rust
indexed_node(EntityId::new(), "assets/source.psd", FileKind::UnsupportedImage),
indexed_node(EntityId::new(), "assets/license.pdf", FileKind::Other),
```

After reopening, assert both kinds round-trip exactly. Extend portable marker coverage:

```rust
let targets = [
    marker_target("assets/source.psd", FileKind::UnsupportedImage),
    marker_target("assets/license.pdf", FileKind::Other),
];
let patch = MarkerPatch {
    review: ReviewPatch::Unchanged,
    favorite: FavoritePatch::Set(true),
};
store.apply_batch(&targets, patch, 42).unwrap();
let restored = store.markers_for_paths(
    &targets.iter().map(|target| target.relative_path.clone()).collect::<Vec<_>>(),
).unwrap();
assert_eq!(
    restored.iter().map(|marker| marker.kind).collect::<Vec<_>>(),
    [FileKind::UnsupportedImage, FileKind::Other]
);
```

Add a derived-index regression proving generic and unsupported-image nodes never invoke image or text extraction, and add a file-command assertion that `FileKind::Other` is an accepted regular source.

- [ ] **Step 2: Run focused persistence and operation tests**

Run:

```bash
cargo test --locked -p viewer-infrastructure --test progressive_scan session_index_commits_a_batch_and_reopens_with_the_same_hierarchy
cargo test --locked -p viewer-infrastructure --test m2_portable_metadata
cargo test --locked -p viewer-infrastructure --test m2_derived_indexing
cargo test --locked -p viewer-infrastructure --test m3_file_commands trash_records_intent_and_only_accepts_supported_regular_files
```

Expected: persisted-value decoding or exhaustive matches fail for kinds 5 and 6, and the operation preflight rejects `Other`.

- [ ] **Step 3: Append kind codes and use capability predicates**

Keep codes 0–4 unchanged and append:

```rust
pub(super) fn encode_kind(kind: FileKind) -> i64 {
    match kind {
        FileKind::Directory => 0,
        FileKind::Jpeg => 1,
        FileKind::Png => 2,
        FileKind::Markdown => 3,
        FileKind::Text => 4,
        FileKind::UnsupportedImage => 5,
        FileKind::Other => 6,
    }
}

fn decode_kind(value: i64) -> rusqlite::Result<FileKind> {
    match value {
        0 => Ok(FileKind::Directory),
        1 => Ok(FileKind::Jpeg),
        2 => Ok(FileKind::Png),
        3 => Ok(FileKind::Markdown),
        4 => Ok(FileKind::Text),
        5 => Ok(FileKind::UnsupportedImage),
        6 => Ok(FileKind::Other),
        _ => Err(persisted_error("kind", value)),
    }
}
```

Apply the same mapping in `portable/markers.rs`. Change regular-file operation validation to:

```rust
fn supported_regular_kind(kind: FileKind) -> bool {
    kind != FileKind::Directory
}
```

In `scan_index.rs`, select derived work with:

```rust
for node in nodes
    .into_iter()
    .filter(|node| node.kind.is_previewable_image() || node.kind.is_previewable_text())
{
    let pending = if node.kind.is_previewable_image() {
        current.image_status == ImageIndexStatus::Pending
    } else {
        current.text_status == TextIndexStatus::Pending
    };
    if !pending {
        continue;
    }
    let source = validated_indexed_source(&active, &node)
        .map(|(source, _, _)| source)
        .ok();
    match node.kind {
        FileKind::Jpeg | FileKind::Png => {
            let result = match source {
                Some(source) => match image.probe(&source).await {
                    Ok(probe) => {
                        let (width, height) = if matches!(probe.orientation, 5..=8) {
                            (probe.height, probe.width)
                        } else {
                            (probe.width, probe.height)
                        };
                        Ok(ImageMetadata { width, height })
                    }
                    Err(_) => Err(ImageIndexStatus::Failed),
                },
                None => Err(ImageIndexStatus::Failed),
            };
            if let Err(error) =
                index.replace_image_metadata(node.entity_id, &node.relative_path, result)
            {
                if is_stale_derived_write_error(&error) {
                    continue;
                }
                return Err(CommandError::from(error));
            }
        }
        FileKind::Markdown | FileKind::Text => match source {
            Some(source) => {
                let extracted =
                    tokio::task::spawn_blocking(move || TextExtractor::extract(source)).await;
                match extracted {
                    Ok(Ok(status)) => {
                        if let Err(error) =
                            index.replace_text(node.entity_id, &node.relative_path, &status)
                        {
                            if is_stale_derived_write_error(&error) {
                                continue;
                            }
                            return Err(CommandError::from(error));
                        }
                    }
                    Ok(Err(_)) | Err(_) => {
                        if let Err(error) =
                            index.mark_text_failed(node.entity_id, &node.relative_path)
                        {
                            if is_stale_derived_write_error(&error) {
                                continue;
                            }
                            return Err(CommandError::from(error));
                        }
                    }
                }
            }
            None => {
                if let Err(error) =
                    index.mark_text_failed(node.entity_id, &node.relative_path)
                {
                    if is_stale_derived_write_error(&error) {
                        continue;
                    }
                    return Err(CommandError::from(error));
                }
            }
        },
        FileKind::Directory | FileKind::UnsupportedImage | FileKind::Other => {
            unreachable!("non-derived kinds were filtered out")
        }
    }
}
```

This preserves the current image-probe, text-extraction, and stale-write
behavior while making the new non-derived kinds explicit.

In `state/markers.rs`, accept every indexed regular kind as a file:

```rust
let kind_matches = match node.kind {
    FileKind::Directory => metadata.is_dir(),
    FileKind::Jpeg
    | FileKind::Png
    | FileKind::Markdown
    | FileKind::Text
    | FileKind::UnsupportedImage
    | FileKind::Other => metadata.is_file(),
};
```

- [ ] **Step 4: Run persistence, indexing, watcher, and operation suites**

Run:

```bash
cargo fmt --all -- --check
cargo test --locked -p viewer-infrastructure --test progressive_scan
cargo test --locked -p viewer-infrastructure --test m2_portable_metadata
cargo test --locked -p viewer-infrastructure --test m2_derived_indexing
cargo test --locked -p viewer-infrastructure --test m3_operation_projections
cargo test --locked -p viewer-infrastructure --test m3_file_commands
cargo test --locked -p viewer-desktop --test m3_watcher_runtime
```

Expected: all tests pass; existing kind codes remain readable and new kinds do not receive derived work.

- [ ] **Step 5: Commit storage and operation support**

```bash
git add crates/viewer-infrastructure/src/search/index/mod.rs \
  crates/viewer-infrastructure/src/portable/markers.rs \
  crates/viewer-infrastructure/src/operation/service/preflight.rs \
  src-tauri/src/state/scan_index.rs \
  src-tauri/src/state/markers.rs \
  tests/progressive_scan.rs \
  tests/m2_portable_metadata.rs \
  tests/m2_derived_indexing.rs \
  tests/m3_file_commands.rs
git diff --cached --name-only
git commit -m "feat: persist and operate on other file kinds"
```

---

### Task 3: Migrate Browse And IPC Contracts To Other Files

**Files:**
- Modify: `crates/viewer-application/src/browse.rs`
- Modify: `src-tauri/src/dto/browse.rs`
- Modify: `scripts/check-m1-ipc-fixtures.mjs`
- Modify: `tests/fixtures/m1-ipc/browse-session.json`
- Test: `tests/m1_browse_queries.rs`
- Test: `tests/m2_browse_projections.rs`
- Test: `tests/m1_desktop_runtime.rs`
- Test: `tests/m2_desktop_runtime.rs`

**Interfaces:**
- Consumes: Task 1 kind predicates and Task 2 stable persistence.
- Produces: `FolderWorkspace::Content { images, other_files }`.
- Produces: IPC JSON `{ workspace: "content", images, otherFiles }`.
- Produces: `ContentFolderCard.other_file_count` / `otherFileCount`.
- Produces: `SelectionTypeCounts.other_files` / `types.otherFiles`.

- [ ] **Step 1: Rewrite backend expectations before production structs**

Change browse tests to include:

```rust
("id-001/source.webp", FileKind::UnsupportedImage),
("id-001/notes.txt", FileKind::Text),
("id-001/license.pdf", FileKind::Other),
```

Assert the new partition:

```rust
let FolderWorkspace::Content { images, other_files } = workspace else {
    panic!("id-001 should be a content workspace")
};
assert_eq!(
    names(&images),
    ["01.jpg", "02.png", "source.webp"]
);
assert_eq!(
    names(&other_files),
    ["license.pdf", "notes.txt"]
);
```

Update selection summaries to:

```rust
SelectionTypeCounts {
    folders: 0,
    images: 2,
    other_files: 2,
}
```

Update the IPC fixture key to `otherFiles`, add one `unsupported_image` and one `other` record, and add both serialized kinds to the fixture validator's allowlist.

- [ ] **Step 2: Run browse and desktop contract tests**

Run:

```bash
cargo test --locked -p viewer-infrastructure --test m1_browse_queries
cargo test --locked -p viewer-infrastructure --test m2_browse_projections
cargo test --locked -p viewer-desktop --test m1_desktop_runtime
cargo test --locked -p viewer-desktop --test m2_desktop_runtime
node scripts/check-m1-ipc-fixtures.mjs
```

Expected: compilation fails on the old `text_files` and `text_count` fields, and the fixture validator initially rejects the new serialized kinds.

- [ ] **Step 3: Implement the backend vocabulary and partition**

Use these application types:

```rust
pub struct ContentFolderCard {
    pub entity_id: EntityId,
    pub relative_path: RelativePath,
    pub name: String,
    pub marker: Marker,
    pub image_count: u64,
    pub other_file_count: u64,
    pub review_progress: FolderReviewProgress,
    pub representative_images: Vec<BrowserFile>,
}

pub enum FolderWorkspace {
    Category { folders: Vec<ContentFolderCard> },
    Content {
        images: Vec<BrowserFile>,
        other_files: Vec<BrowserFile>,
    },
    Empty,
}

pub struct SelectionTypeCounts {
    pub folders: u64,
    pub images: u64,
    pub other_files: u64,
}
```

Replace `split_files` with capability predicates:

```rust
fn split_files(nodes: Vec<IndexedNode>, sort: SearchSort)
    -> (Vec<BrowserFile>, Vec<BrowserFile>)
{
    let mut images = Vec::new();
    let mut other_files = Vec::new();
    for indexed in nodes {
        if indexed.node.kind.is_image() {
            images.push(indexed.into());
        } else if indexed.node.kind.is_other_file() {
            other_files.push(indexed.into());
        }
    }
    images.sort_by(|left, right| compare_files(left, right, sort));
    other_files.sort_by(|left, right| compare_files(left, right, sort));
    (images, other_files)
}
```

Map the DTO with serde's normal camelCase output:

```rust
Content {
    images: Vec<BrowserFileDto>,
    other_files: Vec<BrowserFileDto>,
}
```

Do not retain a `textFiles` compatibility field.

- [ ] **Step 4: Run all browse, IPC fixture, and search tests**

Run:

```bash
cargo fmt --all -- --check
cargo test --locked -p viewer-infrastructure --test m1_browse_queries
cargo test --locked -p viewer-infrastructure --test m2_browse_projections
cargo test --locked -p viewer-infrastructure --test m2_search_queries
cargo test --locked -p viewer-desktop --test m1_desktop_runtime
cargo test --locked -p viewer-desktop --test m2_desktop_runtime
node scripts/check-m1-ipc-fixtures.mjs
```

Expected: all pass; generic other files are searchable by filename/path and never produce body hits.

- [ ] **Step 5: Commit the browse and IPC migration**

```bash
git add crates/viewer-application/src/browse.rs \
  src-tauri/src/dto/browse.rs \
  scripts/check-m1-ipc-fixtures.mjs \
  tests/fixtures/m1-ipc/browse-session.json \
  tests/m1_browse_queries.rs \
  tests/m2_browse_projections.rs \
  tests/m1_desktop_runtime.rs \
  tests/m2_desktop_runtime.rs
git diff --cached --name-only
git commit -m "feat: expose images and other files in browse contracts"
```

---

### Task 4: Build The Adaptive Other-File Shelf And Single Unsupported Preview

**Files:**
- Create: `ui/src/fileKinds.ts`
- Create: `ui/src/components/contentBrowser/OtherFilePanel.tsx`
- Create: `ui/src/components/contentBrowser/OtherFilePanel.test.tsx`
- Create: `ui/src/components/contentBrowser/adaptiveOtherFilePanelModel.ts`
- Create: `ui/src/components/contentBrowser/adaptiveOtherFilePanelModel.test.ts`
- Create: `ui/src/components/UnsupportedFileState.tsx`
- Create: `ui/src/components/UnsupportedFilePreview.tsx`
- Create: `ui/src/components/UnsupportedFilePreview.test.tsx`
- Create: `ui/src/styles/adaptiveOtherFilePanel.css`
- Create: `ui/src/styles/adaptiveOtherFilePanel.test.ts`
- Modify: `ui/src/api/types.ts`
- Modify: `ui/src/state/reducers/workspaceReducer.ts`
- Modify: `ui/src/components/ContentBrowser.tsx`
- Modify: `ui/src/components/contentBrowser/SelectAllChoicePanel.tsx`
- Modify: `ui/src/components/contentBrowser/SelectAllChoicePanel.test.tsx`
- Modify: `ui/src/components/SearchToolbar.tsx`
- Modify: `ui/src/components/InfoOverlay.tsx`
- Modify: `ui/src/components/InfoOverlay.test.tsx`
- Modify: `ui/src/components/FolderFilmstripRow.tsx`
- Modify: `ui/src/components/FolderFilmstripRow.test.tsx`
- Modify: `ui/src/components/FolderOverview.test.tsx`
- Modify: `ui/src/components/MarkerControls.test.tsx`
- Modify: `ui/src/main.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/state/viewerReducer.test.ts`
- Modify: `ui/src/state/useViewerController.test.tsx`
- Delete after replacements pass: old text-panel model, component, CSS, and their tests listed in File Structure.

**Interfaces:**
- Consumes: Task 3 `otherFiles`, `otherFileCount`, and `types.otherFiles`.
- Produces: UI predicates `isImageFile`, `isPreviewableImage`, `isOtherFile`, and `isPreviewableText`.
- Produces: `AdaptiveContentMode` with `other_only`.
- Produces: `SelectAllScope = "images" | "other" | "all"`.
- Produces: request-free `UnsupportedFilePreview({ file, unavailable, onClose })`.
- Reuses without editing: protected `VirtualList.tsx`.

- [ ] **Step 1: Write failing contract, shelf, and unsupported-other tests**

Define fixtures with:

```ts
const workspace = {
  workspace: 'content' as const,
  images: [file('image', 'jpeg')],
  otherFiles: [
    file('notes', 'text'),
    file('license', 'other'),
  ],
}
```

Require the pure model:

```ts
expect(resolveAdaptiveContentMode(0, 0, false)).toBe('empty')
expect(resolveAdaptiveContentMode(1, 0, true)).toBe('image_only')
expect(resolveAdaptiveContentMode(1, 2, false)).toBe('mixed_collapsed')
expect(resolveAdaptiveContentMode(1, 2, true)).toBe('mixed_expanded')
expect(resolveAdaptiveContentMode(0, 2, false)).toBe('other_only')
expect(resolveSelectAllRequest(0, 0)).toEqual({ kind: 'none' })
expect(resolveSelectAllRequest(2, 0)).toEqual({ kind: 'direct', scope: 'images' })
expect(resolveSelectAllRequest(0, 2)).toEqual({ kind: 'direct', scope: 'other' })
expect(resolveSelectAllRequest(1, 2)).toEqual({ kind: 'choice' })
expect(filesForSelectAllScope(workspace, 'other')).toEqual(workspace.otherFiles)
expect(filesForSelectAllScope(workspace, 'all')).toEqual([
  ...workspace.images,
  ...workspace.otherFiles,
])
```

Require shelf UI:

```ts
expect(screen.getByRole('button', { name: '其它文件 · 2' })).toHaveAttribute(
  'aria-expanded',
  'false',
)
expect(screen.queryByText('文本文件')).not.toBeInTheDocument()
```

Collapse and re-expand with selected other files, then require the row selection
to persist. Rerender after removing the last other file and require the shelf to
unmount; rerender after removing the last image and require the shelf to switch
to the full-body `other_only` mode.

Port the CSS contract test to the renamed selectors and require:

```ts
expect(declarationsFor(rules, '.other-file-panel--mixed-expanded')).toEqual({
  flex: '0 1 auto',
  'max-height': '20%',
})
expect(declarationsFor(rules, '.other-file-panel--other_only')).toEqual({
  flex: '1 1 auto',
  'max-height': 'none',
})
expect(declarationsFor(rules, '.other-file-listbox')).toMatchObject({
  flex: '1 1 auto',
  'min-height': '0',
  overflow: 'hidden',
})
```

Require select-all labels `全选图片`, `全选其它文件`, and `全部都选`. Require generic other preview:

```ts
render(<UnsupportedFilePreview file={file('license', 'other')} onClose={onClose} />)
expect(screen.getByRole('dialog', { name: 'license' })).toHaveTextContent('暂不支持预览')
expect(screen.getByText('.OTHER')).toBeInTheDocument()
expect(screen.queryByRole('button', { name: /Quick Look|默认应用/ })).not.toBeInTheDocument()
```

In `App.test.tsx`, double-click a generic other row and assert the unsupported
dialog opens while the bridge's `previewText` spy remains untouched.

- [ ] **Step 2: Run the focused UI tests and verify failure**

Run:

```bash
pnpm --dir ui test -- \
  src/components/contentBrowser/adaptiveOtherFilePanelModel.test.ts \
  src/components/contentBrowser/OtherFilePanel.test.tsx \
  src/components/contentBrowser/SelectAllChoicePanel.test.tsx \
  src/components/UnsupportedFilePreview.test.tsx \
  src/styles/adaptiveOtherFilePanel.test.ts
```

Expected: type checking or module resolution fails because the new contract, components, and model do not exist.

- [ ] **Step 3: Implement the UI contract and other-file shelf**

Use this frontend kind policy:

```ts
export const isImageFile = (file: Pick<BrowserFile, 'kind'>) =>
  file.kind === 'jpeg' || file.kind === 'png' || file.kind === 'unsupported_image'

export const isPreviewableImage = (file: Pick<BrowserFile, 'kind'>) =>
  file.kind === 'jpeg' || file.kind === 'png'

export const isOtherFile = (file: Pick<BrowserFile, 'kind'>) =>
  file.kind === 'markdown' || file.kind === 'text' || file.kind === 'other'

export const isPreviewableText = (file: Pick<BrowserFile, 'kind'>) =>
  file.kind === 'markdown' || file.kind === 'text'
```

Update types:

```ts
export type FileKind =
  | 'directory'
  | 'jpeg'
  | 'png'
  | 'markdown'
  | 'text'
  | 'unsupported_image'
  | 'other'

export type FolderWorkspace =
  | { workspace: 'category'; folders: ContentFolderCard[] }
  | { workspace: 'content'; images: BrowserFile[]; otherFiles: BrowserFile[] }
  | { workspace: 'empty' }

export interface ContentFolderCard {
  entityId: string
  relativePath: string
  name: string
  marker: Marker
  imageCount: number
  otherFileCount: number
  reviewProgress: FolderReviewProgress
  representativeImages: BrowserFile[]
}
```

Render other rows through the existing virtual list inside the accessible listbox:

```tsx
<div
  id={OTHER_LIST_ID}
  role="listbox"
  aria-label="其它文件"
  aria-activedescendant={activeId ? `file-${activeId}` : undefined}
  tabIndex={0}
  className="other-file-listbox"
  style={
    mode === 'mixed_expanded'
      ? { height: files.length * OTHER_FILE_ROW_HEIGHT }
      : undefined
  }
  onKeyDown={handleListKeyDown}
>
  <VirtualList
    items={files}
    rowHeight={OTHER_FILE_ROW_HEIGHT}
    getKey={(file) => file.entityId}
    scrollToIndex={activeIndex >= 0 ? activeIndex : undefined}
    className="other-file-virtual-list"
    renderItem={(file) => (
      <div
        role="option"
        id={`file-${file.entityId}`}
        aria-label={file.name}
        aria-selected={selectedIds.has(file.entityId)}
        className="text-file-row other-file-row"
        onPointerDown={(event) => onRadialMenuPointerDown(file, event)}
        onContextMenu={(event) => onRadialMenuContextMenu(file, event)}
        onClick={(event) => onSelect(file, event)}
        onDoubleClick={() => onPreview(file)}
      >
        <span className="text-file-name">{file.name}</span>
        <span className="text-file-path">{file.relativePath}</span>
      </div>
    )}
  />
</div>
```

Set `OTHER_FILE_ROW_HEIGHT = 56`. In `adaptiveOtherFilePanel.css`, rename the
existing bounded flex rules and preserve their exact behavior:

```css
.other-file-panel--mixed-expanded {
  flex: 0 1 auto;
  max-height: 20%;
}

.other-file-panel--other_only {
  flex: 1 1 auto;
  max-height: none;
}

.other-file-listbox {
  flex: 1 1 auto;
  min-height: 0;
  overflow: hidden;
}

.other-file-virtual-list {
  min-height: 0;
  overscroll-behavior: contain;
}
```

The mixed listbox's row-derived inline height supplies intrinsic height for a
short list, while the panel's 20% cap shrinks long lists. The existing
auto-measuring `VirtualList` observes the actual clipped viewport, so it only
mounts visible rows plus overscan. In `other_only`, omit the inline height and
let the listbox fill the bounded content body.

Keep `text-file-row` as a compatibility class on the row so the protected
`app.css` need not change; add `other-file-row` for new styles. Copy the
existing Finder-drag attributes, organization-drag attributes, marker controls,
and radial-menu pointer/context handlers from `TextFilePanel` into the
corresponding `OtherFilePanel` row without changing their callback signatures.
Replace every user-facing `文本文件` label with `其它文件`, rename every migrated
contract field from `textFiles` to `otherFiles`, and import
`adaptiveOtherFilePanel.css` from `main.tsx`.

Add the new filter and information labels explicitly:

```ts
const FILE_KINDS: Array<[FileKind, string]> = [
  ['jpeg', 'JPEG'],
  ['png', 'PNG'],
  ['unsupported_image', '其它图片'],
  ['markdown', 'Markdown'],
  ['text', 'TXT'],
  ['other', '其它文件'],
  ['directory', '文件夹'],
]
```

`InfoOverlay` reports `文件夹 N · 图片 N · 其它文件 N`, and folder filmstrip
copy reports `${folder.otherFileCount} 个其它文件`.

Use one extension-label function for unsupported files:

```ts
export function fileExtensionLabel(name: string): string {
  const separator = name.lastIndexOf('.')
  if (separator <= 0 || separator === name.length - 1) return '无扩展名'
  return `.${name.slice(separator + 1).toUpperCase()}`
}
```

Use that helper in the reusable placeholder contract:

```ts
export interface UnsupportedFileStateProps {
  file: Pick<BrowserFile, 'name'>
  compact?: boolean
  unavailable?: boolean
}

const format = fileExtensionLabel(file.name)
const status = unavailable ? '文件已不可用' : '暂不支持预览'
const accessibleLabel = `${file.name} ${format} ${status}`
```

Render the filename, `format`, and `status` inside one element with
`aria-label={accessibleLabel}`. `UnsupportedFilePreview` adds the existing
light `role="dialog"` shell and close button only; it never calls a backend
preview command or offers Quick Look/default-application actions.

Route a generic `other` single preview to `UnsupportedFilePreview` in `App.tsx`;
Markdown/TXT continue to route to `TextPreview`. Task 4 test fixtures include
supported images only; Task 5 adds the request-suppression branches required
before unsupported-image fixtures enter the full UI regression suite.

- [ ] **Step 4: Run shelf, state, search, information, and component suites**

Run:

```bash
pnpm --dir ui test -- \
  src/components/ContentBrowser.test.tsx \
  src/components/contentBrowser/OtherFilePanel.test.tsx \
  src/components/contentBrowser/adaptiveOtherFilePanelModel.test.ts \
  src/components/contentBrowser/SelectAllChoicePanel.test.tsx \
  src/components/UnsupportedFilePreview.test.tsx \
  src/components/FolderFilmstripRow.test.tsx \
  src/components/FolderOverview.test.tsx \
  src/components/InfoOverlay.test.tsx \
  src/components/SearchToolbar.test.tsx \
  src/state/viewerReducer.test.ts \
  src/state/useViewerController.test.tsx
pnpm --dir ui check
```

Expected: all pass; no rendered `文本文件` label remains, and generic other files never call `previewText`.

- [ ] **Step 5: Commit the other-file UI migration**

Stage only the new/renamed panel, contract, reducer, label, fixture, and focused style files. Confirm the protected files are absent:

```bash
git add ui/src/fileKinds.ts \
  ui/src/api/types.ts \
  ui/src/state/reducers/workspaceReducer.ts \
  ui/src/components/ContentBrowser.tsx \
  ui/src/components/ContentBrowser.test.tsx \
  ui/src/components/contentBrowser/OtherFilePanel.tsx \
  ui/src/components/contentBrowser/OtherFilePanel.test.tsx \
  ui/src/components/contentBrowser/adaptiveOtherFilePanelModel.ts \
  ui/src/components/contentBrowser/adaptiveOtherFilePanelModel.test.ts \
  ui/src/components/contentBrowser/SelectAllChoicePanel.tsx \
  ui/src/components/contentBrowser/SelectAllChoicePanel.test.tsx \
  ui/src/components/UnsupportedFileState.tsx \
  ui/src/components/UnsupportedFilePreview.tsx \
  ui/src/components/UnsupportedFilePreview.test.tsx \
  ui/src/components/SearchToolbar.tsx \
  ui/src/components/InfoOverlay.tsx \
  ui/src/components/InfoOverlay.test.tsx \
  ui/src/components/FolderFilmstripRow.tsx \
  ui/src/components/FolderFilmstripRow.test.tsx \
  ui/src/components/FolderOverview.test.tsx \
  ui/src/components/MarkerControls.test.tsx \
  ui/src/styles/adaptiveOtherFilePanel.css \
  ui/src/styles/adaptiveOtherFilePanel.test.ts \
  ui/src/main.tsx \
  ui/src/App.tsx \
  ui/src/App.test.tsx \
  ui/src/state/viewerReducer.test.ts \
  ui/src/state/useViewerController.test.tsx
git add -u ui/src/components/contentBrowser ui/src/styles/adaptiveTextPanel.css ui/src/styles/adaptiveTextPanel.test.ts
git diff --cached --name-only
git commit -m "feat: replace text shelf with other files"
```

---

### Task 5: Render Unsupported Images Without Image Requests

**Files:**
- Create: `ui/src/styles/filePreviewExtensions.css`
- Modify: `ui/src/components/contentBrowser/ImageCell.tsx`
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/components/ImagePreview.tsx`
- Modify: `ui/src/components/ImagePreview.test.tsx`
- Modify: `ui/src/state/comparePolicy.ts`
- Modify: `ui/src/state/comparePolicy.test.ts`
- Modify: `ui/src/state/compareModel.test.ts`
- Modify: `ui/src/components/ComparePane.tsx`
- Modify: `ui/src/components/ComparePane.test.tsx`
- Modify: `ui/src/components/CompareWorkspace.tsx`
- Modify: `ui/src/components/CompareWorkspace.test.tsx`
- Modify: `ui/src/components/FolderFilmstripRow.tsx`
- Modify: `ui/src/components/FolderFilmstripRow.test.tsx`
- Modify: `ui/src/main.tsx`

**Interfaces:**
- Consumes: Task 4 `isImageFile`, `isPreviewableImage`, and `UnsupportedFileState`.
- Produces: unsupported image cards, single previews, folder filmstrip cells, and compare panes that issue zero image requests.
- Produces: compare validation accepting `unsupported_image` while retaining the 2–20 limit.
- Produces: optional `unavailableEntityIds` preview input for watcher-removed files.

- [ ] **Step 1: Add failing request-suppression and placeholder tests**

In `ContentBrowser.test.tsx`, render one JPEG and one unsupported image:

```ts
const requestThumbnail = vi.fn().mockResolvedValue('viewer-image://thumbnail')
renderBrowser({
  images: [
    image('supported', 'jpeg'),
    image('raw', 'unsupported_image'),
  ],
  otherFiles: [],
}, { requestThumbnail })

await waitFor(() => expect(requestThumbnail).toHaveBeenCalled())
expect(requestThumbnail.mock.calls.map(([file]) => file.entityId)).toEqual(['supported'])
expect(screen.getByLabelText('raw 暂不支持预览')).toBeInTheDocument()
```

In `ImagePreview.test.tsx`, require an unsupported current file to show the
message and make zero calls. In `ComparePane.test.tsx`, require the same for a
compare panel. Update compare policy expectations:

```ts
expect(validateCompareCandidates([
  { entityId: 'jpg', kind: 'jpeg' },
  { entityId: 'raw', kind: 'unsupported_image' },
])).toEqual({ ok: true })
```

Add a folder-filmstrip assertion that unsupported representatives render a
placeholder and never invoke `requestThumbnail`. Rerender `ImagePreview` with
the current entity in `unavailableEntityIds`; require `文件已不可用`, zero new
image requests, and working previous/next navigation.

- [ ] **Step 2: Run unsupported-image tests and verify failure**

Run:

```bash
pnpm --dir ui test -- \
  src/components/ContentBrowser.test.tsx \
  src/components/ImagePreview.test.tsx \
  src/components/ComparePane.test.tsx \
  src/components/CompareWorkspace.test.tsx \
  src/components/FolderFilmstripRow.test.tsx \
  src/state/comparePolicy.test.ts \
  src/state/compareModel.test.ts
```

Expected: unsupported images are rejected by compare validation or trigger
thumbnail/image requests.

- [ ] **Step 3: Add request-free branches to every image surface**

In `ImageCell`, branch before `AspectThumbnail`:

```tsx
{isPreviewableImage(file) ? (
  <AspectThumbnail
    file={file}
    width={rect.imageWidth}
    height={rect.imageHeight}
    dimensionsKnown={dimensionsKnown}
    loadThumbnail={loadThumbnail}
    onNaturalDimensions={onNaturalDimensions}
  />
) : (
  <UnsupportedFileState file={file} compact />
)}
```

In `ImagePreview`, skip unsupported candidates while prefetching:

```ts
for (const candidate of windowFiles) {
  if (
    !isPreviewableImage(candidate) ||
    unavailableEntityIds.has(candidate.entityId) ||
    fitCache.current.has(candidate.entityId) ||
    pendingFit.current.has(candidate.entityId)
  ) {
    continue
  }
  const request = requestImage(candidate, {
    kind: 'fit_preview',
    maxWidth: 2_400,
    maxHeight: 2_400,
    scaleMilli: 1_000,
  })
  pendingFit.current.set(candidate.entityId, request)
  void request.then(
    (representation) => {
      pendingFit.current.delete(candidate.entityId)
      if (!allowedWindow.current.has(candidate.entityId)) return
      fitCache.current.set(candidate.entityId, representation)
      onDimensions?.(candidate.entityId, representation.width, representation.height)
      refresh((value) => value + 1)
    },
    () => {
      pendingFit.current.delete(candidate.entityId)
      if (candidate.entityId === file.entityId) setError('无法预览该图片。')
    },
  )
}
```

Render `UnsupportedFileState` in the stage when the current file is unsupported,
and disable fit, 100%, zoom, and rotate controls for that file. Include
`unavailableEntityIds` in the prefetch effect dependencies. The original
representation effect starts with:

```ts
if (
  unavailableEntityIds.has(file.entityId) ||
  !isPreviewableImage(file) ||
  mode !== 'original' ||
  original !== null
) {
  return
}
```

Keep previous and next navigation enabled.

Add `unavailableEntityIds?: ReadonlySet<string>` with an empty-set default to
`ImagePreview`. Treat `unavailableEntityIds.has(file.entityId)` as a
higher-priority request-free stage state than supported/unsupported kind, clear
any already cached current representation from the visible stage, disable the
same transform controls, and preserve navigation.

In `ComparePane`, return the normal header/footer plus:

```tsx
<div ref={stageRef} className="compare-pane-stage">
  <UnsupportedFileState file={file} />
</div>
```

when the file is unsupported. Its effects must return before calling
`requestImage`. In `CompareWorkspace`, disable transform buttons when the active
file is unsupported. Change compare policy to:

```ts
export function isSupportedCompareKind(kind: string): boolean {
  return kind === 'jpeg' || kind === 'png' || kind === 'unsupported_image'
}
```

Use the copy `请选择 2–20 张图片进行对比。`. Add
`filePreviewExtensions.css` and import it after the base preview styles.

- [ ] **Step 4: Run image, compare, filmstrip, and full UI type checks**

Run:

```bash
pnpm --dir ui test -- \
  src/components/ContentBrowser.test.tsx \
  src/components/ImagePreview.test.tsx \
  src/components/ComparePane.test.tsx \
  src/components/CompareWorkspace.test.tsx \
  src/components/FolderFilmstripRow.test.tsx \
  src/state/comparePolicy.test.ts \
  src/state/compareModel.test.ts
pnpm --dir ui check
```

Expected: all pass and every unsupported-image test reports zero request calls.

- [ ] **Step 5: Commit unsupported-image surfaces**

```bash
git add ui/src/components/contentBrowser/ImageCell.tsx \
  ui/src/components/ContentBrowser.test.tsx \
  ui/src/components/ImagePreview.tsx \
  ui/src/components/ImagePreview.test.tsx \
  ui/src/state/comparePolicy.ts \
  ui/src/state/comparePolicy.test.ts \
  ui/src/state/compareModel.test.ts \
  ui/src/components/ComparePane.tsx \
  ui/src/components/ComparePane.test.tsx \
  ui/src/components/CompareWorkspace.tsx \
  ui/src/components/CompareWorkspace.test.tsx \
  ui/src/components/FolderFilmstripRow.tsx \
  ui/src/components/FolderFilmstripRow.test.tsx \
  ui/src/styles/filePreviewExtensions.css \
  ui/src/main.tsx
git diff --cached --name-only
git commit -m "feat: show unsupported image placeholders"
```

---

### Task 6: Decompose Text Preview Into Independent One-Or-Two Pane UI

**Files:**
- Create: `ui/src/components/TextPreviewPane.tsx`
- Modify: `ui/src/components/TextPreview.tsx`
- Modify: `ui/src/components/TextPreview.test.tsx`
- Modify: `ui/src/styles/filePreviewExtensions.css`
- Modify: `ui/src/App.tsx`
- Reference without editing: `ui/src/components/ModalSheet.tsx`

**Interfaces:**
- Produces: `TextPreviewFiles = readonly [BrowserFile] | readonly [BrowserFile, BrowserFile]`.
- Produces: `TextPreview({ files, unavailableEntityIds, requestPreview, openExternalLink, onClose, onTaskChange })`.
- Produces: `TextPreviewPane` with pane-local request, encoding, content, error, truncation, scroll, and Markdown links.
- Preserves: single-file double-click behavior by passing `[activePreviewFile]`.

- [ ] **Step 1: Add failing single/split independence tests**

Change the component harness to pass `files`. Add a split test:

```tsx
render(
  <TextPreview
    files={[plainTextFile, markdownFile]}
    requestPreview={requestPreview}
    openExternalLink={openExternalLink}
    onClose={onClose}
    onTaskChange={onTaskChange}
  />,
)

expect(screen.getByRole('dialog', { name: /plain\.txt.*notes\.md/ })).toBeInTheDocument()
await waitFor(() => expect(requestPreview).toHaveBeenCalledTimes(2))
expect(screen.getByRole('combobox', { name: 'plain.txt 文本编码' })).toBeVisible()
expect(screen.getByRole('combobox', { name: 'notes.md 文本编码' })).toBeVisible()
expect(screen.getByTestId(`text-pane-${plainTextFile.entityId}`)).toHaveTextContent('plain body')
expect(screen.getByTestId(`text-pane-${markdownFile.entityId}`)).toHaveTextContent('Markdown title')
```

Add one rejection and verify the other pane remains visible. Change only the
`notes.md 文本编码` selector and assert only the second file is requested again.
Require aggregate task feedback:

```ts
expect(onTaskChange).toHaveBeenLastCalledWith(expect.objectContaining({
  requested: 2,
  completed: 1,
  failed: 1,
  status: 'failed',
}))
```

Add an accessibility regression that focuses a launcher before opening the
dialog, verifies `aria-modal="true"`, wraps Tab from the last enabled control to
the first and Shift+Tab in the opposite direction, closes with Escape, and
asserts focus returns to the launcher.

Add a stale-response regression using two deferred promises: unmount the split
dialog before resolving them, then assert neither pane publishes content or
new task feedback. Rerender a pane with a different `entityId`, resolve the old
request after the new request, and assert the old response cannot replace the
new file's content.

Rerender with only the second entity in `unavailableEntityIds`; assert the left
pane keeps its decoded content, the right pane changes to `文件已不可用`, and no
replacement text request is started for the removed file.

- [ ] **Step 2: Run text preview tests and verify failure**

Run:

```bash
pnpm --dir ui test -- src/components/TextPreview.test.tsx
```

Expected: `TextPreview` accepts only `file`, renders one selector, and cannot
isolate two request states.

- [ ] **Step 3: Extract pane state and build the split shell**

Define:

```ts
export type TextPreviewFiles =
  | readonly [BrowserFile]
  | readonly [BrowserFile, BrowserFile]

export type TextPaneStatus = 'running' | 'complete' | 'failed'

export interface TextPreviewProps {
  files: TextPreviewFiles
  unavailableEntityIds?: ReadonlySet<string>
  requestPreview(file: BrowserFile, encoding?: TextEncoding): Promise<TextPreviewDto>
  openExternalLink(url: string): Promise<void>
  onClose(): void
  onTaskChange?(task: TaskFeedback | null): void
}

export interface TextPreviewPaneProps {
  file: BrowserFile
  unavailable: boolean
  requestPreview(file: BrowserFile, encoding?: TextEncoding): Promise<TextPreviewDto>
  openExternalLink(url: string): Promise<void>
  onStatusChange(entityId: string, status: TextPaneStatus): void
}
```

Move all per-file state and `load(encoding)` behavior into `TextPreviewPane`.
Give each pane a heading with an ID derived from `entityId`, render the pane as
a keyboard-scrollable `region` associated with that heading, and label its
selector `${file.name} 文本编码`. Guard asynchronous results with a
monotonically increasing request-generation ref:

```ts
const requestGeneration = useRef(0)

async function load(encoding?: TextEncoding) {
  const generation = ++requestGeneration.current
  onStatusChange(file.entityId, 'running')
  try {
    const next = await requestPreview(file, encoding)
    if (generation !== requestGeneration.current) return
    setPreview(next)
    onStatusChange(file.entityId, 'complete')
  } catch (error) {
    if (generation !== requestGeneration.current) return
    setError(safeUserMessage(error))
    onStatusChange(file.entityId, 'failed')
  }
}

useEffect(() => () => {
  requestGeneration.current += 1
}, [])
```

Reset pane state and increment the generation when
`file.entityId`/`file.modifiedNs` changes, so closed, replaced, and out-of-order
requests cannot publish stale state. When `unavailable` becomes true, increment
the generation, render `文件已不可用`, report the pane as failed, and do not
start another `requestPreview`.

Define one module-level `EMPTY_ENTITY_IDS: ReadonlySet<string> = new Set()` and
use it as the default for the optional `unavailableEntityIds` prop. The parent
dialog renders:

```tsx
<div className="text-preview-panes" data-pane-count={files.length}>
  {files.map((file) => (
    <TextPreviewPane
      key={`${file.entityId}:${file.modifiedNs}`}
      file={file}
      unavailable={unavailableEntityIds.has(file.entityId)}
      requestPreview={requestPreview}
      openExternalLink={openExternalLink}
      onStatusChange={recordStatus}
    />
  ))}
</div>
```

Derive task feedback from a status map:

```ts
const statuses = files.map((file) => paneStatuses[file.entityId] ?? 'running')
const completed = statuses.filter((status) => status === 'complete').length
const failed = statuses.filter((status) => status === 'failed').length
const settled = completed + failed
onTaskChange?.({
  id: `text-preview-${files.map((file) => file.entityId).join('-')}`,
  label: '读取文本预览',
  status: settled < files.length ? 'running' : failed > 0 ? 'failed' : 'complete',
  requested: files.length,
  completed,
  failed,
  cancellable: false,
  failures: [],
})
```

In the parent shell, keep one `dialogRef`, set `aria-modal="true"`, and reuse the
same focus-boundary algorithm already implemented in `ModalSheet.tsx`: capture
`document.activeElement`, focus the first enabled
`button/input/select/textarea/link/[tabindex]`, wrap Tab and Shift+Tab at the
first and last controls, close on Escape, and restore the captured element on
unmount. The status effect cleanup must call `onTaskChange?.(null)` so closing
either a one- or two-pane preview removes stale task feedback.

Add equal-width grid columns and independent pane overflow to
`filePreviewExtensions.css`. Update the current App single-text route to pass
`files={[activePreviewFile]}`.

- [ ] **Step 4: Run text preview, App smoke, and accessibility checks**

Run:

```bash
pnpm --dir ui test -- src/components/TextPreview.test.tsx src/App.test.tsx
pnpm --dir ui check
```

Expected: single preview remains unchanged; two panes load concurrently,
independently scroll, change encoding, and report errors.

- [ ] **Step 5: Commit the split-capable text component**

```bash
git add ui/src/components/TextPreviewPane.tsx \
  ui/src/components/TextPreview.tsx \
  ui/src/components/TextPreview.test.tsx \
  ui/src/styles/filePreviewExtensions.css \
  ui/src/App.tsx
git diff --cached --name-only
git commit -m "feat: support one or two text preview panes"
```

---

### Task 7: Validate Radial Preview Selection And Route Two Text Files

**Files:**
- Create: `ui/src/state/previewPolicy.ts`
- Create: `ui/src/state/previewPolicy.test.ts`
- Modify: `ui/src/components/radialMenuModel.ts`
- Modify: `ui/src/components/radialMenuModel.test.ts`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`

**Interfaces:**
- Consumes: Task 4 kind predicates and Task 6 `TextPreviewFiles`.
- Produces: `validatePreviewSelection(files) -> PreviewSelectionResult`.
- Produces: preview modes `single` and `split_text`.
- Produces: radial context fields `previewEnabled` and `previewDisabledReason`.
- Preserves: double-click always calls the existing single-file `openPreview`.

- [ ] **Step 1: Write failing pure policy, radial model, and App routing tests**

Use the pure policy matrix:

```ts
const file = (kind: BrowserFile['kind']) => ({ kind })

expect(validatePreviewSelection([file('jpeg')])).toEqual({
  ok: true,
  mode: 'single',
})
expect(validatePreviewSelection([file('other')])).toEqual({
  ok: true,
  mode: 'single',
})
expect(validatePreviewSelection([
  file('text'),
  file('markdown'),
])).toEqual({ ok: true, mode: 'split_text' })
expect(validatePreviewSelection([
  file('text'),
  file('other'),
])).toEqual({
  ok: false,
  reason: '仅支持单文件预览，或同时预览 2 个文本文件',
})
expect(validatePreviewSelection([
  file('text'),
  file('text'),
  file('text'),
])).toEqual({
  ok: false,
  reason: '文本预览最多支持 2 个可预览文件',
})
expect(validatePreviewSelection([
  file('jpeg'),
  file('text'),
  file('other'),
])).toEqual({
  ok: false,
  reason: '仅支持单文件预览，或同时预览 2 个文本文件',
})
```

Require `buildRadialMenuModel` to use the supplied enabled state and reason.
In `App.test.tsx`, open a radial session with two text files, activate `预览`,
and assert both filenames appear in the dialog and `previewText` is called for
both entity IDs. Also assert double-clicking either selected row opens only that
file. Dispatch a workspace refresh that removes only the second split member;
assert the dialog stays open, only that pane says `文件已不可用`, and the
context-repair banner remains visible.

- [ ] **Step 2: Run preview policy and App tests and verify failure**

Run:

```bash
pnpm --dir ui test -- \
  src/state/previewPolicy.test.ts \
  src/components/radialMenuModel.test.ts \
  src/App.test.tsx
```

Expected: the policy module is missing, radial preview still requires exactly
one file, and App cannot open a two-file preview session.

- [ ] **Step 3: Implement pure validation and App session routing**

Define:

```ts
export type PreviewSelectionResult =
  | { ok: true; mode: 'single' | 'split_text' }
  | { ok: false; reason: string }

export function validatePreviewSelection(
  files: readonly Pick<BrowserFile, 'kind'>[],
): PreviewSelectionResult {
  if (files.length === 1) return { ok: true, mode: 'single' }
  if (files.length === 2 && files.every(isPreviewableText)) {
    return { ok: true, mode: 'split_text' }
  }
  if (files.length > 2 && files.every(isPreviewableText)) {
    return { ok: false, reason: '文本预览最多支持 2 个可预览文件' }
  }
  return {
    ok: false,
    reason: '仅支持单文件预览，或同时预览 2 个文本文件',
  }
}
```

Compute the result once for the active radial files and pass:

```ts
previewEnabled: previewValidation.ok,
previewDisabledReason: previewValidation.ok ? undefined : previewValidation.reason,
```

to `buildRadialMenuModel`. For the action:

```ts
if (action === 'preview') {
  const validation = validatePreviewSelection(files)
  if (!validation.ok) return
  if (validation.mode === 'single') {
    openPreview(defined(files[0], 'Single preview requires one file'))
  } else {
    const first = defined(files[0], 'Split text preview requires a left file')
    const second = defined(files[1], 'Split text preview requires a right file')
    openPreviewSession({
      file: first,
      files: [first, second],
      folderOverviewIdentity: null,
    })
    setPreviewEntityId(first.entityId)
  }
}
```

When rendering previews, do not reuse the content workspace's image fallback
for text. Derive the exact current session members first:

```ts
const sessionPreviewFiles =
  activePreview === null
    ? []
    : activePreview.files ?? [activePreview.file]

const activeTextPreviewFiles: TextPreviewFiles | null =
  sessionPreviewFiles.length === 1 &&
  isPreviewableText(defined(sessionPreviewFiles[0], 'Missing single preview file'))
    ? [defined(sessionPreviewFiles[0], 'Missing single preview file')]
    : sessionPreviewFiles.length === 2 &&
        sessionPreviewFiles.every(isPreviewableText)
      ? [
          defined(sessionPreviewFiles[0], 'Missing left text preview file'),
          defined(sessionPreviewFiles[1], 'Missing right text preview file'),
        ]
      : null
```

Derive watcher removals without closing the complete preview:

```ts
const unavailablePreviewEntityIds = useMemo(
  () => new Set(state.contextRepair?.removedEntityIds ?? []),
  [state.contextRepair?.removedEntityIds],
)
```

Replace the current effect that closes when
`removedEntityIds.includes(activePreview.file.entityId)`. Pass
`unavailablePreviewEntityIds` to `TextPreview` and `ImagePreview`, and pass
`unavailable={unavailablePreviewEntityIds.has(activePreviewFile.entityId)}` to
`UnsupportedFilePreview`. Folder/project identity changes still close the
complete preview through their existing effect. A watcher removal therefore
changes only the affected pane or placeholder to `文件已不可用` and leaves the
context-repair banner available.

Render `TextPreview` only when `activeTextPreviewFiles` is non-null. Generic
`other` files render `UnsupportedFilePreview`; supported and unsupported images
render `ImagePreview`. This preserves the existing workspace-image fallback
only for single-image previous/next navigation.

The radial model's preview item becomes:

```ts
{
  id: 'preview',
  label: '预览',
  symbol: '◉',
  disabled: !context.previewEnabled,
  disabledReason: context.previewDisabledReason,
}
```

- [ ] **Step 4: Run preview policy, radial, App, and full UI tests**

Run:

```bash
pnpm --dir ui test -- \
  src/state/previewPolicy.test.ts \
  src/components/radialMenuModel.test.ts \
  src/components/RadialFileMenu.test.tsx \
  src/components/TextPreview.test.tsx \
  src/App.test.tsx
pnpm --dir ui test
pnpm --dir ui check
```

Expected: all pass; one file of any kind is previewable, exactly two text files
open split view, and invalid multi-selections show the correct disabled reason.

- [ ] **Step 5: Commit radial preview validation and routing**

```bash
git add ui/src/state/previewPolicy.ts \
  ui/src/state/previewPolicy.test.ts \
  ui/src/components/radialMenuModel.ts \
  ui/src/components/radialMenuModel.test.ts \
  ui/src/App.tsx \
  ui/src/App.test.tsx
git diff --cached --name-only
git commit -m "feat: open two selected text files in split preview"
```

---

### Task 8: Contract Documentation, Regression Gates, And Latest-Source Launch

**Files:**
- Modify: `docs/architecture/viewer-0.1-api-baseline.md`
- Do not modify: the protected dirty files listed under Global Constraints.

**Interfaces:**
- Consumes: Tasks 1–7.
- Produces: a documented `images + otherFiles` IPC contract and verified latest development executable.
- Preserves: current branch/worktree isolation and no packaged installation.

- [ ] **Step 1: Add the final contract assertions before documentation edits**

Extend the IPC baseline to show:

```json
{
  "workspace": "content",
  "images": [
    { "kind": "jpeg" },
    { "kind": "unsupported_image" }
  ],
  "otherFiles": [
    { "kind": "markdown" },
    { "kind": "other" }
  ]
}
```

In the same architecture document, state the user-visible format policy
explicitly: JPEG/JPG and PNG have image preview; TXT, Markdown, and MD have
bounded text preview; the approved unsupported-image registry appears in the
image collection with placeholders; the approved video registry is ignored;
all other visible regular files appear under `其它文件`.

Run repository-wide stale vocabulary searches:

```bash
rg -n "textFiles|text_files|textCount|text_count|文本文件|个文本" \
  crates src-tauri tests ui/src scripts docs/architecture/viewer-0.1-api-baseline.md
```

Every remaining match must be either a deliberate text-preview/index concept
or a protected pre-existing dirty file. Workspace collection and user-facing
secondary-panel matches are failures.

- [ ] **Step 2: Run format, type, unit, integration, and production-build gates**

Run:

```bash
pnpm quality
pnpm gate:m1
pnpm gate:m2
pnpm gate:m3
```

Expected: policy tests, Biome, TypeScript, all UI tests, UI production build,
Rust formatting, clippy with warnings denied, all Rust workspace tests, and
milestone gates pass.

- [ ] **Step 3: Run classification and preview-specific regression probes**

Run:

```bash
cargo test --locked -p viewer-infrastructure --test progressive_scan
cargo test --locked -p viewer-infrastructure --test m2_browse_projections
cargo test --locked -p viewer-desktop --test m1_desktop_runtime
pnpm --dir ui test -- \
  src/components/ContentBrowser.test.tsx \
  src/components/ImagePreview.test.tsx \
  src/components/CompareWorkspace.test.tsx \
  src/components/TextPreview.test.tsx \
  src/state/previewPolicy.test.ts
```

Expected: all pass with zero ignored failures. Record exact test totals in the
implementation handoff.

- [ ] **Step 4: Commit the clean documentation and final compile fixes**

Stage only clean files changed specifically for this feature:

```bash
git add docs/architecture/viewer-0.1-api-baseline.md
git diff --cached --name-only
git commit -m "docs: document other files and split text preview"
```

All compiler-driven source fixes belong in Tasks 1–7. Do not bundle source
changes into this documentation commit.

- [ ] **Step 5: Launch exactly one latest development Viewer for visual acceptance**

First confirm no packaged or stale process is running, then use the repository
launcher:

```bash
pnpm start:viewer
```

Verify the launched executable path is:

```text
/Users/abc/Project/Viewer/.worktrees/adaptive-text-panel/target/debug/viewer-desktop
```

Visually inspect:

- image-only folders contain no `其它文件` shelf;
- mixed folders show the adaptive shelf and all three select-all choices;
- a short expanded other-file list uses only its intrinsic height, while a long
  list stops at 20% of the content body and scrolls internally;
- an other-only folder gives the list the complete available content body;
- unknown and extensionless files appear under `其它文件`;
- unsupported images show placeholders in the grid, single preview, and
  multi-image comparison;
- one text file opens single preview;
- two selected text files open equal-width light panes with independent
  scrolling and encoding controls;
- more than two selected text files disable preview with the approved reason;
- removing one member of an open split preview marks only that pane unavailable;
- no second Viewer process or packaged application appears.

Do not install or copy an application bundle during this step.
