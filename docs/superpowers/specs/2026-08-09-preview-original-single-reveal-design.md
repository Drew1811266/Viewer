# Preview Original Single-Reveal Design

## Goal

When a user opens or navigates to an image preview, keep the loading progress visible until the original image is ready for its final fitted presentation. The first visible image frame must already use the original representation and the current preview-stage geometry. A fit-preview proxy must not appear as an intermediate frame.

## Current defect

`ImagePreview` currently treats either `fit_preview` or `original100_percent` as sufficient to mark the preview ready. The fit request normally completes first, so the progress indicator exits and the proxy becomes visible. When the original request completes, the component upgrades the same image to the original. This intentional progressive path creates the reported small-image frame and later jump.

## Selected behavior

- Continue requesting and caching `fit_preview` for adjacent navigation and recovery.
- While the current original is loading, render the original image element only as a non-visible loading candidate; keep the progress indicator visible.
- Do not make the preview image visible until all of these are true:
  - the original representation belongs to the current entity;
  - the browser has completed loading the representation URL;
  - the preview stage and source dimensions are positive;
  - the viewport geometry has been calculated from those final dimensions.
- Reveal the original once with the existing short opacity animation. Do not reveal a fit proxy before it.
- Reset browser-load readiness when the entity or original cache key changes, so navigation cannot reuse stale readiness.

## Recovery behavior

If the original cannot be supplied because of a decode/request failure or the safe-preview budget, the existing fit-preview representation remains the fallback. In that case, wait until the fallback URL is browser-loaded and its fitted geometry is ready, reveal it once, and retain the existing warning explaining why the fit preview is being used. If both representations fail, replace progress with the existing error state.

## State and data flow

`useCurrentOriginal` remains the owner of the original request lifecycle. `ImagePreview` derives a single display candidate:

1. Original ready: select original.
2. Original terminal failure: select cached fit fallback.
3. Otherwise: select no visible candidate and keep loading.

The selected candidate is mounted with an entity-and-cache-key identity. Its load event records browser readiness only when it still matches the current candidate. The existing stage measurement and viewport calculation then establish final display geometry. The progress indicator is hidden only when candidate load readiness and geometry readiness agree for the same identity.

## Testing

- Regression: resolve the fit proxy first and assert that progress remains visible and no preview image is visible.
- Resolve the original and assert it is still not visible until its browser load event.
- Fire the original load event and assert the first visible frame is the original at the final fitted size, with progress hidden.
- Navigation: a load event from the previous entity must not reveal the next entity.
- Recovery: an original terminal failure may reveal a browser-loaded fit fallback and keeps the warning.
- Preserve existing zoom, pan, magnifier, rotation, and image-navigation tests.

## Non-goals

- No change to backend image generation, cache limits, magnifier settings, or toolbar layout.
- No preloading gate before opening the preview overlay; the overlay opens immediately and provides progress feedback.
- No artificial delay after the image is ready.
