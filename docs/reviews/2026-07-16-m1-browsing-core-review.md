# M1 Browsing Core Stage Review

> **Historical governance note (2026-07-23):** References to M4 or Viewer 0.1 release acceptance below describe the plan at the time this evidence was recorded. ADR 0005 cancelled M4 and replaced release acceptance with continuous development governance. The measured evidence in this document is unchanged.

- Status: Passed
- Date: 2026-07-16
- Base: `8149d57` (accepted G4 on `main`)
- Reviewed implementation head: `3889db6`
- Review method: M1 plan traceability, full `8149d57...3889db6` diff inspection, aggregate gate, packaged-app acceptance on a disposable project, cache inspection and Apple Silicon artifact inspection
- Decision: **Approved for local fast-forward merge to `main`**

## Exit-criteria traceability

| M1 exit criterion | Evidence | Result |
| --- | --- | --- |
| Empty, single-project lifecycle | Picker opens one canonical directory; a second open is rejected; project close, window close and application exit share the backend cleanup path; Dock reopen was retested after an active-window close | Pass |
| Progressive folder-only hierarchy | Folder batches commit before safe progress publication; deep, empty and duplicate-name folders retain full relative paths; hidden, `.viewer`, link and alias entries are excluded; visible rows are virtualized | Pass |
| Folder and content browsing | Metadata-first descendant cards cover arbitrary folder depth; explicit aggregate mode is separate; image grid mounts only its visible window; Markdown/TXT remain an independent list | Pass |
| Native image preview | Quick Look with Image I/O fallback supplies session-scoped thumbnails and previews; the packaged app exercised PNG/JPG fit, 100%, free zoom, rotation, navigation, failure isolation, information display and focus/scroll restoration | Pass |
| Safe text preview | Reads are bounded to 10 MiB; UTF-8, UTF-16 LE/BE and GB18030 are supported; Markdown HTML is generated and sanitized in Rust; local images use opaque session URLs and external links require an explicit system handoff | Pass |
| Failure and task behavior | Scan, thumbnail and text tasks expose compact progress; corrupt content is isolated; stale generations and inactive sessions cannot publish or resolve image tokens; IPC errors contain no absolute/cache path | Pass |
| Disposable session data | Indexes and image representations live only below the owned application cache; close invalidates tokens, stops work and removes the session directory without touching the project | Pass |
| Reproducible release evidence | Exact M1 gate, dependency/license/security/scope checks, UI build, Rust tests, arm64 application build and DMG integrity all pass | Pass |

## Review findings and fixes

No Critical or Important findings remain open.

The packaged-app lifecycle check found one Important defect that component tests had missed. Closing an active macOS window removed the backend session cache but only hid the WebView, so reopening the existing process displayed its stale React project state. A failing UI test first captured the missing native-close signal. Commit `7e8f8c8` adds a narrow `viewer://project-closed` lifecycle event, resets every frontend projection/selection generation when it arrives, emits it after backend cleanup and again on Dock reopen, and preserves the minimal event-only Tauri capability. The rebuilt application was opened with a project, closed through the native window button and reopened; it returned to the empty import surface and the session cache remained empty.

The dependency review found an Important governance drift: M1 added `encoding_rs`, `pulldown-cmark` and the Tauri dialog packages without extending the frozen direct-dependency assertion. The M1 policy update now requires those exact entries and retains Apache-2.0 compatibility and third-party notice checks.

The interaction review found a coverage gap for the native project-drop bridge. Commit `3889db6` adds a positive controller regression proving that exactly one native path reaches the same guarded open state machine as the picker; the existing negative test rejects a dropped file. This does not add a second import path or any frontend filesystem permission.

The remaining diff review covered session/cache ownership, indexed-source identity revalidation, symlink and alias exclusion, opaque image protocol authority/session checks, Markdown sanitization, structured error disclosure, bounded preview work, virtual grid/tree windows, keyboard selection and stale asynchronous publication. No additional Critical or Important defect was found.

## Packaged-application acceptance

The disposable project contained 217 directories, 1,110 regular files and one symlink. It included three-level content, empty and duplicate-name folders, hidden and `.viewer` entries, unsupported files, lightweight JPG/PNG corpus entries, an intentionally corrupt JPEG, two valid color/alpha fixtures, Markdown, UTF-16 LE and GB18030 text. Viewer published 1,318 supported folder/file nodes while keeping ignored entries out of the UI.

Observed in the release application:

- a native folder picker opened the project into the folder-only tree and descendant overview;
- visible thumbnails were requested incrementally, 11 bad images in the selected fixture folder failed independently, and the two valid PNG/JPG files previewed correctly;
- fit/free zoom, rotation, previous/next navigation and Command-I information remained responsive and closing preview restored the selected grid item;
- Markdown, UTF-16 LE and GB18030 previews rendered readable content with the detected encoding shown;
- explicit project close returned to the empty surface;
- native window close removed the active session cache and reopening the same application process stayed empty.

Computer Use could not reliably synthesize a cross-application Finder drop even after placing Finder and Viewer side by side. The Tauri native-drop adapter, single-path controller path and file rejection are therefore covered by automated tests, while one physical Finder-to-Viewer drag remains an explicit M4 manual release check.

## Final verification evidence

```text
pnpm gate:m1                         PASS
  repository policy                 6/6
  UI                                10 files / 28 tests
  TypeScript + Vite production build PASS
  cargo fmt / strict Clippy          PASS
  locked Rust workspace tests        PASS
  advisories/bans/licenses/sources   PASS
  npm license policy                 119 packages / 11 reviewed expressions
  pnpm audit                         no known vulnerabilities
  Tauri security boundary            7/7
  M1 browse/text/runtime suites       5/5, 5/5, 8/8
  scope coverage                     47/47 requirements exactly once
  M1 IPC fixtures                    PASS
pnpm build:macos                     PASS; Viewer.app + Viewer_0.1.0_aarch64.dmg
file / lipo                          Mach-O 64-bit arm64 / arm64
otool LC_BUILD_VERSION               minos 13.0
hdiutil verify                       VALID
git diff --check                     PASS
```

## Accepted limitations and later ownership

- The disposable corpus proves bounded UI work and failure isolation, not final performance with the user's approximately 10 MiB production images. M4 repeats long-session browse/preview acceptance on the supplied real project folder; the user previously deferred a separate pressure corpus.
- Folder cards and the independent text list are metadata-light for the current expected scale; M4 standard-device evidence decides whether they also require windowing. The high-volume image surface and folder tree are already virtualized.
- A physical Finder-to-Viewer drag is retained as an M4 manual release check because the automation limitation above prevents trustworthy cross-application drag evidence.
- The internal application and DMG are unsigned and unnotarized because Developer ID credentials were not provided. M4 signs/notarizes only when credentials are available.
- Windows remains outside Viewer 0.1. Platform-neutral contracts are preserved, but no Windows adapter or build is claimed.

## Decision

M1 meets its browsing-core exit criteria with the stated M4 acceptance items. The branch may be fast-forward merged to `main`; only then may the M2 Review Efficiency plan be written and executed.
