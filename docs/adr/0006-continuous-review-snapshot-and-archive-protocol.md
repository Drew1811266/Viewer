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

### Task 12 usage claims and output candidates

The optional importer reads only explicitly selected producer-owned project-relative files, never
scans or writes `.viewer/reviews`. Canonical declaration identity is separate from the exact source
bytes inspected. Unadopted candidates are session-bound and capped at 128 entries / 64 MiB; applying
an adoption or an archive re-inspects those bytes. Already adopted declarations are read from their
verified immutable repository copies. Neither inspection nor adoption archives feedback by itself.

The repository validates the declaration's project/stream, basis digest and complete target-key
membership. Output mappings remain raw producer claims: a bad previous-asset ID or mismatched output
does not invalidate an otherwise valid basis or erase the original declaration. This refines Phase B's
over-broad output membership rejection. Preserving a claim does **not** confirm lineage. The importer
reports output candidate checks separately; producer-backed source binding additionally proves the
selected target's old asset identity from the actual basis snapshot, matches the prepared new asset's
path and full digest, requires explicit position confirmation, and remains subject to fresh source
checks. Unrelated later rebinding cannot be laundered through an earlier declaration. No hashing or
declaration field proves who produced an output or whether the requested work was performed.

### Task 13 explicit migration refinements

Migration inspection binds both the original index digest and a composite inspection digest covering
all indexed legacy record digests and the active draft. The latter is essential because draft saves
do not update the legacy index. Only the latest completed record of each stream is offered as a
candidate; all indexed records and declared PNGs are verified, with at most 10,000 legacy records and
64 MiB of retained candidate/draft wire bytes per inspection. No source-file scan implies consent.

Prepare expands migration identities once into the command envelope. Apply and the repository use
the same pure migration-state builder; retries never regenerate identities. The provider receives
`MigrationCommitRequest` (envelope, prepared state and staged evidence), not just a choice/digest:
context, exact generated IDs and image evidence are necessary for the atomic migration boundary.
The command cache accounts for migration IDs; one migration is capped at 100,000 selected targets.

The v3 index has an optional hash-only `legacyIndex` reference; the canonical backup is
`recovery/legacy-index-{blake3}.json`. Legacy refs distinguish `draft` from `completed` (the default
for existing v3 engineering fixtures). Drafts remain at their original canonical paths and bytes,
not relabeled as Completed. Recovery enumeration recognizes verified index backups separately from
editable input. Interrupted attempts may leave multiple immutable backups; these are bounded,
never interpreted as Agent requirements, and not automatically deleted.

Draft feedback retains its identity/text and gains target/text revisions. Old v1/v2 image records
have no immutable clean base: those targets explicitly retain `LegacyEvidenceAbsent` pending state,
even if a live source currently matches. Only an explicit new-asset binding and position confirmation
permits fresh capture. Video source checks remain independent; no video backup is fabricated.
Completed feedback is never automatically current; selected targets gain new identities/provenance,
and unbound selections remain pending. `ContinueLegacy` uses LegacyTargetRef plus explicit bindings
after migration, rather than inventing v3 keys for old targets. History returns a typed Draft or
Completed record and no actionable list. Another stream's active draft blocks an inapplicable context.

A legacy catalog with no active draft and no indexed history has nothing to migrate: it reads as
no_review_state and opening it changes no bytes. Its first successful new save retains the empty
old index backup and creates the first state. Unindexed orphan files are not adopted. Nonempty legacy
data requires explicit migration; unknown protocol versions fail closed. All migration writes share
the legacy writer lease and recheck the complete inspection before atomic index replacement. Faults
after replacement return OutcomeUnknown; earlier faults leave the old entry valid. Legacy writers
reject v3. No real project is migrated and no compatibility "latest" entry is emitted.

### Phase C review corrections

Prepared envelopes bind each selected usage ID to its inspected canonical digest, source-byte digest
and relative path. A selection without an inspected candidate must already exist as an immutable
adopted usage. Reimporting the same ID with different contents after a restart cannot change the
meaning of an outstanding command. ReviewCommandCodecPort::usage_digest computes the same canonical
usage digest as the repository. An adopted record no longer depends on the external file remaining.

Recovery retains unpublished input, never Agent instructions or authorization to replay. Its private
closed wire format now includes exact origin keys, new target/asset identities, confirmation choices
and geometry for historical continuation, legacy continuation, source/applicability confirmation and
continue-as-new restoration. Missing provisional geometry remains explicitly unconfirmed. Early
usage/transition failures also preserve submitted input when storage permits. Optional migration
input preserves the exact inspection digest and selected legacy targets/new bindings. The provider's
save_migration_recovery / load_migration_recovery work before a v3 index exists, sharing the legacy
writer lease without publishing or modifying the old index. Recovery never bypasses fresh preview,
source, inspection, position or expected-snapshot checks; prepared asset handles are not automatically
reconstituted after restart. Already-committed migrations are resolved before considering cancellation.

ConfirmApplicability is separate from source replacement: it revises a provisional target anchor
on the same asset only when its sole stored pending reason is ApplicabilityUnconfirmed. It cannot
clear missing clean evidence or source restrictions. Read-time source checks still decide projection.

History projection coalesces exact SnapshotRefs and target selections, with a conservative 64 MiB
aggregate retained-payload budget across distinct entries. Recovery reconciliation traverses committed
history once per view, leaving unavailable tails unresolved. Exact snapshot/legacy provenance is
verified once per operation, including all declared legacy PNGs; no cross-operation trust is cached.
Checkpoint verification also batches groups by exact basis reference before reading JSON/PNG. It
derives outcomes per batch, then restores the original group and target order without merging their
source metadata; only one full basis state is retained between batches.
Missing index plus legacy drafts is rejected by ordinary writers as well as migration inspection.
The optional public legacyIndex field rejects explicit null, matching its JSON Schema.

Recovery retention remains finite: 10,000 records and 64 MiB total record bytes per project, including
records whose commands already committed. Index backups have a separate 64-record / 64 MiB bound.
Repeated saves therefore consume cumulative capacity; reaching a limit fails closed and does not
delete history or recovery input. This is not an unlimited-session or production-scale guarantee.
Any future retention/compaction policy requires separate design and verification before UI rollout.

The `continuous_review_state`, `continuous_review_mutation`, `continuous_review_archive`,
`continuous_review_restore` and `continuous_review_delta` integration suites cover the pure rules.
Checkpoint evidence and any contract refinements are recorded in the
[Phase A record](../reviews/2026-08-27-continuous-review-domain-phase-a.md).
