# ADR 0003: Progressive Scan, Disposable Search Index, and Generation Safety

- Status: Accepted
- Date: 2026-07-16
- Gate: `scripts/run-g3-scan-search-gate.sh`

## Context

Viewer opens one local project containing hundreds or thousands of images and independent Markdown/TXT files. Folder IDs may occur at any depth. The UI needs a usable folder tree before all file-derived work finishes, while search, filesystem watching, and background indexing must never publish results from an old project session or query generation.

The portable `.viewer/metadata.sqlite` database remains the source of truth for project identity and user markers. Scan/search data is a disposable session artifact and must be safe to delete at application shutdown.

## Decision

### Progressive traversal

- `walkdir 2.5.0` performs non-following traversal in a Tokio blocking worker.
- Hidden components, `.viewer`, symlinks, macOS aliases, unsupported extensions, and paths whose canonical parent escapes the project root are excluded.
- Supported entries are directories, JPG/JPEG, PNG, Markdown, and TXT.
- Folder and file nodes are separated. Folders are emitted first; batches flush at 128 nodes or 20 ms. The bounded forwarding channel holds eight batches.
- A root-level read failure rejects the scan. An individual entry failure emits `FailedItem` and scanning continues.
- Unix device/inode identity supplies a stable session entity key; portable identity remains a separate `.viewer` concern.

### Disposable session index

- Each project session owns one SQLite database outside portable project metadata.
- The database uses WAL, `synchronous=NORMAL`, foreign keys, atomic batch upserts, and decimal text for `i128` modification nanoseconds.
- Scan refreshes preserve review/favorite columns. Parent nodes must already exist, so an invalid batch rolls back completely.
- Subtree deletion removes node and FTS rows transactionally. Closing the application may delete the entire session database and rebuild it next time.

### Text and search

- Markdown/TXT extraction reads at most `10 MiB + 1 byte`. More than 10 MiB is not indexed; invalid UTF-8 is reported without lossy replacement; UTF-8 BOM and line endings are normalized.
- SQLite FTS5 trigram handles body phrases with at least three Unicode scalar values.
- One- and two-character body queries inspect at most 2,000 text entities after SQL scope/type/review/favorite filters.
- `nucleo-matcher 0.3.1` performs Unicode-aware fuzzy filename and path scoring. Ranking bands are exact filename, filename fuzzy, path fuzzy, then body. Ties use normalized relative path and entity ID.
- Search returns only node data, match field, and score; indexed body text never crosses IPC as part of a result page.

### Generation and cancellation

- `TaskCoordinator` admits one active `(SessionId, Generation)` pair.
- A generation is checked before work starts and immediately before publication. Scan channel reservation is followed by an atomic coordinator-locked check and non-blocking send.
- Starting a new session, bumping a generation, or cancelling a session invalidates older queued/running work. Running workers may release resources, but their events/results are discarded.
- Priority classes and bounded capacities are: current query/folder publication P1 (8), visible derived work P2 (32), text indexing P3 (8), and fingerprints P4 (4).

### Filesystem reconciliation

- `notify 8.2.0` and `notify-debouncer-full 0.7.0` provide macOS FSEvents hints with a 250 ms debounce window.
- Watcher events never mutate an entity or index directly. Paths are lexically normalized, constrained to the project root, and converted into parent-directory reconciliation requests.
- Viewer-originated operations are recognized only by expected old/new canonical paths, file identity, and an expiry. They still cause one filesystem reconciliation; the match only prevents duplicate interpretation.
- Rescan flags and watcher errors become overflow requests at the smallest known common ancestor, or the project root when no narrower path is trustworthy.

## Performance evidence

The gate generated a deterministic corpus with 1,000 image placeholders, 100 Markdown/TXT files, 210 folders across three nested directory levels, hidden/reserved entries, a symlink, and unsupported files. Every one of 20 optimized runs created a fresh session database and indexed the text before four search queries. The standard development device was Apple M4 (`aarch64`) running macOS 26.5.2.

| Measurement | Samples | p50 | p95 | Budget | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| First usable folder event | 20 | 20.07 ms | 20.10 ms | 1,500 ms | Pass |
| Base metadata scan | 20 | 42.43 ms | 45.76 ms | 3,000 ms | Pass |
| Indexed search page | 80 | 0.72 ms | 0.95 ms | 100 ms | Pass |
| Peak RSS | process | — | 6.83 MB | 700 MB | Pass |

The gate also ran stale-generation races, atomic index tests, Unicode/short-query search tests, watcher reconciliation tests, strict Clippy, and dependency policy checks. It hashed the portable `.viewer/metadata.sqlite` sentinel before and after all disposable-index rebuilds; the hash was unchanged.

Reproduce the report at `target/g3-scan-search-gate/benchmark-report.json` with:

```bash
./scripts/run-g3-scan-search-gate.sh
```

## Consequences

- Folder browsing becomes usable before file/text work completes, and backpressure is explicit.
- Search and watcher state can be discarded without risking user markers.
- Stale result safety is enforced in the application boundary rather than relying on worker timing.
- Short CJK queries have a deliberate 2,000-text-file completeness bound; the UI can explain or refine broad short queries in very large projects.
- `nucleo-matcher` is MPL-2.0 and the notify crates are CC0-1.0 / MIT-or-Apache-2.0. Release notices must preserve their applicable license information.

## Limitations and follow-up

The generated image entries are metadata placeholders, not the user's eventual approximately 10 MB assets, and this gate does not decode them. G1 already measures image decoding separately. M4 release validation must rerun the combined workflow on the user-provided project folder and must not reinterpret these synthetic scan numbers as final real-media evidence.
