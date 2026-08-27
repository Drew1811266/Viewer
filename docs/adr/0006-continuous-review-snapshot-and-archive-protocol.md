# ADR 0006: Continuous review snapshot and archive boundaries

> Status: Active
>
> Decision: Accepted for Phase A Domain and Task 6 wire contracts; repository integration remains planned.
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

Domain validates supplied state/identity relationships, not file bytes, real-time source freshness,
global uniqueness across an unloaded repository, or graph reachability. Phase B must verify digests,
bounded reference traversal, immutable records and source evidence. Later application/reader code must
not treat a Domain state or a restore plan as an authorization to execute feedback.

Captured `AssetVersion` records, including their historical locator/mtime, remain immutable. Task 10
must represent a confirmed live relocation in the asset catalog or a separate locator mapping; it
must not mint a new content version for a rename or weaken captured-content validation. That locator
ownership/interface is a required Phase B refinement, not an implemented Phase A capability.

Atomic repository commits, evidence capture, migration, application services, Agent readers and UI
are not implemented by the Phase A and Task 6 contract work. Codecs validate declarations, not actual
file bytes or graph reachability; they do not authorize execution of feedback on disk.

No protocol bytes, original materials, product UI or version number are changed in Phase A. Signing,
notarization, formal installers, publication and sales are outside the current development scope.

## Verification

The `continuous_review_state`, `continuous_review_mutation`, `continuous_review_archive`,
`continuous_review_restore` and `continuous_review_delta` integration suites cover the pure rules.
Checkpoint evidence and any contract refinements are recorded in the
[Phase A record](../reviews/2026-08-27-continuous-review-domain-phase-a.md).
