# ADR 0010: Offline dictation through macOS Speech

> Status: Active (0.3.0)
>
> Decision: 0.3.0 provides local dictation through the system Speech framework
> with on-device recognition required. Viewer bundles no ASR model, sends no
> audio to a network service, and refuses dictation when the system reports
> that on-device recognition is unavailable.
>
> Date: 2026-11-01
>
> Supersedes the bundled-model / model-card proposal in
> `docs/milestones/viewer-0.3x-development-plan.md` §5.1.

## Context

The 0.3.0 design puts a microphone inside the review feedback composer. The
product is local-first: no accounts, no cloud sync, no telemetry, and review
evidence is generated on the machine. Audio is sensitive even when the source
images are not, so sending it to a hosted recognizer is out of scope.

The planning phase compared bundled open-source recognizers (Qwen3-ASR,
Whisper, SenseVoice, Paraformer) and found real costs: 180 MB–1 GB model
artifacts, per-model licenses and notices, a staging/packaging pipeline,
hash-pinned downloads, and cold-start latency. macOS 13+ already ships an
on-device Simplified Chinese recognizer through `SFSpeechRecognizer`; the
system owns the model, updates it with the OS, and exposes a clear
`supportsOnDeviceRecognition` signal.

The previous implementation set
`requiresOnDeviceRecognition(supportsOnDeviceRecognition)`. That meant a
machine without the on-device asset could silently fall back to Apple's
network recognizer. The UI already hid the entry point when `onDevice` was
false, but the platform service itself did not enforce the privacy contract.

## Decision

- **Use `SFSpeechRecognizer` with `requiresOnDeviceRecognition = true`.**
  `recognize_file` returns `SpeechError::Unavailable` when
  `supportsOnDeviceRecognition()` is false. There is no network fallback in
  the platform service, even if a caller bypasses the UI.
- **No bundled model.** No model binary, download script, hash manifest, or
  model license is added. `docs/product/ASR_MODEL.md` documents the system
  model, not a redistributed one.
- **Locale:** 0.3.0 exposes Simplified Chinese (`zh-CN`) only. The setting is
  an enum, not a free-form locale, so an unsupported locale cannot be
  persisted. `SpeechStatus.locale_supported` and `on_device` are shown in
  Settings; unavailable states explain why.
- **Audio lifecycle:** recording writes a temporary mono WAV in the system
  temp directory, named with a UUID. The file is deleted after recognition,
  on cancellation, on too-short recordings, and on start failures. Nothing is
  written to the project, the review protocol, or a cache.
- **Permissions:** speech recognition and microphone are requested
  independently. The UI distinguishes not-determined, denied, restricted, and
  authorized states, and keeps the typed draft when permission or recognition
  fails.
- **Input modes:** click-to-start and press-and-hold are UI preferences; both
  use the same recorder and recognizer. Punctuation is a request option
  (`addsPunctuation`), not a post-processing heuristic.
- **Result shape:** the platform returns text, elapsed time, and an `onDevice`
  flag. The application does not persist audio; only the user-confirmed text
  enters the feedback draft.

## Consequences

- Availability depends on the macOS version, installed dictation assets, and
  the user's privacy settings. Viewer cannot make dictation available by
  shipping a model; the Settings and composer must explain the system
  dependency.
- There is no model artifact to audit, stage, or license. The trade-off is
  less control over model quality and language coverage; a future bundled
  model would need a new ADR, not an incremental change.
- Recognition quality is Apple's; automated tests cover permission/status
  mapping, timeouts, temp-file cleanup, and cancellation, not transcription
  accuracy.
- On-device enforcement is a hard invariant: a future refactor that removes
  the `supportsOnDeviceRecognition` guard would violate the product privacy
  contract and should fail review.
- The framework call is synchronous and runs in `spawn_blocking`; the 90-second
  recognition timeout (30 s) and the 250 ms minimum recording bound the wait.

## Alternatives considered

- **Bundle Whisper / Qwen3-ASR / SenseVoice.** Rejected for 0.3.0: model size,
  license/notice work, download staging, cold start, and memory pressure
  outweigh the quality gain for short review notes. The research is kept in
  `docs/quality/2026-10-28-asr-model-research.md` for a later decision.
- **Call a hosted speech API.** Rejected: violates the local-only product
  boundary and adds accounts/network failure modes.
- **Apple Speech with automatic network fallback.** Rejected: the user cannot
  tell when audio leaves the machine; the UI would have to disclose a
  different privacy mode for a feature that is advertised as offline.
- **Record and keep audio for re-transcription.** Rejected: unnecessary
  sensitive data retention; the user can re-record.
