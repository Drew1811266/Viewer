# ADR 0006: Continuous review snapshot and archive boundaries

> Status: Active
>
> Decision: Accepted and verified for Phases A–B (Tasks 1–10); application/UI/Agent integration remains pending.
>
> Date: 2026-08-27

## Context

Viewer is an early-development AI material review tool. Users keep editing natural-language feedback
and manually archive previous iteration requirements. Archiving does not prove execution, resolution
or approval. The existing Completed round remains the active product behavior until the new chain is
implemented and explicitly connected.

The approved [design](../superpowers/specs/2026-08-27-viewer-continuous-review-and-agent-handoff-design.md)
and [implementation plan](../superpowers/plans/2026-08-27-viewer-continuous-review-and-agent-handoff.md)
separate pure rules from protocol, repository, application and UI phases.

## Decision implemented in Phase A

- Add `review::continuous` without modifying legacy round semantics. No IO or new dependencies.
- A snapshot contains the complete current requirements, not derived pass outcomes. An empty current
  state is valid and never falls back to historical instructions.
- Feedback text and each target have separate revisions. Exact archival identity is
  `(feedbackId, textRevisionId, targetId, targetRevisionId)`. Shared feedback can be archived per target.
- Archive known basis B while retaining later C edits and additions. Unknown usage archives only the
  explicitly confirmed current keys. Competing bases are an error; active coverage is deduplicated.
- `ArchiveCheckpoint.before` is a full `SnapshotRef`, including for unknown usage. The checkpoint
  also carries project/stream identity. This avoids fabricating a digest when continuing old feedback.
- Pure apply functions recompute and validate their selections. New states do not invent parent
  digests; application/repository must attach the verified committed predecessor.
- Restoration is explicitly selected and cannot alter unselected shared text. Continuing as new
  requires fresh identities, an explicit creation timestamp and a history reference. An unconfirmed
  anchor is pending, not executable. Continuing a request does not unarchive its historical source.
- Restore plans share immutable text/content and asset payloads by identity. They do not clone an
  entire shared feedback for each selected target; application of the plan uses indexed target merges.
- Source checks are ephemeral inputs. Saved Ready alone is insufficient: projection requires a fresh
  matching check. Restoration does not bypass this gate or mutate historical availability facts.
- Deltas compare net text, geometry, binding and availability. Removed requirements need a connected
  typed transition explaining withdrawal versus archival. Deltas never include old prose or invent
  reverse instructions. History-only events do not replace the actual removal cause.
  Target ownership remains fixed across absent intervals, including historical-only events.

## Wire contract implemented in Task 6

- Add the explicit `review::v3` codecs and five closed JSON schemas. State/index/archive use
  `viewer.review/3`; producer declarations use `viewer.review.usage/1`. Legacy dispatch still rejects
  v3 and cannot reinterpret it as Completed. The application and external reader are not switched.
- State contains full Domain content, command identity/digest, typed transitions and image evidence
  bindings. Snapshot references contain ID and BLAKE3; their canonical `states/{id}.json` location is
  derived, not accepted as an arbitrary path. Index archive/usage/legacy locations must match the ID
  and record type. Legacy locations and bytes remain unchanged.
- Unknown archive usage is `usageBasis: null`; `beforeRef` still identifies the actual historical
  content. Removed/retained keys must account for every selected target exactly once. Repository
  verification of real basis bytes and reachability remains necessary before trusting the result.
- Read results distinguish current, explicit history, no state and errors. Current projection is
  checked against source checks and saved availability. History has a selector and separate entries
  per basis, rather than flattening different revisions into a fabricated snapshot. Legacy entries
  retain legacy Feedback/round identities and do not invent v3 target/revision IDs. History has no
  actionable field. Archive entries include exact selected keys alongside the immutable basis content.
- Required nullable fields must be present. Tags without payload reject extra fields too. UUIDs use
  hyphenated representation, digests are lowercase hex, nanosecond timestamps are canonical decimal
  strings, and other integer values are restricted to JavaScript's exact integer range. Source paths
  are portable, bounded project-relative strings, with no `.viewer`, traversal or backslash aliases.
- Per-feedback/asset limits remain as designed; aggregate target-shaped arrays are bounded by the
  product of the existing feedback and per-feedback target limits. JSON byte limits remain the tighter
  document bound. Encoding stops growing its buffer at that bound rather than allocating an unlimited
  document first. Evidence declarations enforce single-PNG and unique-content aggregate budgets.
- Wire adapters are split by state, asset, feedback/history and change responsibility. They do not add
  serialization dependencies to Domain. Six owned temporary fixture cases validate real hashes and
  transitions across Node-produced JSON and Rust; static media and legacy fixtures are read-only.

## Trust boundary and remaining work

Task 7 interface refinement: application commit requests carry `production: Option<ProductionScope>`;
stored snapshots expose the same scope from their fixed index view. A new stream is registered with
that explicit scope; an existing stream must match it exactly. This supplies the task/batch metadata
absent from the pure Domain state without guessing or rewriting another stream's ownership. Wire
records remain infrastructure-owned and are mapped to independent application types.

Task 7 implements the application repository ports and a filesystem adapter sharing the existing
`write.lock` lease. Atomic publication uses descriptor-relative temporary files and rename in
the same atomic module for both legacy and continuous repositories. The writer also checks that its
lease file and repository directory have not been substituted. Readers do not create metadata.
Immutable create-once uses exclusive rename (Darwin RENAME_EXCL / Linux RENAME_NOREPLACE), never
link-then-unlink: interruption must not strand a double-linked object rejected by nlink checks.
Unsupported exclusive-rename platforms/filesystems fail closed rather than weaken this invariant.

The transaction verifies expected reference and stream scope, validates state/archive/usage inputs,
installs immutable evidence and records, checks their references, rechecks the fixed index bytes,
then publishes one index atomically. Existing object names cannot be reused for different bytes.
Partial archives preserve shared feedback and later edits. Snapshot-origin references and claimed
archive/usage transitions require actual committed evidence; unindexed files do not establish history.
An archive's known basis must be reachable from that checkpoint's historical `before`, not just from
today's current head: restoring the same target later cannot become an earlier archive's basis.
Image base identity stays fixed for a captured AssetVersion across the verified parent chain.

Domain exposes the narrow `validate_shared_identities(candidate, historical)` validator so the
repository can check historical sightings one snapshot at a time without duplicating identity rules
or loading all complete states simultaneously. The adapter separately verifies hashes/reachability.
PNG installation and reads stream one file at a time and check digest, size and image header;
native capture/decoding/annotation rendering remains Task 9, not an implemented repository capability.

Domain validates supplied state/identity relationships, not file bytes, real-time source freshness,
global uniqueness across an unloaded repository, or graph reachability. Phase B must verify digests,
bounded reference traversal, immutable records and source evidence. Later application/reader code must
not treat a Domain state or a restore plan as an authorization to execute feedback.

Captured `AssetVersion` records, including their historical locator/mtime, remain immutable. Task 10
implements confirmed live relocation in the session-owned asset catalog with exact entity/path,
full hash verification and prior-locator CAS. It does not mint a content version for a confirmed rename
or mutate historical metadata. The locator is not a cross-process persistence protocol; later readers
without this authority must verify the captured path or report pending, never guess a new location.
Source checks always stream the content and validate pre/post identity. Preparation incrementally
retains prior tracking. Continuous preparation bypasses both image and video metadata caches and
checks one identity interval spanning hashing and probing. It maps raw ImagePort dimensions into
EXIF-upright review dimensions (5–8 swap axes, invalid orientations are not reviewable), matching
native capture; ImagePort and legacy preparation retain their prior contracts.
Explicit binding revises only the selected target; producer-proof validation
and user position confirmation remain Application prerequisites, not implied by a Domain enum.

Task 8 implements persistent command deduplication before CAS, six fault boundaries, bounded recovery
and history traversal. RecoveryDraft adds an explicit stream_id (including first-save recovery);
the internal closed `viewer.review.recovery/1` format is never Agent feedback. Recovery lists are
bounded to 10,000 records and 64 MiB total. Saves check projected count/bytes under the writer gate,
accounting for replacements before publication. Up to 64 staging remnants are separately bounded
and ignored; they are not recovery input, and neither input nor remnants are automatically deleted.
The index fault is after rename and before directory sync;
uncertain results preserve the published index and are resolved by command/digest, never rolled back.
Active archive coverage cannot be recorded twice; explicit restoration permits subsequent archival.

Task 9 adds immutable in-process BoundReviewImage bytes with complete captured asset identity and
EXIF-upright pixel mapping. Trusted adapters verify hashes before constructing it; native rendering
verifies again and refuses annotated images as clean bases. Source capture uses a bounded streaming
copy into owned private scratch, then decodes only that copy. Staging leases own temporary files until
the repository has copied them. History selectors load only committed evidence; ambiguous archive
previews require an exact snapshot. Legacy evidence absence is distinct from declared-file corruption.

Explicit legacy migration/provenance admission, application services,
Agent readers and UI are not connected by Tasks 6–9. Existing legacy indexes are not silently replaced;
legacy-origin continuous states need the later explicit migration adapter. Repository validation does
not authorize execution of feedback on disk.

No protocol bytes, original materials, product UI or version number are changed in Phase A. Signing,
notarization, formal installers, publication and sales are outside the current development scope.

## Verification

### Phase C application interface refinements (Task 11)

The service constructor additionally takes an immutable `ReviewWorkspaceContext` (project, stream,
production scope). The same context is covered by the command digest. Viewing a stream cannot silently
change the target of a subsequent command. `prepare_assets` binds the in-process preview to captured
AssetVersion IDs before editing; apply only uses those versions or committed immutable evidence.
Redraw input includes its original asset ID so geometry can survive a stale command even after the
target has been withdrawn by another writer. These are application contracts, not new product choices.

Prepare generates and retains command identities without repository writes. The pending-envelope
cache is bounded by both 128 entries and a conservative 64 MiB retained-payload budget; committed
entries are released. Apply accepts the original complete envelope after restart, recomputes its
digest, and looks up the command before CAS. A known commit followed by a view-refresh error returns
`CommittedViewUnavailable(receipt)`, not a claim that the write failed. The original recovery-input
payload stays independent of later current state. `apply_with_cancellation` carries explicit shared
cancellation; blocking repository operations run on the blocking executor.

Command hashing is an internal `viewer.review.command/1` stream of explicitly ordered/tagged JSON
values separated by NUL, including context, generated identities, expected snapshot and the entire
command, excluding the digest itself. It is streamed into BLAKE3 with a 64 MiB cap; no Debug text,
filesystem path authority or client-supplied digest is trusted. Wire/IPC DTO encoding remains Phase D.

Local annotation pixels can be reused for text-only edits, but their TargetVersionKey mappings are
updated to the new text revision in the same state. Changed target revisions or numbering trigger
rendering from the verified immutable base. History views retain separate snapshot entries and
explicit selected keys; they have no actionable field. Coverage lookup is bound to an exact head.

Task 11 measures archive-heavy persistence. A single fixed repository View now retains at most
10,000 hash-verified `(stream, snapshot-ref) -> parent-ref` edges. These are bounded ancestry facts,
not cached full states or image bytes. Repeated reference proofs reuse those facts only within that
operation; requested records, archives, usage declarations and required evidence are still read and
verified. A new operation starts with no inherited proof cache. The historical-before ancestry rule,
cycle/digest checks and per-walk bound are unchanged. This reduces repeated filesystem reads without
claiming constant cost or large-project latency guarantees.

The `continuous_review_state`, `continuous_review_mutation`, `continuous_review_archive`,
`continuous_review_restore` and `continuous_review_delta` integration suites cover the pure rules.
Checkpoint evidence and any contract refinements are recorded in the
[Phase A record](../reviews/2026-08-27-continuous-review-domain-phase-a.md).
