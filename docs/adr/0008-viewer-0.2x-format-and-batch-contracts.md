# ADR 0008: Viewer 0.2.x format expansion and batch-processing contracts

> Status: Draft (adopted incrementally per 0.2.x slice; finalizes at 0.2.5)
>
> Decision: Viewer 0.2.x extends the image pipeline to macOS Image I/O's native
> format matrix under a single decode gate, adds Markdown write-back and batch
> image operations as explicit IPC contracts, and records the dependency
> additions those slices require. Adopted 2026-10-25 with the 0.2.x development
> plan (`docs/milestones/viewer-0.2x-development-plan.md`).
>
> Date: 2026-10-25

## Context

As of v0.2.0 only JPEG and PNG decode fully; every other image extension
(GIF/TIFF/BMP/ICO/WebP/AVIF/HEIC/PSD/RAW/SVG …) classifies as
`unsupported_image` and never enters the decode pipeline. The 0.2.x cycle
(target requirements, 2026-10-25 revision) commits to three capability goals —
broad image-format compatibility, Markdown reading + editing, and batch image
processing — delivered as slices 0.2.1 through 0.2.5.

Code-level facts established during planning (see development plan §1):

- The pipeline has exactly one format gate:
  `image_format()` in `viewer-platform-macos/src/image/image_io.rs` maps
  CGImageSource UTIs to `ImageFormat` and rejects everything else with
  `ImageError::Unsupported`.
- Every representation (thumbnail / fit preview / original 100%) is re-encoded
  to PNG via `encode_png`; the `viewer-image://` protocol therefore only ever
  serves `image/png` artifacts, and its MIME allowlist
  (`image_protocol.rs`) stays locked at `image/jpeg | image/png`.
- `DecodeBudget(700_000_000 bytes, 100_000_000 px)` already guards full decodes.
- File operations already run through the `FileCommandService`
  preflight/write-lane/`OperationJournal` model (ADR 0002 semantics).

## Decision

- **Domain vocabulary.** `FileKind` gains a generic `Image` variant (stable
  encode value 8) for fully previewable images beyond JPEG/PNG. `.jfif`
  reclassifies to `Jpeg` (decision D1). `svg`/`svgz`/`jxl` stay
  `UnsupportedImage` for now; `svg`/`svgz` and `.ai` move to `Image` in 0.2.2
  (decision D4). `ImageFormat` extends with `Gif/Tiff/Webp/Avif/Heic/Bmp/Ico/
  Psd/Raw` so probes keep carrying the decoded source format.
- **Single decode gate, deny by default.** `image_format()` accepts an explicit
  UTI allowlist per new format plus `public.camera-raw-image` as the RAW
  fallback; unknown UTIs keep returning `Unsupported`. Both render paths
  inherit the gate: the web-artifact path through `ImageIoBackend`, and the
  native wgpu viewport through `probe_image_source` / `thumbnail_from_image_source`
  (each re-validates `image_format()`; audited 2026-10-25). Multi-page or
  animated sources decode frame 0 only; 0.2.x ships no animation playback.
- **Wire surface unchanged for images.** Artifacts remain PNG re-encodes; the
  `viewer-image://` MIME allowlist, its tests, and CSP do not change in 0.2.x.
  Reclassified files keep their marker data because entity identity is
  path-based, not kind-based.
- **Decode budget unchanged.** The existing byte and hard-pixel limits apply to
  every newly admitted format; the per-format verification matrix includes
  truncated files, mismatched extensions, and lying-dimension (decompression
  bomb) samples.
- **Markdown write-back (0.2.3).** One new IPC command `save_text`: validates
  session/generation, rejects read-only projects and non-indexed or symlinked
  targets, writes atomically via temp file + rename, and compares a blake3
  content fingerprint against load time to refuse blind overwrites with a
  `conflict` error variant. Write-back is UTF-8 only; saving a non-UTF-8
  source requires an explicit conversion acknowledgement (decision D3). Files
  beyond the 10 MiB preview cap stay read-only.
- **Batch image operations (0.2.4).** New plan/preflight/execute contracts
  mirror `FileCommandService` (single write lane, `BatchProgress` over
  `viewer://operation-progress`, cancellation, paged results). Batch
  conversion produces new files and is **not undoable** (existing
  "copies are not undone" convention). Overwrite mode is destructive and must
  go through staged copy + `OperationJournal` + Trash-before-Replace exactly
  as ADR 0002 prescribes.
- **Dependencies.** 0.2.2 adds `resvg` and `flate2` (workspace, pure Rust);
  0.2.3 adds CodeMirror 6 (`@codemirror/*`, `@uiw/react-codemirror`, MIT) to
  the UI. Each addition syncs `deny.toml`, `THIRD_PARTY_NOTICES.md`, and
  `docs/quality/dependency-exceptions.json` through the locked-dependency
  flow.

## Consequences

- `SUPPORTED_FORMATS.md` moves to tiered promises (full decode / RAW
  quick-preview tier / listed only), and the per-format matrix in
  `tests/image_format_matrix.rs` is the acceptance evidence.
- RAW formats decode at Image I/O "quick preview" quality (no camera profiles,
  weak noise reduction); professional RAW development stays out of scope.
- AVIF/WebP *encoding* availability varies by macOS version and is probed at
  runtime via `CGImageDestinationCopyTypeIdentifiers` (0.2.4); decoding follows
  the platform baseline (macOS 13).
- Each slice lands through `pnpm verify:clean` plus its slice gate; this ADR
  is finalized with the 0.2.5 closure (regression, perf re-baseline, docs
  sync).
