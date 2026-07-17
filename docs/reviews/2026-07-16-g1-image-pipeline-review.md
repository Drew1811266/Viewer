# G1 image pipeline stage review

- Status: Passed
- Date: 2026-07-16
- Base: `bade4b4508755b27144cca121fd0c62637845baa`
- Reviewed head: `965e06aeeaf094eaf12dcab17e022d20ed8a7ae8`
- Review method: requirements traceability, full-diff inspection, focused native integration gates, repository verification and dependency policy audit

## Exit-criteria traceability

| G1 exit criterion | Evidence | Result |
| --- | --- | --- |
| Quick Look thumbnails and Image I/O previews share one port | `ImagePort` is implemented by `MacImagePort`; thumbnail coordination selects Quick Look with Image I/O fallback and non-thumbnail rendering uses Image I/O | Pass |
| ICC, EXIF, transparency and corrupt-input behavior | Deterministic sRGB, Display P3, orientation 6, alpha and truncated fixtures; metadata tests plus backend pixel comparison | Pass |
| Cancellation and stale-publication prevention | Blocking coordinator test and 100-request native cancellation run; 100 active cancellations, zero other errors, stale publications or stale files | Pass |
| Restricted delivery | Session-bound 128-bit opaque tokens, exact normalized protocol paths, GET/authority checks, MIME allowlist, restrictive response headers and CSP tests | Pass |
| Decode and application memory limits | Checked arithmetic, 100,000,000-pixel full-decode ceiling, proxy-set budgeting and measured 112,361,472-byte process peak against the 700,000,000-byte gate | Pass |
| Backend decision is recorded | ADR 0001 selects Quick Look for grid thumbnails with Image I/O fallback; fit and permitted 100% previews use Image I/O | Pass |

## Review findings

No critical or important findings remain open.

The review specifically checked native-object isolation, cancellation races, generated-artifact cleanup, cache-key stability, cross-session token rejection, traversal/authority bypasses, absolute-path disclosure, MIME confusion, overflow handling, corrupt images and fallback behavior. Production code contains no unchecked panic path in the reviewed image pipeline or protocol request flow; the one response-builder `expect` operates only on compile-time static headers.

## Accepted limitations

- The deterministic valid fixtures are 640×480 to 1024×768. They establish correctness and the prototype resource envelope, but do not replace the M4 acceptance run with the user's approximately 10 MB images and several-hundred-image project.
- G1 proves the image port, cache registry and restricted delivery boundary as composable components. M1 owns their project-session/UI command integration and session-directory cleanup.
- Decode concurrency and memory-pressure scheduling remain product-milestone work, as recorded in ADR 0001.

These limitations are already mapped to later roadmap milestones and do not invalidate the G1 architecture decision.

## Verification

The accepted stage evidence includes:

```text
./scripts/run-g1-image-gate.sh          PASS
pnpm verify                             PASS
cargo clippy --workspace --all-targets  PASS with -D warnings
cargo deny --offline --locked check     PASS
git diff --check                        PASS
```

The dependency audit reports only the duplicate-version and unused-license-allowance warnings already permitted by repository policy; advisories, bans, licenses and sources all pass.

## Decision

G1 meets its roadmap exit criteria and is approved for a fast-forward merge into `main`. G2 may begin only after the merge is verified from the main worktree.
