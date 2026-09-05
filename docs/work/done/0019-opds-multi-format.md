# 0019 — OPDS multi-format acquisition links

## Goal

Upgrade the existing OPDS adapter to expose alternate ebook formats from the source-neutral all-format library service introduced in task `0017`, while preserving the existing primary-download OPDS behavior.

This is an OPDS protocol task only.

Do not change the JSON API.
Do not change library-domain semantics.
Do not touch the GUI.

## Existing behavior to preserve

Today a book entry exposes one primary acquisition link:

```text
GET /opds/books/{id}
  -> rel=http://opds-spec.org/acquisition
  -> href=/opds/books/{id}/download
```

and:

```text
GET /opds/books/{id}/download
```

streams `LibraryCatalog::resolve_content(id)` through the shared server content policy.

That legacy primary route and link must remain available and keep its existing semantics.

## 1. Add a format-specific OPDS download route

Add:

```text
GET /opds/books/{id}/download/{format}
```

Route behavior:

- resolve through:
  ```rust
  LibraryCatalog::resolve_content_format(id, format)
  ```
- format matching inherits the service's case-insensitive semantics;
- stream through the existing shared:
  ```text
  server::content
  ```
  policy;
- do not duplicate authorization/canonicalization/streaming logic;
- unavailable book/format/file -> 404;
- forbidden content policy -> 403;
- disabled downloads -> 403;
- over max size -> 413;
- MIME type comes from the resolved normalized format;
- stream via Tokio, never buffer the whole ebook.

The existing:

```text
GET /opds/books/{id}/download
```

must remain unchanged as the primary compatibility route.

## 2. Book-entry acquisition links

For:

```text
GET /opds/books/{id}
```

use:

- `LibraryCatalog::get_book(id)`
- `LibraryCatalog::list_formats(id)`

Preserve the existing legacy primary acquisition link exactly:

```text
href=/opds/books/{id}/download
rel=http://opds-spec.org/acquisition
type=<primary MIME>
title=Download
```

Then add format-specific acquisition links for **alternate formats only** — formats whose normalized name differs case-insensitively from `book.format`.

Each alternate link:

```text
href=/opds/books/{id}/download/{format}
rel=http://opds-spec.org/acquisition
type=<format MIME>
title=Download <UPPERCASE FORMAT>
```

Examples:

```xml
<link href="/opds/books/42/download"
      rel="http://opds-spec.org/acquisition"
      type="application/pdf"
      title="Download" />

<link href="/opds/books/42/download/epub"
      rel="http://opds-spec.org/acquisition"
      type="application/epub+zip"
      title="Download EPUB" />
```

Why alternate-only:

- preserves the exact legacy primary link;
- avoids presenting the same primary payload twice;
- still exposes every additional logical format.

If the primary format is empty or missing, preserve existing legacy behavior; do not redesign metadata-only OPDS semantics in this task.

## 3. Deterministic order

Alternate acquisition links must follow:

```text
LibraryCatalog::list_formats(id)
```

order after excluding the primary format.

For attached Calibre this means deterministic `data.id` order.

Do not alphabetically resort formats in the OPDS layer.

## 4. URL prefix

All generated OPDS hrefs must continue honoring:

```text
server.url_prefix
```

including new format-specific download links.

Do not add absolute host URLs.

## 5. Source neutrality

OPDS must continue using:

```text
ServerState::with_catalog(...)
LibraryCatalog
LibraryBackend
```

No OPDS handler may:

- open `Database` directly;
- query attached Calibre SQLite directly;
- inspect `data` rows directly;
- infer paths/formats from filenames.

Source-specific filesystem authorization remains only in the shared server content module.

## 6. Content policy

Both:

```text
/opds/books/{id}/download
/opds/books/{id}/download/{format}
```

must delegate to the same shared content streamer used by JSON.

This must preserve:

- `download_enabled`;
- `download_allow_external`;
- attached canonical-root containment;
- configured-Database reference blocking;
- `download_max_bytes`;
- MIME mapping;
- streamed bodies.

Do not fork or copy `content.rs`.

## 7. OPDS wire compatibility

Existing OPDS routes remain:

- `/opds`
- `/opds/books`
- `/opds/books/{id}`
- `/opds/books/{id}/download`
- `/opds/search`

The only additive route is:

```text
/opds/books/{id}/download/{format}
```

Existing single-format configured-Database entries should remain effectively unchanged except for implementation internals.

Do not redesign list/search feeds.

Do not add pagination in this task.

## 8. Tests — attached Calibre multi-format

Use a synthetic attached Calibre fixture with one logical book and at least:

```text
data.id 10 = PDF
data.id 11 = EPUB
```

with distinct bytes.

At minimum prove:

1. `GET /opds/books/{id}` succeeds.
2. the legacy primary acquisition link still exists:
   ```text
   /opds/books/{id}/download
   ```
3. its MIME remains PDF for the lowest-`data.id` primary fixture.
4. one alternate EPUB acquisition link exists:
   ```text
   /opds/books/{id}/download/epub
   ```
5. alternate link MIME is `application/epub+zip`.
6. alternate link title is `Download EPUB`.
7. no duplicate primary-format-specific PDF link is emitted.
8. alternate links follow service format order.
9. `GET /opds/books/{id}/download` returns the PDF bytes.
10. `GET /opds/books/{id}/download/epub` returns the EPUB bytes.
11. `GET /opds/books/{id}/download/EPUB` succeeds case-insensitively.
12. unavailable format returns 404.
13. attached format-specific download succeeds with:
    ```text
    download_allow_external=false
    ```
14. attached `metadata.db` bytes remain unchanged after representative requests.
15. configured DB path remains unused when attached source is selected, using the existing `must-not-open.db` style isolation where practical.

## 9. Tests — configured Database compatibility

At minimum prove:

1. existing single-format OPDS entry still contains its legacy primary link;
2. it does not gain a redundant format-specific duplicate for the same canonical format;
3. primary download bytes remain unchanged;
4. an unavailable alternate format returns 404;
5. configured external-reference blocking remains unchanged.

## 10. URL-prefix / auth regressions

At minimum prove:

1. with `url_prefix=/proxy`, alternate acquisition href begins:
   ```text
   /proxy/opds/books/{id}/download/...
   ```
2. existing auth middleware still protects the new format-specific route when auth is enabled.

Reuse existing server test helpers where sensible.

## 11. Documentation

Update the relevant project/API docs minimally.

Add a short OPDS note, for example in:

```text
docs/project/http-json-api.md
```

or a focused OPDS doc if one already exists, explaining:

- legacy primary acquisition route remains;
- alternate formats appear as additional acquisition links;
- format-specific OPDS route shape;
- JSON and OPDS share content policy.

Do not add a new large protocol document unless needed.

## Architecture constraints

- OPDS is a protocol adapter over the source-neutral library service.
- No source paths in OPDS.
- No direct Calibre SQL.
- No direct configured DB reads from OPDS handlers.
- Shared content authorization must remain centralized.
- No source mutations.
- No Calibre process.
- No GUI work.

## Explicit non-goals

Do **not**:

- change JSON API routes or DTOs;
- add OPDS pagination;
- redesign OPDS search/list feeds;
- change primary format selection;
- add covers;
- add writes;
- change attached-source locking/static mode;
- modify the GUI;
- access the user's real library in automated work.

## Expected files

Likely:

- `crates/server/src/http.rs`
- `crates/server/src/opds.rs`
- server OPDS/integration tests
- minimal docs
- `docs/work/reports/0019.md`
- move task to `docs/work/done/0019-opds-multi-format.md`

Keep changes bounded.

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

Write `docs/work/reports/0019.md` with:

- new route;
- legacy primary compatibility;
- alternate-link ordering/format semantics;
- shared content-policy reuse;
- configured-DB compatibility;
- attached-Calibre multi-format tests;
- auth/prefix behavior;
- validation actually run;
- explicit statement that real multi-format OPDS runtime is pending human acceptance.

Move this task to:

`docs/work/done/0019-opds-multi-format.md`

Commit and push exactly one bounded implementation branch:

`codex/0019-opds-multi-format`

Do not work on any other task.
