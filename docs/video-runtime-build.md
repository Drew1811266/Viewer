# Viewer Video Runtime Build

Viewer builds and bundles its own LGPL-configured libmpv runtime. Release builds
must never search Homebrew, `/usr/local`, `PATH`, or user directories for media
libraries or tools.

## Reviewed source set

The source of truth is `scripts/video/runtime.lock.json`. It pins pkgconf,
FreeType, FriBidi, HarfBuzz, libass, FFmpeg, libplacebo, mpv, and libplacebo's
required fast_float and Vulkan-Headers submodules by official archive URL and
SHA-256. The submodule commits exactly match the gitlinks in the libplacebo
`v6.338.2` tag. The text-rendering dependencies and FFmpeg are linked statically
into `libmpv.2.dylib`; the app bundle contains the resulting libmpv dynamic
library plus the reviewed ffmpeg and ffprobe tools.

mpv is configured with `gpl=false`, `cplayer=false`, and `libmpv=true`. FFmpeg
is configured with GPL, nonfree, network, shared libraries, documentation, and
ffplay disabled. VideoToolbox and AudioToolbox remain enabled for local media.
On macOS, mpv links to the platform-provided `/usr/lib/libiconv.2.dylib` for
character-set conversion. This approved Apple system library is not part of the
video decoder payload; all non-system media dependencies remain bundled or
statically linked from the runtime lock.

mpv's supported Cocoa/OpenGL VideoToolbox bridge is built with Swift while its
player shell, Cocoa callback surface, media-player integration, and Touch Bar
features remain disabled. Swift compilation uses an architecture-specific
`*-apple-macos13.0` target rather than inheriting the installed SDK version.
The staging step removes the Xcode or command-line tools Swift runtime search
path. The packaged library may resolve Swift only from the macOS-provided
`/usr/lib/swift` runtime.

## Local arm64 build

The script requires Xcode command-line tools, Node.js, Python 3, curl, and make.
Meson, Ninja, Jinja2, and MarkupSafe are installed into the disposable build
directory using the pinned versions in `scripts/video/build-tools.txt`;
Jinja2 is required by libplacebo's source generator. These tools are not system
dependencies and do not ship with Viewer.

```bash
video_work_dir=$(mktemp -d)
video_stage_dir="$(pwd)/target/viewer-video-runtime/aarch64-apple-darwin/ViewerVideoRuntime"
VIEWER_VIDEO_WORK_DIR="$video_work_dir" \
VIEWER_VIDEO_STAGE_DIR="$video_stage_dir" \
VIEWER_VIDEO_ARCH=arm64 \
scripts/video/build-macos-runtime.sh
```

Both `VIEWER_VIDEO_WORK_DIR` and `VIEWER_VIDEO_STAGE_DIR` must be empty. Source
digests are checked before extraction. The build uses deployment target macOS
13 and fails when any required feature or dependency is unavailable.

## Verification

```bash
VIEWER_VIDEO_STAGE_DIR="$(pwd)/target/viewer-video-runtime/aarch64-apple-darwin/ViewerVideoRuntime" \
scripts/video/verify-runtime.sh
```

Verification checks the locked manifest, expected executables/library,
component license files, FFmpeg configuration, dynamic linkage, and the staged
SHA-256 inventory. Dynamic dependencies are restricted to bundle-relative
install names, `/usr/lib`, and public Apple frameworks. Other absolute paths,
including Homebrew, `/usr/local`, `Cellar`, and user directories, are release
failures. Verification also requires the approved system iconv linkage and
rejects every Swift runtime search path except `/usr/lib/swift`.

The corresponding source acquisition path is the ordered component list from:

```bash
node scripts/video/runtime-lock.mjs list scripts/video/runtime.lock.json
```
