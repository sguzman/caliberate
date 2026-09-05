# 0018 — Versioned HTTP/JSON library API

## Goal

Expose the source-neutral Caliberate library service as a stable, versioned HTTP/JSON API so arbitrary sibling applications can consume either:

- the configured Caliberate database; or
- an attached read-only Calibre library

through the same server process and semantics.

This is the first general-purpose programmatic API.

OPDS remains a parallel protocol adapter and must keep its existing wire behavior.

## Versioned route namespace

Add:

```text
/api/v1
```

under the existing server router and existing optional `server.url_prefix`.

Required routes:

```text
GET  /api/v1/books
POST /api/v1/books/query
GET  /api/v1/search
GET  /api/v1/books/{id}
GET  /api/v1/books/{id}/formats
GET  /api/v1/books/{id}/content
GET  /api/v1/books/{id}/content/{format}
GET  /api/v1/facets/{kind}
```

Do not add mutations.

The existing:

```text
/health
/opds...
```

remain unchanged.

## 1. Dedicated JSON DTOs — never serialize domain structs directly

Create a bounded server module such as:

```text
crates/server/src/api.rs
```

Define explicit wire DTOs with `serde::Serialize` / `Deserialize`.

Do **not** add Serialize derives to library-domain structs merely to serve HTTP.

This keeps HTTP versioning separate from domain evolution.

### Book summary DTO

The list/query/search result item should expose useful metadata but **must not expose filesystem paths**.

Include:

- `id`
- `title`
- `primary_format`
- `authors`
- `tags`
- `series`:
  - `name`
  - `index`
- `rating`
- `publisher`
- `languages`
- `has_cover`
- `date_added`
- `date_modified`
- `pubdate`

Do not expose:

- `LibraryBook.path`
- `LibraryBookSummary.path`
- source root
- SQLite path
- `storage_mode`

### Format DTO

Expose:

- `format`
- `size_bytes`
- `content_href`

`content_href` must point to the versioned format-specific content route and honor the configured `url_prefix`.

Do not expose the source path.

### Book detail DTO

`GET /api/v1/books/{id}` should expose:

- `id`
- `title`
- `primary_format`
- `formats` from `LibraryCatalog::list_formats(id)`

It may also expose self/content hrefs.

Do not fake rich metadata here if the domain does not currently provide a direct per-ID rich-summary method. Rich metadata is already present in paged query results.

## 2. Safe bounded default browsing

### GET /api/v1/books

Use `LibraryCatalog::query_summary_page`.

Accepted query parameters:

- `limit`
- `offset`
- `sort`
- `direction`
- `title`
- `author`
- `tag`
- `series`
- `publisher`
- `language`
- `identifier`
- `format`

Map directly to existing `LibraryQuery` semantics.

Defaults:

```text
limit = 100
offset = 0
sort = id
direction = asc
```

Maximum limit:

```text
500
```

Reject:

- `limit = 0`
- `limit > 500`
- unknown sort values
- unknown direction values

with HTTP 400 JSON errors.

Do not default to dumping the complete library.

### Page response

Return:

```json
{
  "items": [...],
  "total": 123,
  "offset": 0,
  "limit": 100
}
```

Preserve service order.

## 3. Full structured query endpoint

### POST /api/v1/books/query

Accept JSON:

```json
{
  "title": null,
  "author": null,
  "tag": null,
  "series": null,
  "publisher": null,
  "language": null,
  "identifier": null,
  "format": null,
  "sort": "title",
  "direction": "asc",
  "limit": 100,
  "offset": 0,
  "metadata_filters": [
    {
      "field": "authors",
      "mode": "include",
      "value": "Tolstoy"
    },
    {
      "field": "tags",
      "mode": "exclude",
      "value": "draft"
    }
  ]
}
```

All fields may be optional except individual metadata-filter objects.

Map exactly to:

- `LibraryQuery`
- `LibrarySortField`
- `LibraryMetadataFilterField`
- `LibraryMetadataFilterMode`

Supported sort strings:

- `id`
- `title`
- `authors`
- `series`
- `tags`
- `format`
- `rating`
- `publisher`
- `languages`
- `date_added`
- `date_modified`
- `pubdate`

Supported metadata filter fields:

- `authors`
- `tags`
- `series`
- `publishers`
- `ratings`
- `languages`

Modes:

- `include`
- `exclude`

Use the same limit defaults/max as GET /books.

Unknown enum values and malformed filter objects -> HTTP 400 JSON error.

Do not reinterpret or duplicate filtering logic in the server. Translate request DTO -> `LibraryQuery`, then call `query_summary_page`.

## 4. Simple search compatibility endpoint

### GET /api/v1/search?q=<term>&limit=<n>&offset=<n>

The existing source-neutral `search_books(q)` semantics are already used by OPDS and have proven real-library behavior.

Expose them in JSON without changing them in this task.

Requirements:

- `q` required and non-empty after trimming;
- default limit 100;
- max 500;
- offset default 0;
- call `LibraryCatalog::search_books(q)`;
- compute `total` from the returned result length;
- apply offset/limit to the result before serializing;
- return a page object with:
  - `items`
  - `total`
  - `offset`
  - `limit`

Because `search_books` currently returns `LibraryBook`, search items may expose only:

- `id`
- `title`
- `primary_format`

Do not expose `path`.

**Important:** document that this endpoint currently materializes the existing simple-search result set before server-side paging. Do not claim this path is full-library-optimal. A later measured-performance task may move paging into the service without changing this HTTP contract.

Do not expand `LibraryQuery` in this task merely to optimize this endpoint.

## 5. Book detail and all formats

### GET /api/v1/books/{id}

Use:

- `LibraryCatalog::get_book(id)`
- `LibraryCatalog::list_formats(id)`

Return 404 if the book does not exist.

Return the detail DTO described above.

The existing primary format stays explicit as `primary_format`; all stored formats are in `formats`.

### GET /api/v1/books/{id}/formats

Return:

```json
{
  "book_id": 56016,
  "formats": [
    {
      "format": "epub",
      "size_bytes": 354595,
      "content_href": "/api/v1/books/56016/content/epub"
    }
  ]
}
```

If the book does not exist -> 404.

A metadata-only existing book -> 200 with an empty `formats` array.

Do not infer existence solely from `list_formats`; call `get_book` or another source-neutral existence check.

## 6. Centralize content authorization/streaming before adding JSON downloads

Current OPDS owns private helpers for:

- source-aware path authorization;
- attached-source canonical containment;
- configured-Database external-reference policy;
- actual file metadata/open;
- max-size enforcement;
- content type mapping;
- streaming.

Do **not** duplicate that logic in `api.rs`.

Extract a small server-internal module, for example:

```text
crates/server/src/content.rs
```

that owns reusable functions for:

- authorizing/canonicalizing `LibraryContent`;
- streaming authorized content;
- MIME mapping.

Then:

- existing OPDS primary download delegates to the shared helper;
- JSON primary and format-specific content routes delegate to the same helper.

OPDS status semantics and content bytes must remain unchanged.

## 7. JSON content routes

### GET /api/v1/books/{id}/content

Resolve:

```rust
catalog.resolve_content(id)
```

and stream the existing primary content through the shared server content policy.

### GET /api/v1/books/{id}/content/{format}

Resolve:

```rust
catalog.resolve_content_format(id, format)
```

and stream through the same shared server content policy.

Requirements for both:

- respect `server.download_enabled`;
- preserve attached-Calibre canonical-root containment;
- preserve configured-Database external-reference blocking;
- preserve `download_allow_external`;
- preserve `download_max_bytes`;
- stream via Tokio; do not load the whole ebook into memory;
- correct MIME from resolved normalized format;
- unavailable book/format/file -> 404;
- forbidden policy -> 403;
- too large -> 413.

The JSON API returns raw ebook bytes on these content routes, not JSON.

## 8. Facets

### GET /api/v1/facets/{kind}

Supported kinds:

- `authors`
- `tags`
- `series`
- `publishers`
- `ratings`
- `languages`

Use `LibraryCatalog::list_facets`.

Return:

```json
{
  "kind": "authors",
  "values": [
    {
      "id": 1,
      "name": "Some Author",
      "count": 12
    }
  ]
}
```

Unknown kind -> HTTP 400 JSON error.

Preserve service ordering.

## 9. Stable JSON errors

For all `/api/v1` JSON endpoints, return a consistent envelope such as:

```json
{
  "error": {
    "code": "invalid_request",
    "message": "limit must be between 1 and 500"
  }
}
```

Required codes at minimum:

- `invalid_request`
- `not_found`
- `forbidden`
- `payload_too_large`
- `internal_error`

Do not put:

- filesystem paths;
- SQLite errors;
- source roots;
- stack/debug strings

into HTTP JSON error bodies.

Log internal errors with tracing.

Raw content endpoints may return an empty status body for non-success if sharing the existing OPDS content helper makes that cleaner; JSON metadata/query endpoints must use the JSON error envelope.

## 10. Authentication and URL prefix

The API must live inside the existing authenticated router.

Therefore:

- existing bearer auth applies unchanged;
- `--disable-auth` continues to allow local development;
- configured `url_prefix` prefixes API routes exactly as it prefixes OPDS.

Generated `content_href` values must include the configured prefix.

Do not add a second auth system.

## 11. Source neutrality

Every metadata/query/format operation must go through:

```text
ServerState::with_catalog(...)
LibraryCatalog
LibraryBackend
```

No API handler may:

- open Caliberate `Database` directly;
- query Calibre SQLite directly;
- inspect Calibre schema;
- infer source type for metadata semantics.

Only the shared server content authorization layer may inspect source identity for filesystem security, as already required for OPDS.

## 12. Tests

Use synthetic sources only.

Do not access the user's real Calibre library.

At minimum prove:

### Configured Database

1. GET /api/v1/books returns JSON, defaults to bounded limit 100, and does not contain filesystem paths.
2. limit/offset and deterministic sort work.
3. invalid limit/sort/direction -> 400 JSON error.
4. POST /api/v1/books/query maps at least:
   - two metadata filters ANDed together;
   - one descending relation sort;
   - paging/total.
5. GET /api/v1/search returns expected result, total, paging, and no path.
6. GET /api/v1/books/{id} returns primary format and formats.
7. GET /api/v1/books/{id}/formats returns canonical single managed-DB format.
8. missing book -> 404 JSON.
9. facet route maps at least two kinds and rejects unknown kind.
10. configured external-reference content remains forbidden when external downloads are disabled.
11. configured managed/internal content still streams.

### Attached Calibre

12. GET /api/v1/books reads attached source, not configured DB.
13. structured POST query works against attached source.
14. detail/formats route exposes both synthetic formats in `data.id` order.
15. JSON never exposes attached source path/root.
16. format-specific EPUB request returns EPUB bytes.
17. format-specific PDF request returns PDF bytes.
18. format path matching is case-insensitive.
19. unavailable format -> 404.
20. primary content route preserves existing lowest-`data.id` primary projection.
21. attached content streams with `download_allow_external=false`.
22. source metadata bytes remain unchanged after representative API requests.

### Shared server behavior

23. OPDS download regression test still passes after content-helper extraction.
24. attached symlink/canonical containment policy test remains.
25. auth middleware still protects `/api/v1` when enabled.
26. `url_prefix` applies to API routing.
27. generated `content_href` includes `url_prefix`.
28. response Content-Type is `application/json` for JSON endpoints.

Factor synthetic Calibre fixture support instead of duplicating a giant schema if practical.

## 13. Documentation / discoverability

Add a concise server API document, for example:

```text
docs/project/http-json-api.md
```

Include:

- route table;
- request/response examples;
- sort/filter enum values;
- pagination defaults/max;
- error envelope;
- content route behavior;
- source-neutral guarantee;
- warning that simple `/search` currently materializes matches before paging;
- attached static-source safety remains controlled by server launch flags, not API consumers.

Do not generate OpenAPI in this task.

## Architecture constraints

- API DTOs are protocol-owned, not domain structs.
- Filesystem paths never appear in JSON.
- OPDS stays wire-compatible.
- Shared content policy prevents authorization drift.
- Existing library-domain semantics are reused rather than reimplemented.
- No source mutations.
- No GUI changes.
- No Calibre process.

## Explicit non-goals

Do **not**:

- add write/mutation endpoints;
- add covers endpoint yet;
- add OpenAPI/Swagger;
- add websocket/SSE;
- add filesystem browsing;
- redesign OPDS format links yet;
- optimize legacy simple search by changing the library query model;
- change attached-source locking/static mode;
- modify the GUI;
- access the user's real library in automated work.

## Expected files

Likely:

- `crates/server/src/api.rs`
- `crates/server/src/content.rs`
- `crates/server/src/http.rs`
- `crates/server/src/opds.rs`
- `crates/server/src/lib.rs`
- `crates/server/Cargo.toml` only if JSON test/serialization dependency is needed
- server integration tests/support
- `docs/project/http-json-api.md`
- `docs/work/reports/0018.md`
- move this task to `docs/work/done/0018-http-json-library-api.md`

Keep modules bounded. Do not create a new server god file.

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

Write `docs/work/reports/0018.md` with:

- API route surface;
- DTO/wire design;
- query translation;
- pagination rules;
- all-format exposure;
- shared OPDS/JSON content policy extraction;
- auth/url-prefix behavior;
- tests and actual validation results;
- known simple-search materialization limitation;
- real-library JSON runtime still pending human acceptance.

Move this task to:

`docs/work/done/0018-http-json-library-api.md`

Commit and push exactly one bounded implementation branch:

`codex/0018-http-json-library-api`

Do not work on any other task.
