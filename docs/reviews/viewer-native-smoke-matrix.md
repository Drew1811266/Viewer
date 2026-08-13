# Viewer Native Smoke Matrix

This matrix is the bounded macOS integration layer for Viewer. Browser visual acceptance owns the 92 established atlas states plus 12 video states; these journeys prove only behavior that requires the Tauri process, native window, system picker, Finder, bundled video runtime, or synthesized desktop input. Windows-native behavior remains pending until Windows development begins.

The controller uses one clean copied fixture, one Viewer process, one native helper session, and one project session. The native folder picker is opened once. Finder is invoked only by the two Finder rows. A source fixture reset is permitted only after a journey that actually mutates the disposable fixture; none of the current journeys commits such a mutation.

| ID | Native boundary | Pass condition | Session rule |
| --- | --- | --- | --- |
| `launch-single-instance` | Development launch and PID binding | Exactly one current-worktree Viewer process and one approved window are bound | Start shared session |
| `launch-empty` | Native empty window | The project chooser is visible before a project is opened | Reuse process |
| `open-project-picker` | macOS folder picker | The one-time disposable project opens through the system picker | Open picker once |
| `workspace-scan` | Tauri scan bridge | Loading indicators settle and the workspace is visible | Reuse opened project |
| `sidebar-window` | Native window geometry | Sidebar collapse/resize remains operable at the requested viewport | Reuse opened project |
| `thumbnail-keyboard` | Synthesized keyboard focus | Thumbnail focus and arrow-key selection move through real accessibility nodes | Reuse opened project |
| `radial-native` | Native context click | A right-click opens the approved circular file menu | Reuse opened project |
| `preview-native` | Radial command activation | The circular menu opens the real image preview | Reuse opened project |
| `compare-native` | Multi-selection shortcut | Two selected images open the comparison workspace | Reuse opened project |
| `text-native` | Desktop preview shortcut | A text document opens in the formal text preview | Reuse opened project |
| `video-section-native` | Tauri scan, probe, and registered cover bridge | The expanded `视频 · N` section keeps fixed card geometry and shows registered cached covers without exposing a filesystem path | Reuse opened project |
| `video-first-frame-native` | Bundled libmpv and AppKit render surface | Double-clicking a playable card opens the shell immediately, reveals one native first frame at final fitted geometry, and shows no enlarged cover or black flash | Reuse opened project and playback generation |
| `video-controls-native` | Typed playback commands and native events | Play/pause, frame movement, seek, volume, mute, rate, fullscreen, final-frame stop, and Escape ownership remain synchronized with the active generation | Reuse active playback generation |
| `video-navigation-native` | Generation-scoped native teardown and reopen | Previous/next stays within videos, closes the old generation, and first-frame-gates the newly opened entity | Reuse opened project; replace playback generation |
| `video-error-retry-native` | Normalized engine failure and deterministic retry | A failed preview retains navigation, Retry, Done, and Escape; Retry finishes closing the failed generation before reopening the same entity | Reuse opened project; replace failed generation |
| `video-cache-clear-native` | Runtime cache statistics and verified clear command | Settings reports current usage against the fixed 1 GiB limit and confirmed clear leaves the current decoded frame uninterrupted | Reuse active playback generation |
| `info-shortcut` | Command-key shortcut | Command-I opens the file information inspector | Reuse opened project |
| `rename-dialog-native` | Native menu-to-dialog path | The radial organize action opens the rename dialog and cancels without mutation | Reuse opened project |
| `finder-drop-valid` | Finder drag, valid source | Dropping the disposable project folder opens it successfully | Finder only; source unchanged |
| `finder-drop-invalid` | Finder drag, invalid source | Dropping a file shows the folder-only rejection state | Finder only; source unchanged |
| `close-project-native` | Desktop project lifecycle | The project closes and Viewer returns to the native empty state | End shared project session |

## Evidence contract

- Every run is bound to a clean commit, exact executable path, PID, window ID, and `1024x720` or `1440x900` viewport.
- The smoke report records each journey result and action log separately from browser visual verdicts.
- A failed journey is never promoted to visual acceptance. Diagnostics may continue only when the remaining journey is non-destructive and the shared session can be restored safely.
- Browser `forced-colors` evidence is not macOS Increase Contrast or Windows High Contrast evidence.
