# 0021 — Canonical catalog provenance and logical-format foundation

## Goal

Make the existing Caliberate-owned SQLite database explicitly capable of representing:

- a mutable canonical logical book;
- external source provenance;
- multiple logical formats per book;
- one or more physical asset representations linked to those formats;
- external/reference versus Caliberate-managed storage.

This is the schema/domain foundation for materializing a legacy Calibre library into a Caliberate-owned database on a normal local path such as:

```text
A:\Data\Books\db\caliberate.sqlite
```

Do **not** import a real or synthetic Calibre catalog in this task. The actual Calibre -> canonical DB materializer is the next task after this foundation is accepted.

The durable architecture is documented in:

```text
docs/project/library-ownership-and-storage.md
```

Read it before implementation.

## Product invariant

External Calibre `metadata.db` is a source/provenance database, not the long-term canonical catalog for a maintained Caliberate library.

Caliberate's own `Database` remains the mutable catalog.

Existing ingest/assets/metadata code must be extended, not replaced by a second competing database subsystem.

## Existing pieces to preserve

The existing managed DB already has:

- `books` and rich metadata relation tables;
- `assets` with:
  - `book_id`;
  - `storage_mode` (`copy` / `reference`);
  - `stored_path`;
  - `source_path`;
  - sizes;
  - checksum;
  - compression flag;
- mutable metadata APIs;
- add/remove/clone/asset verification workflows;
- a Calibre-parity `data` table.

Do not remove these.

The existing Calibre-parity `data` table is **not** the new canonical logical-format table. Its `name` semantics are Calibre-specific. Introduce an explicitly Caliberate-owned logical-format table instead.

## 1. Schema version and migrations

Bump the managed Caliberate database schema version by exactly one.

Add three canonical tables:

### `library_sources`

Purpose: registered external provenance/sync sources.

Required columns:

```text
id            INTEGER PRIMARY KEY
kind          TEXT NOT NULL
locator       TEXT NOT NULL
label         TEXT
read_only     INTEGER NOT NULL DEFAULT 1
created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
last_sync_at  TEXT
```

Constraint:

```text
UNIQUE(kind, locator)
```

Examples of `kind` values:

- `calibre`
- `directory`

Do not hardcode a CHECK constraint that prevents future source kinds.

`locator` is an opaque source locator string at the DB layer. Path canonicalization belongs to the source adapter/CLI, not generic DB schema code.

### `source_books`

Purpose: map a canonical Caliberate book to an external source record.

Required columns:

```text
id                 INTEGER PRIMARY KEY
source_id          INTEGER NOT NULL
book_id            INTEGER NOT NULL
external_id        TEXT NOT NULL
external_uuid      TEXT
external_modified  TEXT
imported_at        TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
last_seen_at       TEXT
```

Constraints/indexes:

```text
UNIQUE(source_id, external_id)
UNIQUE(source_id, book_id)
INDEX(source_id)
INDEX(book_id)
```

Foreign keys to `library_sources(id)` and `books(id)`.

Do not implement sync conflict policy yet.

### `book_formats`

Purpose: canonical logical format inventory independent of physical storage.

Required columns:

```text
id          INTEGER PRIMARY KEY
book_id     INTEGER NOT NULL
format      TEXT NOT NULL COLLATE NOCASE
size_bytes  INTEGER
created_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
```

Constraints/indexes:

```text
UNIQUE(book_id, format)
INDEX(book_id)
```

Foreign key to `books(id)`.

Format semantics:

- normalize format strings to lowercase in DB APIs;
- empty format is invalid for a `book_formats` row;
- `size_bytes` is logical/uncompressed source size when known;
- unknown size -> NULL.

## 2. Extend `assets` without replacing it

Add nullable columns:

```text
book_format_id  INTEGER
source_id       INTEGER
```

with indexes.

Semantics:

- `book_format_id` links the physical asset representation to one logical `book_formats` row;
- `source_id` identifies the external source that owns/provided the physical representation when applicable;
- native Caliberate-managed assets may have `source_id = NULL`;
- old/pre-migration assets remain valid.

Foreign keys where practical without breaking existing SQLite migration behavior.

Do **not** add archive-member fields yet.
Do **not** invent a second assets table.

## 3. Migration/backfill existing managed libraries

The migration must preserve existing user DBs.

For every existing `books` row with non-empty `books.format`:

1. insert/upsert one `book_formats` row using lowercase `books.format`;
2. use NULL size unless an existing authoritative canonical source exists.

For existing `assets` rows whose `book_format_id` is NULL:

- if the owning book has a non-empty canonical `books.format`, link the asset to that corresponding backfilled `book_formats` row;
- leave `source_id = NULL`.

Do not infer multiple formats from asset filenames during migration.

Do not mutate/delete content files.

Migration must be idempotent under the repository's migration model.

## 4. DB row/domain structs

Add explicit DB-layer structs, naming may vary slightly:

```rust
LibrarySourceRow
SourceBookRow
BookFormatRow
```

Extend:

```rust
AssetRow
```

with:

```rust
pub book_format_id: Option<i64>
pub source_id: Option<i64>
```

Keep existing fields.

## 5. Source provenance DB APIs

Add bounded APIs to `Database`:

### Sources

```text
upsert_library_source(kind, locator, label, read_only) -> source_id
get_library_source(id)
find_library_source(kind, locator)
list_library_sources()
update_library_source_last_sync(id, timestamp)
```

Upsert semantics:

- identity is `(kind, locator)`;
- repeated registration returns the same source ID;
- label/read_only may be updated to the supplied current values;
- do not silently change locator identity.

### Source-book mappings

```text
upsert_source_book(
    source_id,
    book_id,
    external_id,
    external_uuid,
    external_modified,
    last_seen_at
)
get_source_book(source_id, external_id)
list_source_books(source_id)
```

Repeated upsert of the same `(source_id, external_id)` updates:

- canonical `book_id`;
- `external_uuid`;
- `external_modified`;
- `last_seen_at`.

Do not overwrite `imported_at` on ordinary refresh.

## 6. Logical-format DB APIs

Add:

```text
upsert_book_format(book_id, format, size_bytes) -> book_format_id
get_book_format(book_id, format)
list_book_formats(book_id)
list_book_formats_for_books(book_ids)
remove_book_format(book_id, format)
```

Semantics:

- format is normalized lowercase;
- case-insensitive identity;
- deterministic order by `book_formats.id`;
- repeated upsert updates `size_bytes` when a new explicit size is supplied;
- do not replace a known size with NULL;
- missing book -> normal DB error/constraint behavior consistent with existing APIs;
- removal only removes the logical row when no linked asset would be orphaned, or returns a clear error. Do not silently orphan assets.

### Batched loader

`list_book_formats_for_books` must:

- be parameterized;
- use bounded/chunked IDs;
- not query once per book;
- return empty vectors for requested books with no logical formats.

This supports summary pages after imported multi-format data arrives.

## 7. Format-aware asset insertion while preserving old callers

Keep the existing public:

```rust
Database::add_asset(...)
```

signature working.

Its compatibility behavior:

- look up the owning book's non-empty canonical `books.format`;
- ensure/get the matching `book_formats` row;
- insert the asset linked to that `book_format_id`;
- `source_id = NULL`.

Add a new explicit API for future import/storage code, naming may vary:

```rust
add_asset_for_format(
    book_id,
    book_format_id,
    source_id,
    storage_mode,
    stored_path,
    source_path,
    size_bytes,
    stored_size_bytes,
    checksum,
    is_compressed,
    created_at
)
```

Validation:

- `book_format_id` must belong to `book_id`;
- if `source_id` is Some, it must identify an existing registered source;
- do not require a `source_books` row at this low-level asset API;
- existing `copy` / `reference` storage semantics remain unchanged.

Do not add new storage modes in this task.

## 8. Managed `Database` implementation of `LibraryBackend`

Upgrade the managed Database backend to use `book_formats` as its all-format inventory.

### `list_formats(book_id)`

- return all `book_formats` rows in deterministic ID order;
- source-neutral `LibraryFormat`;
- lowercase format;
- propagate size when known;
- metadata-only book -> empty;
- for compatibility with a malformed/pre-migration DB where no logical rows exist but `books.format` is non-empty, a one-format fallback is allowed and should be tested.

### `query_summary_page`

Do not return only the old canonical format anymore when multiple `book_formats` rows exist.

Use the new batched `list_book_formats_for_books` behavior for the page.

No N+1.

Existing primary compatibility `summary.format` stays `books.format`.

### `resolve_content_format(book_id, format)`

Resolve only assets linked to the matching logical format.

Deterministic representation preference:

1. `storage_mode == "copy"`;
2. then lowest asset ID.

Return `LibraryContent.format` from the normalized logical format.

For legacy compatibility:

- if the requested format equals `books.format` and no linked asset representation exists, preserve the existing `book.path` fallback;
- do not use an asset linked to a *different* logical format.

### `resolve_content(book_id)`

Preserve current primary-format semantics.

Prefer resolving `books.format` through the new format-aware path.

Retain only the minimum compatibility fallback needed for old DB rows/tests.

Do not change attached-Calibre backend behavior.

## 9. Delete semantics

Existing `delete_book_with_assets` must continue deleting canonical DB rows/assets without deleting physical files itself.

Ensure canonical deletion cleans:

- `source_books` mappings;
- `book_formats`;
- linked assets;

through explicit transaction cleanup or safe cascade behavior.

Do not delete rows from `library_sources` merely because their last book was removed.

Do not change CLI file-deletion flags in this task.

## 10. Tests — migration/backfill

At minimum prove:

1. an old-style DB with one book format + one asset migrates successfully;
2. one lowercase `book_formats` row is created;
3. existing asset is linked to it;
4. content bytes/path fields are unchanged;
5. existing book ID/metadata are unchanged;
6. source_id remains NULL;
7. rerunning migration does not duplicate logical formats or source rows.

## 11. Tests — provenance

At minimum prove:

1. source upsert is stable by `(kind, locator)`;
2. source metadata updates without changing ID;
3. source-book mapping persists external ID/UUID/modified;
4. repeated source-book upsert updates sync metadata but preserves `imported_at`;
5. two different sources may map different external IDs to the same canonical book;
6. duplicate external ID within one source is deterministic/upserted rather than duplicated;
7. deleting a book removes its source-book mappings but not the source registry row.

Use synthetic paths only.

## 12. Tests — formats/assets

At minimum prove:

1. one book can have PDF, EPUB, MOBI logical formats;
2. format order is insertion/ID order;
3. format identity is case-insensitive/lowercase;
4. known size is not replaced by NULL on upsert;
5. batched lookup returns correct vectors for multiple books, including empty;
6. existing `add_asset` auto-links to canonical format;
7. explicit asset insertion links EPUB and PDF assets to different logical formats;
8. format-specific resolution returns the correct distinct path per format;
9. format-specific resolution prefers `copy` over `reference` for the same logical format;
10. it never selects an asset of the wrong format;
11. removing a logical format with linked assets is rejected clearly;
12. book deletion removes linked assets/formats/provenance in DB state.

## 13. Tests — library summaries

For managed `Database` through `LibraryCatalog`:

1. a three-format book summary returns all three `LibraryFormat` values;
2. primary compatibility `format` remains the canonical `books.format`;
3. a page with multiple books uses batched format loading;
4. metadata-only book has zero formats;
5. sorting/filtering/paging totals stay unchanged.

Attached-Calibre tests remain unchanged and green.

## 14. Documentation

Update:

- `docs/project/current-status.md` only if task completion changes factual status;
- `docs/project/library-ownership-and-storage.md` only for implementation details that differ from the accepted architecture;
- relevant DB/storage roadmap if useful.

Do not rewrite unrelated old roadmaps.

## Architecture constraints

- Caliberate Database is canonical mutable catalog.
- External source identity is provenance, not runtime ownership of metadata.
- Logical format is separate from physical asset.
- Asset storage subsystem is extended, not replaced.
- Existing copy/reference behavior remains valid.
- No filesystem scanning for catalog queries.
- No Calibre process.
- No real user library in automated tests.
- No GUI product work.

## Explicit non-goals

Do **not**:

- implement Calibre -> canonical import yet;
- implement source resync yet;
- implement conflict/overlay policy;
- add archive-member storage yet;
- add new server mutation routes;
- change JSON/OPDS protocol shapes;
- change attached-Calibre backend;
- copy any real ebook files;
- mutate a Calibre source;
- access the user's real library;
- touch GUI behavior beyond compile/test adaptations.

## Expected files

Likely:

- `crates/db/src/database.rs`
- DB migration/schema helpers/tests
- `crates/library/src/catalog.rs`
- library tests
- maybe bounded asset API adaptations
- `docs/work/reports/0021.md`
- move this task to `docs/work/done/0021-canonical-provenance-formats.md`

Keep modules bounded. If `database.rs` would grow substantially, extract a focused DB module rather than adding another large block to the god file.

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

Write `docs/work/reports/0021.md` with:

- schema/version changes;
- migration/backfill behavior;
- source provenance APIs;
- logical-format APIs;
- asset linkage semantics;
- managed LibraryBackend changes;
- deletion behavior;
- tests and actual native-Windows validation;
- explicit statement that Calibre materialization/import is still the next task.

Move this task to:

`docs/work/done/0021-canonical-provenance-formats.md`

Commit and push exactly one bounded implementation branch:

`codex/0021-canonical-provenance-formats`

Do not work on any other task.
