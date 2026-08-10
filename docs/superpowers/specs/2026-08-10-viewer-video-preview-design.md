# Viewer Bundled Video Preview Design

Date: 2026-08-10
Status: Approved

## Summary

Viewer will recognize local video files, show them in a dedicated collapsible
video section, and open them in a native, high-performance preview experience.
The complete playback engine is bundled with Viewer: users are not required to
install mpv, VLC, Homebrew packages, codecs, or another media player.

The selected engine is libmpv built in its LGPL configuration. The first
implementation is complete for macOS, while all application-facing contracts
remain platform-neutral so a Windows adapter can be added without replacing
the React player or application services.

The design was developed and approved section-by-section with the user. It
covers product behavior, engine boundaries, performance, thumbnail caching,
distribution, licensing, security, settings, failure handling, and acceptance
criteria. This document authorizes planning, not implementation; implementation
begins only after a separate plan is reviewed.

## Context

Viewer 0.1.5 classifies common video extensions as ignored or generic other
files. They do not enter the supported browse/index/preview path. The existing
desktop application uses React inside Tauri 2, a Rust domain/application/
infrastructure split, and a macOS platform crate for native behavior.

Video support must satisfy three constraints that rule out a WebView-only
`<video>` implementation:

1. the decode engine and codecs must travel with Viewer;
2. format coverage must not depend on the user's macOS installation or WebView
   codec set;
3. the selected engine and application boundary must support a future Windows
   build.

The experience is intended for inspecting local visual assets, not for
streaming, library consumption, or entertainment playback. Frame stepping,
timeline inspection, fast local startup, predictable resource release, and
thumbnail quality therefore take priority over playlist, subtitle, and online
media features.

## Confirmed Product Decisions

- macOS receives the complete first implementation; Windows is architected but
  not implemented in this delivery.
- Videos appear in a dedicated, collapsible section rather than mixing with the
  image grid or `Other Files`.
- The video section is expanded by default when videos exist and is omitted
  entirely when the folder has none.
- Video cards use an automatically selected and cached representative frame.
- Cards show a play affordance and duration.
- Double-clicking a video opens the dedicated preview and starts playback
  automatically at the file's first frame and original volume.
- Playback ends on the last frame; it does not loop and does not advance.
- Previous/next navigation includes only the current video list.
- Controls fade after approximately 2.5 seconds while playing and remain shown
  while paused, seeking, changing a value, or reporting an error.
- The control set includes play/pause, timeline, elapsed/total time, volume,
  mute, fullscreen, speed, frame step, and frame back-step.
- Reopening a video always starts at the beginning; playback position is not
  persisted.
- Embedded subtitle selection, external subtitles, and multi-audio-track
  selection are out of scope for the first release. The first playable audio
  track is used.
- Timeline hover and drag display a corresponding frame preview and time.
- Representative frame selection samples the early portion of the video and
  skips black or invalid frames rather than using a fixed timestamp.
- Keyboard shortcuts are Space for play/pause, Left/Right for previous/next
  frame, M for mute, F for fullscreen, and Escape for fullscreen exit followed
  by preview exit. Pressing Left/Right during playback first pauses the video.

## Goals

- Preview common local video containers with the bundled engine on a clean Mac.
- Use hardware video decoding and direct GPU-backed rendering where supported.
- Keep decoded frames out of JavaScript, React, and Tauri IPC.
- Make the first visible frame appear at its final fitted geometry without a
  poster-to-video size jump or black flash.
- Support forward and backward logical frame inspection.
- Generate useful cover and timeline thumbnails without turning folder scanning
  into sustained decode work.
- Release decoder, GPU, audio, thread, and file resources deterministically.
- Keep engine-specific types below a platform-neutral `VideoEngine` boundary.
- Bundle and document third-party components in a license-compliant way.
- Preserve current image, text, comparison, and organization behavior.

## Non-goals

- Implementing the Windows player in this delivery.
- URL playback, livestreams, HLS/DASH browsing, online extraction, or downloads.
- Playlists, automatic next-video playback, shuffle, or repeat.
- External subtitle discovery, embedded subtitle selection, or audio-track
  selection.
- Editing, trimming, transcoding, exporting frames, or saving playback state.
- Picture-in-picture, AirPlay, casting, or background playback.
- Image-preview zoom, pan, rotation, or magnifier tools for videos.
- Guaranteeing real-time playback for every possible 8K, high-bit-rate,
  professional, or malformed codec profile on every Mac.
- Static linkage of the engine into the Viewer executable.

## Engine Evaluation

### libmpv Render API — selected

mpv provides broad container and codec support through FFmpeg and exposes
commands/properties appropriate for a custom player, including frame stepping,
frame back-step, seeking, playback rate, screenshots, and observable state. Its
official embedding examples recommend the Render API over raw window embedding
for platform-sensitive macOS integrations. mpv supports macOS and Windows and
has platform hardware-decode paths such as VideoToolbox and D3D11VA.

mpv is GPL by default. The official client API documentation states that a
`-Dgpl=false` build makes the core LGPL 2.1-or-later. Viewer must use that
configuration and audit every enabled dependency; selecting libmpv does not
permit an unreviewed default mpv binary.

### libVLC — not selected

libVLC is an established LGPL embeddable engine with native macOS and Windows
drawable support. It is a strong choice for ordinary playback, but the approved
reverse-frame and frequent timeline-thumbnail requirements require more custom
seek/snapshot behavior than libmpv's command model. Its plugin distribution is
also typically broader than Viewer needs.

### GStreamer — not selected

GStreamer is cross-platform and has mature Rust bindings, but the pipeline,
plugin, codec, packaging, and license matrix would make the first implementation
substantially more complex. Viewer does not currently need its general-purpose
media pipeline flexibility.

References:

- [mpv repository](https://github.com/mpv-player/mpv)
- [libmpv embedding examples](https://github.com/mpv-player/mpv-examples/blob/master/libmpv/README.md)
- [libmpv client API licensing note](https://github.com/mpv-player/mpv/blob/master/include/mpv/client.h)
- [mpv hardware decode options](https://github.com/mpv-player/mpv/blob/master/DOCS/man/options.rst)
- [VLC/libVLC repository](https://github.com/videolan/vlc)
- [GStreamer repository](https://github.com/GStreamer/gstreamer)

## Architecture

```text
React UI
  VideoSection / VideoCard / VideoPreview / VideoControls
                ↕ typed commands and throttled events
Tauri video bridge
                ↕
VideoPreviewService + VideoThumbnailService
                ↕ platform-neutral VideoEngine port
macOSLibmpvAdapter (now)       WindowsLibmpvAdapter (future)
                ↕
Bundled LGPL libmpv runtime and reviewed dynamic dependencies
```

### Frontend

React owns layout, buttons, accessibility, shortcuts, loading/error
presentation, navigation, and the visible state model. React never receives
decoded frame buffers and does not draw video to a canvas.

Playback time is published to React at a bounded cadence, initially 10 Hz,
rather than once per decoded frame. Discrete state changes such as first frame,
pause, end, mute, fullscreen, and errors are delivered immediately.

### Tauri bridge

The bridge exposes typed commands for opening, closing, playback, seeking,
frame movement, volume, rate, fullscreen, and thumbnail requests. It publishes
typed events for state, bounded progress, media properties, first-frame
readiness, thumbnail readiness, and errors.

The bridge contains no playback policy. It validates entity/session identity,
rejects stale commands, and forwards valid work to the application service.

### Application services

`VideoPreviewService` owns the active playback session and navigation policy.
It serializes state-changing commands, cancels stale seeks and thumbnail
requests, and makes resource teardown idempotent.

`VideoThumbnailService` owns cover selection and timeline-thumbnail scheduling.
It is independent from the active playback instance and must not lower playback
quality or keep the app busy after its request becomes irrelevant.

### Platform port and adapters

The `VideoEngine` port expresses Viewer capabilities, not raw mpv commands.
Engine-specific handles, properties, errors, render contexts, and callbacks
remain inside `viewer-platform-macos`.

The macOS adapter owns the GPU-backed render surface, libmpv handle, render
context, event loop, audio output, and deterministic cleanup. A future Windows
adapter implements the same port with Windows rendering and hardware decode.
Common crates and UI code may not import AppKit, Objective-C, Win32, or libmpv
FFI types.

## Mandatory Rendering Feasibility Gate

Before schema and product implementation expands, a bounded technical slice
must prove that the selected libmpv Render API can coexist with the current
Tauri/WKWebView window model. The slice must demonstrate:

- a GPU-backed video surface at a DOM-coordinated preview rectangle;
- correct resizing and Retina scale handling;
- React controls composited above or around the surface without intercepting
  native playback input unexpectedly;
- first-frame readiness, pause, frame step, frame back-step, and close;
- VideoToolbox activation on a supported H.264 and HEVC sample;
- no CPU frame copy into JavaScript;
- safe window close and repeated mount/unmount.

If the Render API cannot meet these constraints, implementation pauses and the
design is revisited. The team must not silently replace it with frame copies,
an external process, a WebView codec, or a different engine.

## Performance Model

The intended hot path is:

```text
local file → libmpv demux/decode → VideoToolbox when supported
           → GPU-backed native render surface → macOS window compositor
```

Hardware decoding is explicitly requested with the safe automatic mode. A
format/profile not supported by the hardware may fall back to a copy path or
software decode. The UI reports a playback error only if both normal and
fallback decoding fail.

Performance rules:

- only one primary playback decoder is active;
- decoded frames never cross Tauri IPC;
- React progress updates are bounded and coalesced;
- continuous render callbacks do not call UI state setters directly;
- thumbnail work uses a separate low-priority worker with default concurrency
  one;
- nonessential thumbnail work pauses during active playback;
- stale seek/thumbnail work is canceled or ignored by session generation;
- closing preview immediately stops video, audio, render callbacks, and file
  access;
- engine and UI logs avoid high-frequency per-frame output in release builds.

Zero-copy decode/render is preferred but cannot be promised for every codec,
pixel format, color profile, or driver. Tests record which path mpv selected so
performance regressions are diagnosable.

## File Discovery And Metadata

### Candidate containers

The initial extension allowlist includes:

- MP4, M4V, MOV;
- MKV, WebM;
- AVI, WMV;
- MPG, MPEG;
- TS, MTS, M2TS;
- FLV, OGV, 3GP.

The allowlist keeps directory scanning cheap; an extension is only a candidate,
not proof that a file is playable. The worker probes each candidate with the
bundled engine and records success or a stable failure category. Future engine
coverage can extend the allowlist without changing UI architecture.

### Domain model

`FileKind::Video` is added with a new stable persistence value. Existing enum
encodings are not renumbered. `VideoMetadata` contains at least:

- duration in microseconds;
- encoded/display width and height;
- rotation/orientation;
- nominal or average frame rate when known;
- video and first audio codec identifiers;
- probe status and normalized failure category.

Codec strings are diagnostic metadata, not a promise that every frame is
decodable. Missing duration or frame rate is valid for unusual local files and
must not crash the card or player.

### Probe scheduling

Directory enumeration publishes candidates without opening a decoder. Metadata
probe work runs after the browse result is usable, is bounded, cancelable, and
does not delay images. Failed and corrupt candidates remain visible as video
cards with an unavailable state rather than silently returning to Other Files.

## Representative Frames And Timeline Thumbnails

### Cover selection

The cover worker samples several early positions, beginning after any immediate
container lead-in. It rejects decode failures and frames below conservative
luminance/variance thresholds, then accepts the first useful frame. If no
sample passes, it uses the first decodable frame. If nothing decodes, the card
uses a stable unavailable-video presentation.

The algorithm is deterministic for the same file and algorithm version. It is
not a semantic AI scene selector and does not scan the whole video.

### Timeline preview

Timeline frames are generated only after a video preview is open and the user
first interacts with the timeline. Requests are quantized to reusable time
buckets. Pointer movement debounces work, and results carry the session and
request generation so a late frame cannot appear for a different hover time or
video.

The first implementation may fill buckets incrementally rather than decoding a
complete contact sheet. Visible playback always outranks preview generation.

### Cache

The cache lives in Viewer-owned system cache storage and never writes beside
project files. A cache identity includes canonical source identity, size,
modification time, output kind, requested bucket, and algorithm/schema version.

The default video cache budget is 1 GiB with least-recently-used eviction. The
settings screen reports current cache use and offers a clear action. Clearing
the cache cancels or waits for active cache writes, removes only verified
Viewer video-cache targets, and does not interrupt the currently decoded frame.

Video cards reserve their final geometry before a cover exists. The cover fades
into the reserved card without changing layout.

## Browse Experience

- A `Video · N` disclosure appears below the image browser and above generic
  other files when the current folder has videos.
- It is expanded by default for a folder containing videos.
- It is absent when the folder contains none.
- Collapse/expand changes presentation only and does not restart probing,
  discard covers, or alter selection.
- Cards show the cover, central play indicator, duration, and filename using
  the existing content-card visual language.
- Unavailable videos show a stable unavailable mark and concise status.
- Video cards participate in search, selection, file operations, and metadata
  overlays according to the same safety rules as other regular files.
- The file-kind filter gains `Video`; videos no longer appear as generic Other
  Files.

## Preview Experience

### Entry and first reveal

Double-click opens the full preview shell immediately. The engine surface stays
hidden behind a neutral loading layer while libmpv opens the file paused,
discovers display geometry, and renders the first correct frame. The surface is
revealed once at its final fit-to-window rectangle, then playback starts.

The card cover is not enlarged as a temporary video image. The player therefore
cannot show a small poster and jump to a large native surface. Loading uses an
indeterminate progress treatment because local open/decode does not provide a
trustworthy percentage. It has no artificial minimum duration and respects
reduced-motion preferences.

### Sizing

Video preserves display aspect ratio, includes rotation metadata, fits inside
the available preview stage, and is never cropped. Video preview does not reuse
image zoom, rotation, or magnifier controls. Fullscreen removes surrounding
Viewer chrome while retaining the auto-hiding player controls.

### Controls

The control overlay contains:

- play/pause;
- previous-frame and next-frame controls;
- timeline with elapsed and total time;
- volume and mute;
- rate menu with 0.5×, 0.75×, 1×, 1.25×, 1.5×, and 2×;
- fullscreen.

The overlay appears on pointer movement or keyboard control. It fades after
approximately 2.5 seconds while playing. It stays visible while paused,
seeking, adjusting controls, focused by keyboard, or displaying an error.

Timeline hover/drag shows a cached or requested frame preview and timestamp
above the pointer. The preview clamps to the player edges. A pending preview
uses a stable placeholder and never shows a frame from a stale request.

### Navigation and completion

The bottom navigation pill shows current video index and total video count. It
only moves through the current video result list. Switching creates a new
session generation, cancels stale work, tears down the old media, and applies
the same first-frame gate to the new file.

Playback ends paused on the final frame. It does not loop or advance. Reopening
always starts from the beginning with 1× speed and 100% volume. No playback
history is stored.

### Keyboard

- Space toggles play/pause.
- Left and Right pause active playback, then move backward or forward one
  logical frame.
- M toggles mute.
- F enters or exits fullscreen.
- Escape exits fullscreen first; when not fullscreen it closes video preview.

Shortcuts are inactive while a text input or unrelated dialog owns keyboard
focus. Buttons expose accessible names and the shortcut in their help text.

## Playback State And Races

The UI/application state machine distinguishes at least:

```text
idle → preparing → ready/playing ↔ paused
                         ↕
                 seeking/frame-step
                         ↓
                     ended

any active state → failed
any active state → closing → idle
```

Each open creates a monotonically increasing session generation. Every command,
engine event, and asynchronous thumbnail result carries that generation.
Results from an old video are ignored after navigation, close, project change,
or app shutdown.

Close and failure cleanup are idempotent. An unavailable event order, duplicate
end event, late first frame, or canceled seek cannot resurrect a closed player.

## Commands And Events

Exact Rust and TypeScript names may follow repository conventions, but the
semantic contract is:

Commands:

- open video by validated entity identity;
- close active session;
- play, pause, mute, set volume, set rate;
- seek to bounded time;
- step logical frame backward/forward;
- enter/exit fullscreen;
- request timeline thumbnail for a bounded time bucket;
- query and clear video-cache usage.

Events:

- session/media prepared;
- first frame ready;
- state changed;
- coalesced time/duration update;
- volume/mute/rate changed;
- timeline thumbnail ready;
- playback ended;
- normalized error;
- session closed.

The bridge does not accept arbitrary mpv command strings or options from the
frontend.

## Packaging And License Compliance

Viewer builds libmpv from a pinned official source tag and commit checksum with
`-Dgpl=false`. Dependencies and FFmpeg features are explicit allowlists rather
than inherited host-machine defaults. GPL-only optional components are not
enabled in the distributed build.

macOS ships reviewed dynamic libraries inside the application bundle with
bundle-relative loader paths. Apple Silicon and Intel artifacts are built from
the same manifest and may be merged into the release shape supported by the
Viewer packaging pipeline. Nested binaries are signed and included in
notarization verification.

Viewer does not search Homebrew, system PATH, or arbitrary user directories for
libmpv. Development overrides, if any, are explicit and cannot be active in a
release build.

Distribution includes:

- LGPL license text and third-party notices;
- exact engine/dependency versions and enabled build options;
- reproducible build scripts and checksums;
- a documented source acquisition path for the corresponding sources.

The same manifest model reserves Windows DLL output and a Windows adapter, but
no Windows runtime is shipped in this macOS delivery. License review is a
release gate, not an assumption based solely on mpv's top-level license.

## Runtime Isolation And Security

- Viewer disables user mpv configuration, profiles, scripts, input bindings,
  and automatic script discovery.
- Online extraction, URL playback, and network protocols are not exposed.
- Only canonical local files already represented by Viewer entities may be
  opened.
- External subtitle and same-directory sidecar auto-loading are disabled.
- The frontend cannot pass arbitrary command lines, property names, protocols,
  or filesystem paths to libmpv.
- Path authorization is rechecked at the Rust boundary and bound to the active
  project/session generation.
- Engine logging is bounded and avoids leaking complete sensitive paths in
  user-facing errors.
- Cache deletion resolves and verifies the exact Viewer cache root before any
  material removal.
- Malformed media is treated as untrusted input. Probe/playback failures are
  contained and do not mutate the project or corrupt the search index.

libmpv is in-process, so a native decoder vulnerability can affect Viewer. The
pinned engine and FFmpeg dependencies therefore enter the repository's
dependency-health and upgrade process, including security advisory review.

## Persistence And Migration

- `FileKind::Video` receives a new encoded value; old values are never shifted.
- Video metadata uses an additive schema migration or focused companion table.
- Existing indexes remain readable and are incrementally enriched or safely
  rebuilt according to current generation rules.
- Thumbnail files are cache artifacts, not database truth.
- Playback position, rate, mute, and history are not persisted.
- The only new user-facing persisted preference is the ordinary section
  collapse state if Viewer already persists comparable disclosure state; the
  first encounter still defaults to expanded.

## Error Presentation

Normalized user-facing categories include:

- unsupported container or codec;
- damaged/unreadable media;
- permission or file-disappeared error;
- engine initialization failure;
- hardware decode failure with software fallback failure;
- render-surface failure;
- thumbnail unavailable while playback remains available.

Errors retain the preview shell and offer `Retry` and `Done`. Escape remains
available. Technical details may be copied or logged for diagnostics, but the
primary message remains concise. A cover or timeline-thumbnail failure never
makes an otherwise playable video unavailable.

## Accessibility And Motion

- Video cards and controls have keyboard focus states and accessible names.
- The timeline exposes its current value, minimum, maximum, and formatted time.
- State changes such as loading failure are announced without announcing every
  progress tick.
- Auto-hide does not dismiss controls while keyboard focus is within them.
- Reduced-motion mode removes loading sweeps and control fades while preserving
  visible state transitions.
- Contrast and target sizes follow the current Viewer acceptance baseline.

## Settings

The settings screen adds a Video Cache row containing current disk usage, the
1 GiB automatic limit, and a `Clear Video Cache` action. Hardware decoding is
automatic with fallback and is not exposed as a normal preference in the first
release. Diagnostic logs may report the selected decoder path.

## Test Strategy

### Domain and infrastructure

- extension classification and stable enum encoding;
- additive migration from a 0.1.5-era index;
- metadata with missing duration/frame rate/audio;
- cache identity, invalidation, LRU eviction, and safe clear targeting;
- black-frame rejection and deterministic cover fallback;
- cancellation and stale-generation rejection;
- corrupt and disappeared files.

### Application and bridge

- full state-machine transitions and idempotent close;
- automatic start, stop-on-final-frame, and video-only navigation;
- seek and thumbnail request coalescing;
- play-to-frame-step automatic pause;
- engine error normalization;
- no arbitrary path or mpv command crosses the bridge;
- mock adapter contract tests that can later be reused for Windows.

### React

- default-expanded and no-video-hidden sections;
- stable card geometry and cover fade-in;
- first-frame loading gate without poster/video size swap;
- controls, 2.5-second auto-hide, focus retention, and reduced motion;
- keyboard shortcuts and text-input suppression;
- timeline preview clamping, pending, ready, and stale states;
- video-only previous/next count;
- loading, retry, ended, and unavailable states;
- cache settings and clear confirmation behavior.

### Media fixture matrix

Repository test fixtures or generated artifacts cover at least:

- MP4 with H.264/AAC;
- MOV with HEVC;
- MOV with ProRes;
- WebM with VP9/Opus;
- MKV with AV1 when enabled in the reviewed build;
- rotated portrait video;
- variable-frame-rate video;
- video with no audio;
- truncated/corrupt video;
- candidate extension with an unsupported codec.

Fixtures remain small and redistribution-safe. Larger performance samples are
kept outside normal source history if repository policy requires it.

### Performance and lifecycle

- common 1080p60 and 4K30 fixtures play through the intended hardware path on
  supported reference Macs;
- post-warmup dropped-frame target is below 1% on the documented reference
  sample and machine;
- user command to engine-response target is below 100 ms under normal local
  playback;
- 30 open/close and rapid-navigation cycles show no monotonically increasing
  decoder instances, worker threads, render callbacks, or memory ownership;
- closing returns decoder/GPU/audio work to idle promptly;
- thumbnail generation concurrency remains one and yields to playback;
- the complete signed app plays offline on a clean Mac without mpv, VLC,
  Homebrew, or downloaded codecs.

Performance targets are evaluated with machine, codec, resolution, bit depth,
and hardware path recorded. Unsupported extreme media may fail gracefully
rather than meeting targets intended for common local assets.

## Expected Implementation Areas

Implementation planning should expect changes in:

- `viewer-domain` file kinds, metadata, and video state types;
- scan classification and search index encoding/migrations;
- application ports and video services;
- a focused libmpv FFI/runtime crate or platform module;
- `viewer-platform-macos` native surface and lifecycle integration;
- Tauri commands, events, runtime packaging, capabilities, and bundle settings;
- React API types/state, content browser, search filters, video components,
  settings, styles, acceptance scenes, and tests;
- CI/dependency health, third-party notices, build scripts, and release checks.

The implementation plan must map these boundaries to exact files after the
rendering feasibility gate establishes the final platform module shape.

## Delivery Sequence

1. Prove the macOS libmpv Render API slice and hardware decode path.
2. Add pinned reproducible LGPL engine build and packaging verification.
3. Add stable domain/index/schema support for videos.
4. Add metadata probe, cover generation, cache, and browse section.
5. Add the application session model and typed bridge.
6. Add first-frame-gated playback and controls.
7. Add timeline previews, settings, and error/accessibility states.
8. Run fixture, performance, lifecycle, clean-machine, signing, and license
   acceptance.

Each stage must keep the repository testable. The browse/index change must not
land in a state that exposes playable cards without an explicit unavailable or
feature-gated preview path.

## Acceptance Criteria

1. On a clean supported Mac, Viewer recognizes the approved containers and
   opens supported codecs without any separately installed player or codec.
2. Folders with videos show an expanded `Video · N` section with stable cached
   cover cards; folders without videos show no empty video section.
3. Double-click enters the preview shell immediately, then reveals the first
   native video frame once at its final fit-to-window geometry and starts
   playback without a small-image jump or black flash.
4. Video rendering does not send decoded frames through React or Tauri IPC and
   uses VideoToolbox when the sample and hardware support it.
5. The approved controls, 2.5-second behavior, shortcuts, frame movement,
   hover previews, fullscreen, end-on-last-frame, and video-only navigation
   work as specified.
6. Reopening starts from the beginning at 1× and 100% volume; no playback
   history, subtitle selection, track selection, looping, or auto-advance is
   present.
7. Representative frames avoid ordinary black intros, cache safely, invalidate
   after source changes, and never block directory enumeration.
8. Thumbnail work yields to playback, stale work cannot update another video,
   and cache usage remains within the automatic 1 GiB budget.
9. Unsupported, corrupt, missing, or fallback-failed media produces a contained
   retryable error and does not destabilize browsing or indexing.
10. Repeated playback/navigation/close tests show deterministic release without
    sustained background decode or monotonic resource growth.
11. The application bundle contains only the pinned reviewed engine/dependencies,
    correct signatures/loader paths, required notices, and reproducible build
    metadata.
12. Common code exposes a platform-neutral adapter contract suitable for a
    later Windows implementation and contains no leaked macOS/libmpv types.
