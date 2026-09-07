# 0026 — Bounded restartable bulk source adoption

## Context

Tasks 0024–0025.2 established:

- explicit single-format adoption into Caliberate-managed CAS storage;
- canonical source/reference provenance;
- source retirement/readiness metrics;
- real 105,570-book audit correctness;
- real audit query performance reduced from 1,527.46 seconds to 21.36 seconds total, with the catalog metrics themselves completing in well under one second;
- one real managed Romanovs EPUB independently verified with zero defects.

The next step is not a second adoption implementation. It is orchestration over the already-accepted `adopt_format(...)` service.

## Goal

Add a bounded, deterministic, restartable bulk-adoption service and CLI for progressively replacing a registered source's dependent reference formats with Caliberate-owned managed copies.

Core principle:

> Bulk adoption selects source-dependent logical formats and delegates every mutation to the existing single-format adoption service.

Do not duplicate CAS, hashing, compression, asset insertion, or managed-verification semantics.

## 1. Source-dependent adoption candidate query

Add a DB-level bounded keyset query for source-dependent logical formats.

Suggested row:

```rust
pub struct SourceAdoptionCandidate {
    pub book_id: i64,
    pub book_format_id: i64,
    pub format: String,
    pub reference_asset_id: i64,
}
```

Definition:

A candidate is a logical format where:

- at least one asset has `source_id = target`, `storage_mode = 'reference'`, and non-null `book_format_id`;
- no Caliberate-owned managed replacement exists for the same `book_format_id` with `storage_mode = 'copy' AND source_id IS NULL`.

For multiple references from the same source to the same logical format:

- choose the lowest reference asset ID deterministically.

The selected `reference_asset_id` MUST be passed into `AdoptFormatRequest.reference_asset_id` so bulk adoption never accidentally chooses a reference belonging to another source.

Ordering/cursor:

- deterministic ascending `book_format_id`, then `reference_asset_id` if needed;
- keyset paging only;
- hard page maximum 500;
- no OFFSET;
- no full 100k Rust materialization.

Use the accepted 0025.2 partial indexes. Add an index only if EXPLAIN evidence shows a concrete missing shape.

## 2. Bulk adoption service

Add a library-domain orchestration module, suggested:

`crates/library/src/bulk_adopt.rs`

Suggested options:

```rust
SourceBulkAdoptOptions {
    apply: bool,
    max_formats: usize,
    page_size: usize,
    problem_limit: usize,
}
```

Hard bounds:

- `page_size`: 1..=500;
- `problem_limit`: 0..=1000;
- `max_formats`: bounded; choose a conservative hard maximum suitable for one explicit invocation (for example 10,000).

CLI should default to a small bounded batch (for example 25 or 100), not the entire source.

### Dry-run mode

Default behavior MUST be non-mutating.

Without explicit `--apply`:

- select up to `max_formats` candidates;
- report what would be attempted;
- do not open/read/hash source ebook files;
- do not create managed files;
- do not insert assets.

Dry-run may use canonical DB metadata only.

### Apply mode

With explicit `--apply`:

- process candidates sequentially in deterministic order;
- for each candidate call existing `adopt_format(...)` with the selected source-specific `reference_asset_id`;
- do not reimplement source-file checks, hashing, compression, CAS naming, existing-object reuse, or managed asset insertion;
- continue after an individual candidate failure;
- record a bounded problem detail;
- never mutate/delete the source reference.

Sequential processing is preferred for this task. Do not add concurrency yet.

## 3. Restartability

Do not add a persistent job/checkpoint table in this task.

Restartability is derived from canonical state:

- successfully adopted formats cease to satisfy the source-dependent candidate query;
- rerunning the same command naturally resumes over the remaining dependent formats;
- CAS/object reuse and existing managed-copy behavior remain idempotent through the existing single-format service.

If a process is interrupted:

- already committed managed assets remain valid;
- reference assets remain untouched;
- rerun selects only still-dependent formats.

No transaction may encompass the whole batch.

Each individual adoption retains the existing commit/file semantics of `adopt_format`.

## 4. Result/report model

Return a structured result containing at least:

- `source_id`
- `apply`
- `selected`
- `attempted`
- `adopted_new`
- `already_adopted`
- `reused_existing_objects`
- `failed`
- `logical_bytes_adopted`
- `stored_bytes_adopted`
- `last_book_format_id` (if any)
- bounded `problems`

Problem detail should contain enough to diagnose:

- book_id
- book_format_id
- format
- reference_asset_id
- error message.

Aggregate counts must remain complete even when the problem list is capped.

Dry-run should report `selected` and candidate identifiers but zero mutation counters.

## 5. Progress observer

Follow the retirement-audit pattern: library code exposes progress events; CLI renders them.

Suggested events:

- `SelectionStarted`
- `CandidateSelected` only if not too noisy; prefer page/batch counts over per-file events
- `PageStarted` / `PageComplete`
- `AdoptionProgress { attempted, adopted, failed }` at reasonable intervals
- `Complete`

Do not print directly from the library crate.

Machine stdout must remain exactly one JSON object.

Progress goes to stderr.

Do not emit one progress line per file for large batches; use page/chunk cadence.

## 6. CLI

Add under the source namespace, suggested:

```text
calibredb sources adopt --id <SOURCE_ID>
    [--max-formats <N>]
    [--apply]
    [--problem-limit <N>]
    [--for-machine]
```

Semantics:

- default is dry-run;
- `--apply` is required for any source-file read or managed mutation;
- default `max-formats` is a small bounded number;
- hard maximum enforced;
- machine mode emits exactly one JSON object on stdout;
- progress/timing/logging stays on stderr;
- ordinary CLI logging config remains respected.

Do NOT add an `--all` or unbounded mode in this first task.

Full-corpus migration should be achieved by repeated bounded invocations after real acceptance proves the batch path.

## 7. Readiness integration

Before and after apply mode, obtain the fast source audit counts (catalog-only, no managed file verification).

Report at least:

- dependent formats before;
- dependent formats after;
- managed-backed formats before;
- managed-backed formats after.

In dry-run, before/after values are identical.

Do not run full retirement physical verification automatically after each batch.

This gives measurable migration progress without turning every batch into a full hash/decode pass.

## 8. Required synthetic coverage

Add focused tests covering:

1. deterministic candidate selection by source;
2. multiple references for one logical format -> lowest reference ID from the requested source;
3. reference from another source is never selected;
4. already-managed format excluded from candidate query;
5. keyset paging across more than one page;
6. dry-run performs zero filesystem reads/mutations and zero DB asset inserts;
7. apply adopts multiple candidates through existing `adopt_format` behavior;
8. one missing/unavailable source file increments failure but later candidates still succeed;
9. rerun after partial success selects only remaining dependent formats;
10. CAS existing-object reuse is counted;
11. bounded problem details with complete failure totals;
12. catalog readiness/dependency counts decrease exactly after successful adoption;
13. source reference assets remain unchanged;
14. no source file is deleted/modified.

Use synthetic sources only. Do not access the user's real library.

## 9. Query-plan coverage

Add focused EXPLAIN QUERY PLAN evidence that candidate selection uses:

- the source-reference format-oriented index;
- the managed-copy format index.

Do not depend on unstable full textual plans.

## 10. Safety invariants

Must preserve:

- Calibre/external source is read-only provenance/content source;
- source files are never modified or deleted;
- reference asset rows remain;
- canonical logical book/format IDs remain stable;
- migration occurs only by adding Caliberate-owned physical representations;
- no logical book recreation;
- managed root comes exactly from configured `paths.library_dir`;
- existing CAS/compression policy is reused.

## 11. Explicit non-goals

Do not:

- add concurrency;
- add an unbounded/all-source migration mode;
- add persistent job tables;
- detach/delete a source;
- delete reference assets;
- resync source metadata;
- repair corrupted managed assets;
- alter retirement/readiness semantics;
- add HTTP endpoints;
- change OPDS;
- change GUI behavior;
- add pack/chunk storage;
- access the real user library in automated tests.

## 12. Documentation

Update:

- `docs/project/current-status.md`
- `docs/project/library-ownership-and-storage.md` only if a durable invariant needs clarification
- `docs/work/reports/0026.md`

Document that bulk adoption is bounded, sequential, restartable from canonical state, dry-run by default, and delegates to the accepted single-format adoption primitive.

## Validation

Run on native Windows:

- `cargo fmt --check`
- `cargo test -p caliberate-assets`
- `cargo test -p caliberate-db`
- `cargo test -p caliberate-library`
- `cargo test -p caliberate-app --bin calibredb`
- `cargo check --workspace --locked`
- `cargo test --workspace --locked`

All must pass.

## Handoff

Write:

`docs/work/reports/0026.md`

Commit and push exactly one bounded implementation branch:

`codex/0026-bounded-bulk-source-adoption`

Move task to:

`docs/work/done/0026-bounded-bulk-source-adoption.md`

Return checkout to `main` before exit.

Preserve local:

`config/control-plane.toml`

Do not discard/reset/clean/overwrite/silently stash it.

Do not work on any other task.