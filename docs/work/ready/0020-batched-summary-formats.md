# 0020 — Batched all-format projection in summary pages

## Goal

Eliminate the N+1 format-discovery gap exposed by real-library acceptance.

The current API can list formats for one book:

```text
GET /api/v1/books/{id}/formats
```

but discovering multi-format books across a 105,570-book corpus requires one additional HTTP request per book.

Human runtime acceptance scanned the first 5,000 real books and found no multi-format specimen. Do **not** respond by scanning the rest one-by-one.

Instead, make all stored formats available efficiently in normal paged summary results.

This is a source-neutral library-service + JSON projection task.

Do not change OPDS behavior.
Do not touch GUI behavior except compile adaptations required by the library summary type.
Do not access the user's real Calibre library in automated work.

## Desired result

A bounded JSON browse page such as:

```text
GET /api/v1/books?limit=500&offset=0
```

should return, for every summary item:

```json
{
  "id": 42,
  "title": "Example",
  "primary_format": "pdf",
  "format_count": 3,
  "formats": [
    {"format":"pdf","size_bytes":123},
    {"format":"epub","size_bytes":456},
    {"format":"mobi","size_bytes":789}
  ]
}
```

without issuing one backend query per book.

The same applies to:

```text
POST /api/v1/books/query
```

Existing primary-format fields remain compatibility fields.

## 1. Extend the library summary domain additively

Extend:

```rust
LibraryBookSummary
```

with:

```rust
pub formats: Vec<LibraryFormat>
```

where `LibraryFormat` is the source-neutral type introduced in task `0017`.

Do not remove or rename existing:

- `format`
- `path`

Those remain the primary-format compatibility projection used by existing callers.

Semantics:

- `formats` contains all logical stored formats for that book;
- deterministic source order;
- normalized lowercase names;
- metadata-only book -> empty vector;
- no filesystem paths;
- no duplicates after source normalization;
- `book.format`, when non-empty and source-consistent, remains the primary compatibility value.

## 2. No per-book format calls inside summary paging

This task exists specifically to prevent:

```text
for book in page {
    catalog.list_formats(book.id)
}
```

Do **not** implement summary formats by calling `list_formats` once per book.

Both backends must fill `LibraryBookSummary.formats` using page-level/batched behavior.

## 3. Attached Calibre backend — one bounded batch format load per summary page

For `CalibreLibraryBackend::query_summary_page`:

- obtain the page's book IDs as it does today;
- load all corresponding `data` rows in one bounded query or bounded chunked queries;
- preserve:
  ```text
  ORDER BY book, data.id
  ```
  semantics;
- normalize format names to lowercase;
- map nonnegative `uncompressed_size` to `Some(u64)`;
- invalid/negative/NULL size -> `None`;
- case-only duplicate formats -> keep lowest-`data.id` normalized format only;
- metadata-only books -> empty list.

Use only Calibre base tables.

Do not use Calibre views/functions.

Do not expose `data.name` or filesystem paths in `LibraryFormat`.

### SQLite parameter safety

Current public API page size max is 500, but keep the backend helper robust.

If an `IN (...)` implementation is used, chunk IDs conservatively so it does not depend on an unusually high SQLite variable limit.

Do not string-interpolate book IDs.

## 4. Configured Caliberate Database backend

The managed Caliberate DB still models one canonical logical format per book.

For `Database::query_summary_page` through the library adapter:

- derive `formats` directly from each already-loaded summary record's canonical `format`;
- non-empty format -> one normalized `LibraryFormat { size_bytes: None }`;
- empty format -> empty vector.

Do not perform an extra DB query per book.

Do not infer formats from arbitrary assets or filenames.

Do not change the managed DB schema.

## 5. Fake backend and all summary constructors

Update all `LibraryBookSummary` constructors/tests/fake backends to include the new field.

Keep code modular.

Do not create a generic default that silently hides missing format data if an implementation should supply it.

## 6. JSON summary DTO

Extend only the summary items returned by:

```text
GET  /api/v1/books
POST /api/v1/books/query
```

with:

```json
"format_count": 2,
"formats": [
  {
    "format": "pdf",
    "size_bytes": 123
  },
  {
    "format": "epub",
    "size_bytes": 456
  }
]
```

Important:

- these summary format entries do **not** need `content_href`;
- existing detail and `/formats` endpoints keep their current richer format DTO with `content_href`;
- do not change search DTOs in this task;
- no filesystem paths;
- `format_count` must equal `formats.length`.

The JSON API remains versioned at `/api/v1`; this is an additive field change.

## 7. Real-library discovery use case

After this task, a caller should be able to find a multi-format book without N+1 per-book requests by paging:

```text
/api/v1/books?limit=500&offset=...
```

and checking:

```text
item.format_count > 1
```

This reduces a 105,570-book discovery scan from ~105k+ HTTP requests to roughly 212 bounded page requests.

Do not add a dedicated multi-format-only route in this task.

Do not add `min_format_count` query semantics yet.

First close the batching gap cleanly.

## 8. Performance constraints

### Attached Calibre

For one `query_summary_page` request:

- existing base page query remains bounded;
- existing rich metadata batch behavior remains;
- format loading adds at most a small bounded number of page-level SQL queries;
- no query-per-book behavior.

Use tracing consistent with the existing attached backend if useful.

### Managed Database

No new query-per-book behavior.

Do not add a metrics subsystem.

## 9. Tests — attached Calibre

Extend synthetic attached fixtures to include:

- book 1: PDF primary + EPUB + MOBI;
- book 2: one format;
- book 3: metadata-only.

At minimum prove through `query_summary_page`:

1. book 1 `formats` = `pdf, epub, mobi` in `data.id` order;
2. primary `format` remains `pdf`;
3. valid sizes are exposed;
4. book 2 has exactly one format;
5. book 3 has zero formats;
6. case-only malformed duplicate is deduplicated by lowest `data.id`;
7. invalid/negative/NULL size behavior is safe;
8. summary order remains the requested query order;
9. page limit/offset/total remain unchanged;
10. source metadata bytes remain unchanged.

Also add a larger synthetic page (for example 50–100 books) and assert summary formats are returned correctly. The code structure must make clear this is batch loaded rather than repeated `list_formats` calls.

## 10. Tests — configured Database

At minimum prove:

1. summary page canonical EPUB book reports exactly one `epub` format;
2. canonical format is normalized lowercase;
3. size is `None`;
4. no formats are invented from attached/reference assets;
5. summary pagination/sort/filter behavior remains unchanged.

## 11. Tests — JSON

At minimum prove:

1. GET `/api/v1/books` item includes `format_count` and `formats`;
2. POST `/api/v1/books/query` does too;
3. `format_count == formats.length`;
4. attached synthetic multi-format item reports three formats in service order;
5. metadata-only attached item reports zero;
6. JSON still contains no filesystem/source paths;
7. existing detail/`/formats` endpoints retain their current `content_href` behavior;
8. search DTO remains unchanged;
9. default/max paging behavior remains unchanged;
10. auth/prefix behavior remains unchanged.

## 12. Documentation

Update:

```text
docs/project/http-json-api.md
```

to document that browse/query summary items now include:

- `primary_format`;
- `format_count`;
- `formats[]` with `format` and optional `size_bytes`.

State explicitly that summary format projection is batched and intended for large-library discovery without per-book format requests.

Do not document a multi-format filter that does not exist.

## Architecture constraints

- source-neutral format semantics live in `caliberate-library`;
- attached Calibre SQL stays inside its adapter;
- JSON DTOs remain protocol-owned;
- no filesystem paths in JSON;
- no N+1 summary format loading;
- no source mutation;
- no Calibre process;
- no GUI product work.

## Explicit non-goals

Do **not**:

- add `min_format_count`;
- add a dedicated multi-format discovery endpoint;
- change OPDS;
- change search semantics;
- change primary format selection;
- change the managed DB schema;
- infer formats from asset filenames;
- add covers;
- add writes;
- touch GUI behavior beyond compile/test adaptations;
- access the user's real library in automated work.

## Expected files

Likely:

- `crates/library/src/summary.rs`
- `crates/library/src/catalog.rs`
- `crates/library/src/calibre/mod.rs`
- perhaps a bounded Calibre metadata/format helper module
- `crates/library/src/calibre/tests.rs`
- `crates/server/src/api.rs`
- `crates/server/tests/api.rs`
- `docs/project/http-json-api.md`
- `docs/work/reports/0020.md`
- move this task to `docs/work/done/0020-batched-summary-formats.md`

Keep changes modular and avoid growing existing god files unnecessarily.

## Validation

Run on native Windows:

```text
cargo fmt --check
cargo test -p caliberate-library
cargo test -p caliberate-server
cargo test -p caliberate-app --bin calibre-server
cargo check --workspace --locked
cargo test --workspace --locked
```

All must pass.

## Handoff

Write `docs/work/reports/0020.md` with:

- summary-domain change;
- attached batch-loading strategy;
- configured-Database one-format limitation;
- JSON additive fields;
- evidence that summary paging does not call `list_formats` per book;
- tests/validation actually run;
- real-library batched discovery runtime still pending human acceptance.

Move the task to:

`docs/work/done/0020-batched-summary-formats.md`

Commit and push exactly one bounded implementation branch:

`codex/0020-batched-summary-formats`

Do not work on any other task.
