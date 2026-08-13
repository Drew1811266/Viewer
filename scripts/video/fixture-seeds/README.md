# Task 14 synthetic codec seeds

These base64 files contain no third-party audiovisual content. Viewer project
contributors place them under CC0-1.0 for unrestricted redistribution with the
test suite.

- `vp9-opus.webm.b64` contains moving Canvas color bars encoded as VP9 by the
  repository's `generate-browser-seeds.mjs` helper plus a 440 Hz synthetic Opus
  tone encoded by the reviewed FFmpeg n8.0 runtime. The complete reviewed WebM
  is embedded because the locked runtime intentionally has no VP9 encoder and
  Matroska/WebM byte identity is not stable across remux invocations.
- `av1.mkv.b64` contains one synthetic `testsrc2` frame. The reviewed runtime
  generated a PNG, macOS ImageIO encoded that image as AVIF/AV1, and the
  reviewed runtime copy-remuxed the AV1 packet into Matroska. The locked runtime
  intentionally has no AV1 encoder.

The production fixture generator only decodes these checked-in base64 bytes.
It never invokes Chrome, ImageIO, Homebrew, VLC, PATH-resolved media tools, or
the network. It verifies the final codec/container/audio shape using the exact
`ffprobe` beside the bundled `libmpv` runtime and verifies every manifest hash.
The unsupported-codec fixture is derived byte-for-byte from the AV1 Matroska
seed by replacing its sole `V_AV1` codec identifier with the unknown same-size
identifier `V_NO1`; no decoder or host codec tool is involved.
