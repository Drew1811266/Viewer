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

## Bundled-video development evidence

The video-specific development journeys (`video-section-native`, `video-first-frame-native`,
`video-controls-native`, `video-navigation-native`, `video-error-retry-native`, and
`video-cache-clear-native`) are tracked by the video acceptance record below. They are not part of
the executable shared-session controller matrix above until equivalent controller journeys exist.

Task 14 development acceptance is recorded in
[`2026-08-10-video-preview-acceptance.md`](2026-08-10-video-preview-acceptance.md).
The historical Task 14 native artifacts lack the current dirty-source,
fixture, machine, app/runtime, and shared-run binding, so the core native
decision is `UNVERIFIED` pending a new run. The collector rejects all stale or
mismatched bindings and has no timestamp fallback.

The developer smoke runs the same strict inventory/hash/architecture/loader
audit before launch. Offline status additionally requires the runner-produced
artifact showing that the exact audited app was spawned with the canonical
macOS `sandbox-exec` deny-network profile and then bound to its target process
and stable window. The emitter cannot replace this with an environment flag or
boolean. This is sandbox launch evidence, not independent packet capture.
Signed mode adds Developer ID, secure timestamp, nested/app signature, stapler,
and `spctl`; those release-signing checks remain deferred by scope.
