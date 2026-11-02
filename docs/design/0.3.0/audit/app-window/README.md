# Real app window samples — 2026-11-01

Capture method: activate `viewer-desktop`, then `screencapture -R95,45,1280,800`;
the 2× Retina image is downscaled to 1280×800 for review. The window is the
real Tauri app, not the acceptance harness.

- `01-launch-before.png` — launch screen before the 2026-11-01 card fix.
- `02-workspace.png` — project workspace with two folders.
- `05-preview-workbench.png` — opening an image enters the continuous review
  workbench (toolbar + feedback rail), not the clean preview of design frame 05.
- `09-annotation-draft-highlighter.png` — highlighter stroke + floating
  “返工意见” composer; the multiply blend lets the shirt pattern show through.
- `24-markup-menu.png` — the old vertical markup list, kept as a before image.
- `24-markup-menu-continuous.png` — the new grouped palette (acceptance
  harness, continuous review; 11 entries).
- `25-more-menu.png` — workspace “更多” menu.

The test project is `/tmp/Viewer-QA-project`; it is not committed.
