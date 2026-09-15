# ADR 0007: Image protocol response caching is memory-only reuse

> Status: Active
>
> Decision: `viewer-image://` responses carry `Cache-Control: private, max-age=60`
> (previously `no-store`). Image bytes remain out of persistent storage outside
> the project; WebKit may reuse image resources in the WebContent process memory
> cache for 60 seconds. Adopted 2026-09-15.
>
> Date: 2026-09-15

## Context

The image protocol originally answered every request with `Cache-Control:
no-store` so that review image bytes would never be persisted by WebKit. The
security goal stands: images belonging to a `.viewer/` project must not leak to
caches the project does not control.

The cost showed up in scroll-performance sampling (30 s, WebContent main
thread, 2026-09-15): with `no-store`, WebKit cannot reuse a decoded image
across `<img>` remounts. Every scroll pass over previously seen filmstrip cells
re-decodes the bitmap (IIO samples in the paint path) and re-copies the decoded
pixels into a shareable bitmap for the GPU process
(`ShareableBitmap::createFromImagePixels` + `memmove`, ~170 samples). Repeated
scrolling therefore kept paying decode + copy costs with no reuse — the
remaining main-thread hitch source after the prefetch, unmount and
content-visibility fixes.

## Decision

- Serve `Cache-Control: private, max-age=60` on every image protocol response
  (success and error paths share one `response` builder).
- `viewer-image://` is a custom scheme served by the WKURLSchemeHandler. Custom
  scheme responses never enter WebKit's on-disk network cache; the only consumer
  of the header is the in-memory WebCore `MemoryCache`. The change therefore
  reuses decoded bitmaps inside one WebContent process lifetime without writing
  image bytes to disk — the original security property is preserved.
- Retention is bounded: 60 s freshness plus normal memory-cache pressure
  eviction. Nothing survives app relaunch.
- Staleness is impossible by construction: protocol URLs embed the session id
  and a content-keyed registry token, so changed image content yields a
  different URL and cannot hit an old cache entry.
- The unmount and prefetch windows in `FolderFilmstripRow` stay in place: they
  bound live rendering and request churn; the memory cache only makes remounts
  cheap, not unbounded.

## Consequences

- Repeat scrolling reuses decoded bitmaps: no re-decode, no re-copy to the GPU
  process, and no repeated scheme-handler IPC for cache hits.
- WebContent RSS may grow with recently viewed images within the 60 s window;
  scroll sessions must be re-measured (see `docs/quality/perf-baseline.md`)
  before accepting the change.
- Any future switch to a persistent cache for this scheme would be a new
  security-boundary decision requiring a further ADR.
