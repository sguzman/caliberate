# 0025 — Source retirement/readiness audit

## Goal

Add a first-class, scalable audit that answers:

> How dependent is the canonical Caliberate library on a registered external source, and is that source safe to detach from the runtime content graph?

This task is measurement only.

It must not mutate source mappings, assets, files, or source registration.

The immediate real source is the materialized legacy Calibre source with ~105,570 books and ~106,949 source-backed logical formats. One Romanovs EPUB is now managed; the rest are still legacy-reference dependent.

## Core principle

A source-retirement audit must not require the source itself to be available.

If Caliberate is trying to prove that a source can disappear, the audit cannot depend on opening/reading that source.

Therefore:

- catalog readiness uses only the canonical Caliberate DB;
- physical verification reads only Caliberate-managed copies;
- never open/stat/hash the legacy/reference source files;
- never open the source's `metadata.db`;
- never instantiate `CalibreLibraryBackend`.

## Readiness model

For a target `library_sources.id`, define:

### Source-backed logical format

A logical format is source-backed by this source when at least one linked asset has:

```text
assets.book_format_id = logical format
assets.source_id = target source
storage_mode = "reference"
```

Use distinct `book_format_id` identities.

Multiple legacy reference rows for the same logical format count as one source-backed format.

### Caliberate-managed replacement

A source-backed logical format is managed-backed when it has at least one linked asset with:

```text
storage_mode = "copy"
source_id IS NULL
```

on the same `book_format_id`.

This task intentionally uses Caliberate-owned `copy` assets as the retirement requirement.

Do not count another unowned/reference asset as sufficient retirement coverage yet.

### Source-dependent format

A source-backed logical format with no Caliberate-managed replacement.

### Catalog-ready

```text
source_dependent_formats == 0
AND structural_anomalies == 0
```

Catalog-ready means canonical metadata says every source-backed logical format has an owned representation.

It does **not** yet mean those managed files were physically verified.

### Verified-ready / retirement-ready

Only true after an explicit managed verification pass and only when:

- catalog-ready;
- every selected managed representation exists as a regular file;
- selected managed path is within configured `paths.library_dir`;
- stored size matches DB;
- logical decoded size matches DB;
- checksum exists;
- logical SHA-256 matches DB checksum;
- zstd representations decode successfully;
- no verification errors/anomalies remain.

Call the final boolean `retirement_ready`.

Do not call a source retirement-ready from catalog counts alone.

## 1. DB aggregate audit

Add focused source-audit DB queries, preferably in:

```text
crates/db/src/database/canonical.rs
```

or a focused sibling module if that keeps canonical.rs from becoming unwieldy.

Create a DB/domain result with at least:

```text
source_id
mapped_books
source_reference_assets
source_backed_formats
managed_backed_formats
source_dependent_formats
metadata_only_source_books
fully_managed_source_books
source_books_with_dependencies
unlinked_source_assets
orphan_source_assets
catalog_ready
```

Definitions:

### mapped_books

Count rows in `source_books` for target source.

### source_reference_assets

Count asset rows with:

```text
source_id = target
storage_mode = "reference"
```

### source_backed_formats

Distinct non-null `book_format_id` among those source reference assets.

### managed_backed_formats

Distinct source-backed `book_format_id` values having at least one same-format:

```text
storage_mode = "copy"
source_id IS NULL
```

### source_dependent_formats

```text
source_backed_formats - managed_backed_formats
```

computed from relational identity, not subtraction if anomalies could make subtraction incorrect.

### metadata_only_source_books

Mapped source books with zero source-backed logical formats.

These do not block retirement because their metadata already lives in the canonical DB.

### fully_managed_source_books

Mapped source books that have at least one source-backed format and for which every source-backed format has a managed replacement.

### source_books_with_dependencies

Mapped source books having at least one source-dependent format.

### unlinked_source_assets

Source-linked reference assets whose `book_format_id IS NULL`.

These are structural anomalies and block catalog readiness.

### orphan_source_assets

Source-linked assets whose `book_id` has no `source_books` mapping for this source.

These are structural anomalies and block catalog readiness.

Use aggregate SQL / CTEs.

Do not load 100k books and count in Rust.

Do not issue one query per book or format.

## 2. Coverage percentage

Expose a deterministic coverage percentage:

```text
managed_coverage_percent
```

Semantics:

- if `source_backed_formats == 0`, define coverage as 100.0;
- otherwise:
  `managed_backed_formats / source_backed_formats * 100`.

Human output may show two decimal places.

Keep integer counts authoritative.

Do not use this percentage for readiness decisions.

## 3. Verification candidate query

Add a bounded, keyset-paged query that returns the preferred Caliberate-managed replacement for each source-backed logical format.

Preferred selection must match current content selection semantics:

- `storage_mode == "copy"`;
- same `book_format_id`;
- lowest asset ID wins.

Return enough data to verify:

```text
book_id
book_format_id
format
asset_id
stored_path
size_bytes
stored_size_bytes
checksum
is_compressed
```

Paging:

- deterministic keyset by `book_format_id` (and asset ID tie-break if needed);
- default/max page size 500;
- no unbounded full-result materialization;
- no OFFSET walk over the full corpus.

This query must consider only source-backed formats for the requested source.

## 4. Library retirement audit service

Create a focused module, for example:

```text
crates/library/src/retirement.rs
```

Preferred API, naming may vary:

```rust
pub struct SourceRetirementAuditOptions {
    pub verify_managed: bool,
    pub page_size: usize,
    pub problem_limit: usize,
}

pub struct SourceRetirementAudit {
    pub source: LibrarySourceRow,
    pub mapped_books: u64,
    pub source_reference_assets: u64,
    pub source_backed_formats: u64,
    pub managed_backed_formats: u64,
    pub source_dependent_formats: u64,
    pub metadata_only_source_books: u64,
    pub fully_managed_source_books: u64,
    pub source_books_with_dependencies: u64,
    pub unlinked_source_assets: u64,
    pub orphan_source_assets: u64,
    pub managed_coverage_percent: f64,
    pub catalog_ready: bool,

    pub verification_performed: bool,
    pub managed_candidates_verified: u64,
    pub missing_managed_files: u64,
    pub managed_paths_outside_root: u64,
    pub stored_size_mismatches: u64,
    pub logical_size_mismatches: u64,
    pub missing_checksums: u64,
    pub checksum_mismatches: u64,
    pub decode_errors: u64,
    pub verification_errors: u64,

    pub retirement_ready: bool,
    pub problems: Vec<SourceRetirementProblem>,
}
```

Exact naming may vary.

Keep `problems` bounded by `problem_limit`; counts must remain complete even when detail collection is capped.

## 5. Managed verification semantics

When `verify_managed == true`, verify only preferred managed replacements.

Do **not** inspect legacy/reference paths.

For each preferred managed asset:

### Managed-root containment

The stored path must be under configured:

```text
paths.library_dir
```

Use path semantics consistent with current configured-database server authorization.

Do not require the source locator/root.

If an existing file can be canonicalized cheaply and safely, canonical containment may be used; do not turn this task into a cross-platform symlink-policy rewrite.

### File presence

- missing -> `missing_managed_files += 1`;
- directory/non-regular -> verification error.

### Stored size

Actual physical file length must equal `asset.stored_size_bytes`.

Mismatch increments `stored_size_mismatches`.

### Logical size

- identity -> actual file length;
- zstd -> decoded byte count using existing bounded streaming decoder/helpers.

Must equal `asset.size_bytes`.

Mismatch increments `logical_size_mismatches`.

### Checksum

A retirement-grade managed asset requires a recorded checksum.

If absent:

```text
missing_checksums += 1
```

and it is not verified.

If present:

- identity -> hash stored logical bytes;
- zstd -> hash decoded logical bytes;
- compare to recorded checksum.

Mismatch increments `checksum_mismatches`.

### Decode failure

Invalid zstd increments `decode_errors`.

Do not panic.
Do not fall back to source reference.
Do not repair/delete anything.

### Verification count

`managed_candidates_verified` should count candidates that passed all required checks.

## 6. Retirement-ready calculation

`retirement_ready = true` only when:

```text
verification_performed
&& catalog_ready
&& managed_candidates_verified == source_backed_formats
&& missing_managed_files == 0
&& managed_paths_outside_root == 0
&& stored_size_mismatches == 0
&& logical_size_mismatches == 0
&& missing_checksums == 0
&& checksum_mismatches == 0
&& decode_errors == 0
&& verification_errors == 0
```

If no source-backed formats exist:

- catalog readiness may be true if structural anomalies are zero;
- verified readiness may be true after an explicit verification pass confirms there were zero candidates.

## 7. Problem detail model

Provide bounded problem details sufficient for remediation, for example:

```text
kind
book_id
book_format_id
format
asset_id
path
message
```

Do not expose these through HTTP/JSON API in this task.

CLI only.

Never include source file contents.

## 8. CLI source commands

Add a focused top-level command group:

```text
calibredb sources list
calibredb sources audit --id <SOURCE_ID> [--verify-managed] [--problem-limit <N>] [--for-machine]
```

Exact nesting may vary slightly if the existing Clap shape strongly suggests another style.

### sources list

Show at least:

```text
id
kind
label
locator
read_only
last_sync_at
```

This is read-only.

### sources audit

Uses the configured canonical DB and configured `paths.library_dir`.

Defaults:

- `verify_managed = false`;
- verification page size internal default 500;
- `problem_limit = 50`.

Hard-bound any public/internal page size to 1..=500.

Hard-bound `problem_limit` to a reasonable maximum such as 1000.

Do not expose source filesystem probing.

## 9. Machine-readable CLI output

For `--for-machine`, emit exactly one JSON object on stdout containing all audit counts/booleans and bounded problems.

Logs may remain on stderr according to existing conventions.

The JSON must be straightforward for a PowerShell acceptance blob to parse.

At minimum include:

```json
{
  "source_id": 1,
  "mapped_books": 105570,
  "source_backed_formats": 106949,
  "managed_backed_formats": 1,
  "source_dependent_formats": 106948,
  "managed_coverage_percent": 0.0009,
  "catalog_ready": false,
  "verification_performed": true,
  "managed_candidates_verified": 1,
  "retirement_ready": false,
  "problems": []
}
```

Do not promise these exact real counts in tests.

## 10. Synthetic catalog-audit tests

Build a canonical DB with one source and a mix:

### Book A

- EPUB source reference;
- managed EPUB copy.

### Book B

- EPUB source reference only.

### Book C

- EPUB + PDF source references;
- managed EPUB only.

### Book D

- mapped metadata-only book.

### Book E

- source reference asset with null `book_format_id` anomaly.

### Orphan source asset

- source-linked reference for a book with no `source_books` mapping to this source.

Prove exact aggregate counts:

- mapped books;
- source reference assets;
- source-backed distinct formats;
- managed-backed formats;
- dependent formats;
- metadata-only books;
- fully managed books;
- books with dependencies;
- unlinked source assets;
- orphan source assets;
- catalog_ready false.

Then repair anomalies/dependencies in fixture and prove catalog_ready true.

No filesystem access should be needed for catalog-only audit.

## 11. Synthetic verification tests

Use temporary managed files.

Prove individually:

1. healthy identity managed asset verifies;
2. healthy zstd managed asset verifies;
3. missing managed file counted;
4. path outside managed root counted;
5. stored-size mismatch counted;
6. logical-size mismatch counted;
7. missing checksum counted;
8. checksum mismatch counted;
9. corrupt zstd counted as decode error;
10. problem detail list is bounded while aggregate counts remain complete;
11. no legacy/reference file is opened or required.

For point 11, source reference paths may deliberately point to nonexistent files and verification must still succeed/fail solely according to managed-copy state.

## 12. Real acceptance target

Do not access the real user library in automated tests.

After merge, human acceptance will run one self-contained PowerShell blob against:

Canonical DB:

```text
A:\Data\Books\db\caliberate.sqlite
```

Managed root:

```text
\\wsl$\Ubuntu\mnt\wsl\PHYSICALDRIVE0p1\books\managed
```

Expected current shape after one real Romanovs adoption:

```text
mapped_books ~= 105570
source_backed_formats ~= 106949
managed_backed_formats ~= 1
source_dependent_formats ~= 106948
catalog_ready = false
verification_performed = true
managed_candidates_verified ~= 1
retirement_ready = false
```

Treat those as human expectations, not test constants.

## 13. Performance constraints

Catalog-only audit must be aggregate SQL and should be fast on 100k+ books.

Verification may be expensive because it hashes/decodes managed content, but it must:

- page candidates in bounded chunks;
- avoid loading all candidate rows into memory;
- never read source/reference content;
- emit progress at page/chunk granularity, not per asset.

Do not add timing thresholds to automated tests.

## 14. Documentation

Update:

```text
docs/project/library-ownership-and-storage.md
docs/project/current-status.md
```

Document the distinction:

```text
catalog_ready
  = canonical graph says source is no longer required

retirement_ready
  = catalog_ready plus explicit verification of all managed replacements
```

Explicitly state source detachment/deletion is **not implemented** by this task.

## Architecture constraints

- read-only audit;
- source independence is measured from Caliberate's canonical graph;
- verification reads only Caliberate-owned managed representations;
- no source mutation;
- no source filesystem read;
- no per-book/per-format SQL N+1;
- no unbounded verification result materialization;
- logical format identity remains central.

## Explicit non-goals

Do **not**:

- detach/delete a source;
- delete reference assets;
- delete/move legacy Calibre files;
- bulk-adopt formats;
- resync metadata;
- repair broken managed assets;
- add HTTP/JSON endpoints;
- change OPDS;
- change GUI behavior;
- add pack/chunk storage;
- access the user's real library in automated tests.

## Expected files

Likely:

- focused DB audit queries under canonical DB module;
- `crates/library/src/retirement.rs`;
- `crates/library/src/lib.rs` wiring;
- `crates/app/src/bin/calibredb.rs`;
- focused tests;
- docs;
- `docs/work/reports/0025.md`;
- move task to:
  `docs/work/done/0025-source-retirement-audit.md`.

Do not grow unrelated god files substantially.

## Validation

Run on native Windows:

```text
cargo fmt --check
cargo test -p caliberate-assets
cargo test -p caliberate-db
cargo test -p caliberate-library
cargo test -p caliberate-server
cargo test -p caliberate-app --bin calibredb
cargo check --workspace --locked
cargo test --workspace --locked
```

All must pass.

## Handoff

Write:

```text
docs/work/reports/0025.md
```

with:

- aggregate SQL design;
- exact readiness definitions;
- verification behavior;
- explicit proof source/reference files are never read;
- paging/bounds;
- CLI syntax and machine JSON shape;
- tests and validations actually run;
- statement that real full-source audit is pending human acceptance;
- statement that source detachment remains future work.

Commit and push exactly one bounded implementation branch:

```text
codex/0025-source-retirement-audit
```

Return checkout to `main` before exit.

Do not work on any other task.
