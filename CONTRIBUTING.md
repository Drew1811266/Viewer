# Contributing to Viewer

## Branches and commits

- Use a short-lived descriptive branch; Codex-created branches use the `codex/` prefix.
- Keep one reviewable concern per commit and use `type(scope): outcome` commit subjects.
- Do not combine formatter-only changes with runtime refactors.

## Verification

- Run the closest focused test before editing and after each responsibility move.
- Run `pnpm verify:clean` before requesting review.
- Treat security, recovery, path-boundary and data-consistency failures as blockers.

## Dependency changes

- Use locked/frozen package-manager commands.
- Record every new direct dependency in `THIRD_PARTY_NOTICES.md` and repository policy.
- Add or update a dated exception entry before changing `deny.toml` advisory ignores.

## Architecture records

- Add a design and implementation plan before cross-module behavior changes.
- Add or supersede an ADR when a public contract, persistence schema or security boundary changes.
- Run `pnpm architecture:boundaries` for every dependency-direction, composition-root or UI bridge change; it is a mandatory gate.
- Review `pnpm architecture:trends` before adding responsibility to an existing outlier; it is non-blocking evidence, not a delivery gate.
- Change the trend baseline only with a one-to-one update to `docs/quality/architecture-trend-classifications.json`, including owner, rationale and review trigger for every exact key.

## Change discipline

- Do not stage unrelated user changes.
- Preserve IPC, Domain/Application and persistence contracts during refactors.
- Run focused tests before `pnpm verify:clean`.
- Update `THIRD_PARTY_NOTICES.md` and dependency policy for every direct dependency.
- Do not commit `.DS_Store`, generated caches or accidental project `.viewer` metadata.
- Never open `tests/fixtures/images` as a writable Viewer project; copy static media into a temporary project directory.
- Tests that create `.viewer` metadata must own a `tempfile::TempDir` or `mkdtemp` root and let it clean up after the test.
