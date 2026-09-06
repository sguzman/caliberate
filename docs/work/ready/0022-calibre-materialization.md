# 0022 — Materialize a legacy Calibre source into the canonical Caliberate catalog

## Goal

Import/materialize an existing read-only Calibre library into Caliberate's own mutable canonical SQLite catalog **without copying ebook files**.

This is the first real offramp from perpetual attached-Calibre operation.

Durable architecture:

```text
docs/project/library-ownership-and-storage.md
```

Task 0021 established:

- `library_sources`;
- `source_books`;
- `book_formats`;
- format-aware `assets`;
- managed `Database` multi-format resolution.

This task uses that foundation.

## Target lifecycle

Given:

```text
legacy Calibre source
  <root>/metadata.db
  <root>/<Calibre book paths>/...
```

and a Caliberate-owned target DB:

```text
A:\Data\Books\db\caliberate.sqlite
```

materialization must produce:

```text
Caliberate canonical DB
  books + metadata relations
  source provenance
  logical formats
  reference assets -> legacy Calibre files
```

After successful materialization, ordinary catalog/query operations against the target Caliberate DB must not require reopening the Calibre `metadata.db`.

Actual ebook content still depends on the legacy file tree until formats are later adopted into Caliberate-managed storage.

## Critical non-filesystem rule

Materialization is a metadata/database operation.

Do **not** recursively scan the Calibre directory tree.

Do **not** `stat`/open/hash every ebook file during import.

Derive reference asset paths from:

- Calibre `books.path`;
- Calibre `data.name`;
- Calibre `data.format`;

using the existing safe Calibre path validation.

The source `metadata.db` supplies logical sizes.

File existence/integrity auditing belongs to a later explicit audit task.

The only filesystem checks allowed during source opening are the existing root / `metadata.db` validation already performed by `CalibreLibraryBackend`.

## 1. Source-neutral canonical import record

Add a focused canonical import/write model in the DB layer, preferably under:

```text
crates/db/src/database/canonical.rs
```

Naming may vary, but it should express a source-neutral record roughly equivalent to:

```rust
CanonicalBookImport {
    external_id,
    external_uuid,
    external_modified,

    title,
    sort,
    timestamp,
    pubdate,
    series_index,
    author_sort,
    uuid,
    has_cover,
    last_modified,

    authors,
    tags,
    series,
    publisher,
    rating,
    languages,
    identifiers,
    comment,

    primary_format,
    primary_path,

    formats: Vec<CanonicalFormatImport>,
}

CanonicalFormatImport {
    format,
    size_bytes,
    representations: Vec<CanonicalAssetImport>,
}

CanonicalAssetImport {
    storage_mode,
    stored_path,
    source_path,
    size_bytes,
    stored_size_bytes,
    checksum,
    is_compressed,
}
```

This DB input type must not mention Calibre.

It should be reusable by future directory/source importers.

Do not introduce a second catalog abstraction.

## 2. Canonical chunk writer

Add a DB API, naming may vary, for example:

```rust
Database::materialize_source_books(
    source_id,
    records: &[CanonicalBookImport],
    imported_at_or_seen_at,
) -> CanonicalMaterializeBatchResult
```

### Transaction behavior

One source page/chunk must be written inside one SQLite transaction.

Do **not** call existing high-level per-book APIs that each start their own independent transaction for every authors/tags/languages relation.

The point is to avoid hundreds of thousands of tiny transactions on a 105k-book import.

The writer may use focused transaction-local helper functions.

Do not rewrite unrelated existing DB APIs.

### Per-book atomicity within a chunk

A malformed record should fail the chunk transaction rather than leave a half-materialized canonical book from that chunk.

Previously committed chunks may remain and make the import resumable.

### Existing source mappings

Before inserting a record, check the persisted identity:

```text
(source_id, external_id)
```

If it already exists:

- skip it;
- do not create another canonical book;
- do not overwrite local canonical metadata;
- do not rewrite formats/assets;
- count it as `skipped_existing`.

**This is materialization/resume, not resync.**

Explicit source refresh/reconciliation is the next task.

### New canonical book

For a new source record:

1. create one canonical `books` row;
2. preserve the Calibre primary compatibility projection:
   - `books.format` = normalized lowest-data-id primary format, or empty;
   - `books.path` = primary reference path, or empty for metadata-only;
3. write supported metadata relations;
4. create `source_books` mapping;
5. create all logical `book_formats`;
6. create one reference asset for every valid physical Calibre `data` row;
7. each asset:
   - `storage_mode = "reference"`;
   - `book_format_id` = matching canonical logical format;
   - `source_id` = registered Calibre source;
   - `stored_path` = metadata-derived legacy file path;
   - `source_path = NULL` unless there is a strong existing convention requiring otherwise;
   - `size_bytes` / `stored_size_bytes` from Calibre `uncompressed_size` when nonnegative, otherwise use a safe neutral representation consistent with the existing non-null asset columns;
   - `checksum = NULL`;
   - `is_compressed = false`.
8. no file copy.

### Metadata to materialize

Materialize the currently supported canonical fields when available from modern Calibre base tables:

- title;
- sort;
- timestamp/date added;
- pubdate;
- series index;
- author sort;
- uuid;
- has_cover;
- last_modified;
- authors;
- tags;
- series;
- publisher;
- rating;
- languages;
- identifiers;
- comment/description.

Do not materialize:

- Calibre custom columns;
- annotations/reading positions;
- saved searches/preferences;
- covers as physical assets;
- plugin data.

Those are separate future tasks.

### Relation order

Preserve deterministic source relation order where Calibre has one.

At minimum:

- authors preserve link-table ID order;
- languages preserve `item_order` then deterministic tie-break;
- formats preserve `data.id` order.

Tags/identifiers may use stable deterministic source order.

## 3. Calibre materialization reader

Create a focused module, for example:

```text
crates/library/src/calibre/materialize.rs
```

Do not put this into the existing large `calibre/mod.rs`.

It owns Calibre-specific extraction into the source-neutral canonical import records.

### Source SQL

Use only modern Calibre base tables.

Do not depend on:

- Calibre views;
- `concat`;
- `books_list_filter`;
- `title_sort`;
- other Calibre user-defined SQL functions.

### Paging

Read books using deterministic keyset paging by Calibre book ID:

```sql
WHERE b.id > ?last_id
ORDER BY b.id ASC
LIMIT ?page_size
```

Do not use an unbounded `SELECT *` over 105k books.

Do not use OFFSET for the full import walk.

Default source page size:

```text
500 books
```

Keep it bounded/configurable in code for tests; no user-facing tuning is required unless trivial.

### Relation/format loading

For each page:

- batch-load metadata relations for the page IDs;
- batch-load `data` rows including:
  - `data.id`;
  - `data.book`;
  - `data.format`;
  - `data.uncompressed_size`;
  - `data.name`;
- no per-book query loop.

Bound SQLite ID parameters conservatively, consistent with existing 400-ID chunking when using `IN (...)`.

### Paths

Reuse the existing safe Calibre path construction.

A `data` row path must be derived without checking whether the ebook currently exists.

Case-only duplicate logical formats:

- one canonical `book_formats` identity;
- retain deterministic physical representation rows in `data.id` order if multiple source rows exist;
- primary remains lowest `data.id`.

Unsafe Calibre path components must be an explicit import error, not silently rewritten.

## 4. Materializer service

Add a focused public entry point in `caliberate-library`, naming may vary:

```rust
materialize_calibre_source(
    source: &CalibreLibraryBackend,
    target: &mut Database,
    options: CalibreMaterializeOptions,
) -> CalibreMaterializeReport
```

Options should minimally support:

- optional source label;
- page size for tests/default 500.

Do not accept a filesystem-walk mode.

### Source registration

Register:

```text
kind = "calibre"
locator = canonical source.library_root() string
read_only = true
```

using `Database::upsert_library_source`.

Identity must be stable on repeated materialization of the same canonicalized root.

### Completion sync timestamp

Only after the entire source scan completes successfully:

- update `library_sources.last_sync_at` / materialization completion timestamp.

If a later chunk fails:

- already committed earlier chunks remain;
- source remains resumable;
- do not falsely mark full completion.

This timestamp means "last completed materialization pass" for now. True resync semantics come next.

## 5. Result/report model

Return/report at least:

```text
source_id
source_books_seen
imported_books
skipped_existing
metadata_only_books
logical_formats
reference_assets
last_external_id
completed
```

Also expose elapsed time in CLI/log output if convenient, but do not bake wall-clock timing into deterministic DB tests.

Tracing:

- one info event at start;
- one progress event per committed page/chunk;
- one completion event;
- do not log one info line per book.

## 6. CLI

Add a bounded first-class command to:

```text
calibredb
```

Preferred shape:

```text
calibredb import-calibre \
  --source <CALIBRE_LIBRARY_ROOT> \
  --database <CALIBERATE_DB_PATH> \
  [--immutable] \
  [--label <LABEL>]
```

Exact clap spelling may vary slightly, but keep the intent obvious.

### Target database

`--database` explicitly selects the Caliberate-owned target SQLite file.

Open it through the normal Caliberate `Database` migration/config path.

Create parent directories when needed.

Do not ever point the target at source `metadata.db`.

If source `metadata.db` and target DB resolve to the same file/path identity, reject clearly before writing.

### Source mode

- default: `CalibreOpenMode::LockingReadOnly`;
- `--immutable`: `CalibreOpenMode::ImmutableReadOnly`.

The immutable mode must reuse the existing Windows UNC/WSL `win32-none` behavior.

Do not add automatic fallback.

For the user's real WSL source, human acceptance will use `--immutable`.

### Output

Human-readable output must include the final counters.

If the existing CLI has a machine-readable convention that is easy to reuse, supporting it is welcome but not required by this task.

## 7. No server source-mode change yet

Do not change `calibre-server` source selection in this task.

After import, the existing configured-Database server can already point at the target DB via config.

A later human acceptance will prove:

```text
local Caliberate DB -> server metadata/query
legacy Calibre paths -> referenced content bytes
```

Do not add a new "hybrid server source" abstraction.

## 8. Synthetic tests — source extraction

Build a synthetic modern Calibre fixture with at least:

### Book 1

- title/sort/uuid/dates;
- two ordered authors;
- tags;
- series + non-default index;
- publisher;
- rating;
- two ordered languages;
- identifiers;
- comment;
- PDF data.id 10;
- EPUB data.id 11.

### Book 2

- one format.

### Book 3

- metadata-only.

Prove:

1. keyset page extraction order by external book ID;
2. all supported metadata fields are read correctly;
3. relation order is deterministic;
4. formats retain `data.id` order;
5. primary is lowest `data.id`;
6. reference paths are constructed correctly;
7. no source content file needs to exist for extraction/materialization;
8. unsafe source path is rejected.

For point 7, deliberately do **not** create the ebook files in at least one successful materialization test.

## 9. Synthetic tests — canonical materialization

Materialize into a fresh Caliberate DB and prove:

1. source registry created once;
2. three canonical books created;
3. source external IDs map to canonical IDs;
4. metadata is present through existing DB APIs;
5. book 1 primary compatibility format/path is PDF;
6. book 1 logical formats are PDF then EPUB;
7. PDF and EPUB reference assets are linked to the correct logical format;
8. assets carry `source_id`;
9. asset `storage_mode == "reference"`;
10. no ebook files were copied into a Caliberate library directory;
11. metadata-only book has no logical formats/assets;
12. target DB is independently queryable after the source backend object is dropped;
13. source `metadata.db` bytes are unchanged.

## 10. Resume/idempotence tests

Run materialization twice against the same source/target.

Prove second run:

- imports zero duplicate canonical books;
- counts existing mappings as skipped;
- preserves canonical IDs;
- does not duplicate logical formats;
- does not duplicate assets;
- does not overwrite a deliberate local canonical metadata edit made between runs.

This is critical.

Do not implement resync to make the local edit match Calibre.

## 11. Partial-resume test

Use a test-only option/fault seam or bounded materializer primitive to simulate:

- first page commits;
- later page is not processed / materialization stops;
- rerun resumes by source mapping and completes missing books;
- first-page canonical books are not duplicated.

Do not add production "crash simulation" CLI flags.

## 12. Performance evidence

Automated test must include a synthetic source of at least ~100 books and a small page size to exercise multiple source pages/chunks.

Code structure must demonstrate:

- keyset paging;
- batch relation loading;
- batch format loading;
- chunk transaction writes;
- no per-book source SQL query loop;
- no per-book SQLite transaction loop;
- no ebook filesystem scan.

Do not build a timing threshold into tests.

## 13. Documentation

Update:

```text
docs/project/library-ownership-and-storage.md
docs/project/current-status.md
```

only as needed to record the implemented materialization behavior.

Document the operational distinction:

```text
attached-Calibre direct mode
  = inspect/serve external catalog directly

import-calibre/materialized mode
  = canonical metadata lives in Caliberate DB;
    ebook bytes remain reference assets until adopted
```

## Architecture constraints

- Caliberate DB is canonical mutable catalog.
- Calibre is read-only provenance/source.
- Import is materialization, not synchronization.
- Logical format != physical asset.
- No file copying.
- No recursive filesystem scan.
- No per-book SQL N+1 source loading.
- No per-book transaction N+1 target writing.
- No source mutation.
- No Calibre executable/process.
- No GUI product work.
- Keep new code in focused modules.

## Explicit non-goals

Do **not**:

- implement resync/update-existing behavior;
- overwrite local canonical edits on repeat import;
- migrate/copy books into managed storage;
- implement source retirement audit;
- add archive-member/chunk storage;
- hash legacy ebook files;
- verify every legacy file exists;
- import covers as assets;
- import custom columns;
- import annotations/reading positions;
- add server write APIs;
- change JSON/OPDS shapes;
- change attached-Calibre service behavior;
- access the user's real library in automated tests.

## Expected files

Likely:

- `crates/db/src/database/canonical.rs`
- focused DB import structs/helpers/tests
- `crates/library/src/calibre/materialize.rs`
- `crates/library/src/calibre/mod.rs` only small module/export wiring
- Calibre materialization tests
- `crates/app/src/bin/calibredb.rs`
- docs
- `docs/work/reports/0022.md`
- move task to:
  `docs/work/done/0022-calibre-materialization.md`

Do not grow `database.rs` or `calibre/mod.rs` substantially.

## Validation

Run on native Windows:

```text
cargo fmt --check
cargo test -p caliberate-db
cargo test -p caliberate-library
cargo test -p caliberate-server
cargo test -p caliberate-app --bin calibredb
cargo test -p caliberate-app --bin calibre-server
cargo check --workspace --locked
cargo test --workspace --locked
```

All must pass.

## Handoff

Write:

```text
docs/work/reports/0022.md
```

with:

- source reader architecture;
- canonical chunk writer architecture;
- exact imported metadata fields;
- reference-asset path derivation;
- explicit proof that ebook files are not scanned/copied;
- resume/idempotence behavior;
- source completion timestamp behavior;
- CLI syntax;
- tests and validations actually run;
- explicit statement that full 105,570-book real-library import is pending human acceptance;
- explicit statement that resync/adoption/source-retirement remain future work.

Commit and push exactly one bounded implementation branch:

```text
codex/0022-calibre-materialization
```

Return the checkout to `main` before exit.

Do not work on any other task.
