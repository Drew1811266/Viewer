# ADR 0005: Replace M4 release acceptance with continuous engineering governance

> Status: Accepted
>
> Date: 2026-07-23

## Context

Viewer 0.1 is an early development stage with substantial product work still ahead. Treating 0.1 as an internal release target and assigning remaining quality work to M4 conflates continuous engineering constraints with delivery acceptance.

## Decision

Viewer 0.1 is a development-stage baseline, not a release or delivery target. The M4 milestone is cancelled. Deterministic safety, dependency, regression and accessibility checks move to continuous gates or the owning feature's definition of done. Real-media, Finder, VoiceOver, soak, signing, notarization and clean-install exercises are optional specialist checks and do not block stage completion.

Active scope uses M1, M2, M3, Continuous, Future and NotApplicable. No renamed phase may recreate a centralized final acceptance gate.

## Consequences

- Existing G1–G4 and M1–M3 evidence remains historical.
- Active roadmaps no longer point to M4.
- Distribution and release acceptance requirements are not applicable to the current development stage.
- Synthetic regression evidence remains valid only for the behavior it measures.
- Future product features are planned independently from this engineering optimization program.
