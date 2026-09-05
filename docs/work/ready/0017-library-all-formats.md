# 0017 — Expose all formats per logical book in the library service

## Goal

Remove the current single-format compatibility limitation from the reusable library service without breaking existing callers.

A logical book may have multiple stored ebook formats. The library domain must expose all available formats and allow callers to resolve a specific format deterministically.

This task is **library-domain/backend work only**.

Do not add the HTTP/JSON API yet.
Do not redesign OPDS yet.
Do not remove the existing primary-format compatibility fields/methods.

## Current compatibility behavior

Today:

- `LibraryBook` has one `format` and one `path`;
- `LibraryBookSummary` has one `format` and one `path`;
- `LibraryBackend::resolve_content(book_id)` resolves one primary content item;
- attached Calibre selects the `data` row with the smallest `data.id` as that primary projection.

That behavior is already used by the GUI and OPDS and must remain stable.

The new all-format API is additive.

## 1. Add a source-neutral format descriptor

Add a small domain type in `caliberate-library`, for example:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryFormat {
    pub format: String,
    pub size_bytes: Option<u64>,
}
```

Exact naming is flexible.

Requirements:

- `format` is normalized lowercase;
- no filesystem path is exposed through this descriptor;
- `size_bytes` is optional because not every backend has authoritative per-format size metadata;
- do not add protocol/HTTP fields.

## 2. Extend LibraryBackend additively

Add:

```rust
fn list_formats(&self, book_id: i64) -> CoreResult<Vec<LibraryFormat>>;

fn resolve_content_format(
    &self,
    book_id: i64,
    format: &str,
) -> CoreResult<Option<LibraryContent>>;
```

and matching `LibraryCatalog` forwarding methods.

Semantics:

### list_formats

- missing book -> empty vector;
- metadata-only book -> empty vector;
- deterministic ordering;
- lowercase format names;
- no duplicate logical format names in normal operation;
- do not expose source paths.

### resolve_content_format

- exact format selection, case-insensitive at the API boundary;
- missing book -> `None`;
- unavailable requested format -> `None`;
- returned `LibraryContent.format` is normalized lowercase;
- returned `LibraryContent.path` remains internal domain locator behavior, as with existing `resolve_content`;
- no source mutation.

Keep:

```rust
resolve_content(book_id)
```

with its existing primary-content semantics.

Do not implement `resolve_content` by arbitrarily choosing alphabetical format order. Preserve existing backend-specific primary behavior.

## 3. Attached Calibre backend — expose every data row

For `CalibreLibraryBackend`:

Use Calibre base tables directly.

`list_formats(book_id)` must read the book's rows from:

```text
data(id, book, format, uncompressed_size, name)
```

Requirements:

- order by `data.id ASC`;
- lowercase `data.format`;
- use nonnegative `uncompressed_size` as `Some(size_bytes)`;
- if size is NULL/invalid/negative in a weird source, return `None` for that size rather than panic or cast to a huge unsigned number;
- metadata-only book -> empty;
- do not depend on Calibre views/functions.

If malformed source data produces duplicate format names differing only by case, return one deterministic entry per normalized format, choosing the lowest `data.id` row.

## 4. Attached Calibre format-specific content resolution

For:

```rust
resolve_content_format(book_id, requested_format)
```

on attached Calibre:

- match format case-insensitively;
- choose the lowest `data.id` matching row if malformed case-duplicates exist;
- read:
  - `books.path`;
  - `data.name`;
  - `data.format`;
- reconstruct the exact format-specific source path using the existing safe path policy;
- return:
  ```text
  storage_mode = Some("reference")
  ```
- reject unsafe `books.path`, `data.name`, or `data.format` exactly as existing primary content resolution does;
- do not open ebook contents;
- do not modify source state.

The requested format string is untrusted input. Parameterize SQL. Do not interpolate it into SQL or filesystem paths.

## 5. Preserve attached-Calibre primary projection

Existing:

```rust
resolve_content(book_id)
```

and existing `LibraryBook.format/path` behavior must still use the smallest `data.id` primary projection.

For the standard synthetic fixture where:

```text
data.id 10 = PDF
data.id 11 = EPUB
```

existing primary behavior must remain `pdf`.

The new all-format API should report:

```text
pdf
epub
```

in `data.id` order.

## 6. Configured Caliberate Database backend

The current Caliberate DB model has one canonical `books.format` and does not yet model multiple logical formats cleanly.

Do not invent format identity from arbitrary asset filenames.

For `Database`:

### list_formats

- missing book -> empty;
- non-empty `book.format` -> one normalized `LibraryFormat`;
- use an authoritative size only if the existing model can unambiguously provide one for that selected content; otherwise `size_bytes = None`;
- empty format -> empty.

### resolve_content_format

- compare requested format case-insensitively to `book.format`;
- if it matches, delegate to the existing `resolve_content(book_id)`;
- otherwise return `None`.

Document this as a managed-DB model limitation, not fake multi-format parity.

Do not change the DB schema in this task.

## 7. Fake backend / source-neutral seam proof

Update the fake non-Database backend in `catalog.rs` tests to implement the new methods.

Add an assertion proving `LibraryCatalog` delegates both:

- `list_formats`;
- `resolve_content_format`;

without knowing backend implementation details.

## 8. No N+1 expansion of existing page APIs

Do **not** change:

- `list_books`;
- `query_books`;
- `query_page`;
- `query_summary_page`;

to call `list_formats` once per book.

This task adds explicit per-book all-format APIs.

A later API/protocol task may add a batched/page-level format projection if needed.

Do not introduce hidden N+1 behavior into existing browse paths.

## 9. Tests — attached Calibre

Extend the existing synthetic Calibre fixture.

At minimum prove:

1. `list_formats(1)` returns both existing fixture formats in `data.id` order:
   ```text
   pdf
   epub
   ```
2. format names are lowercase.
3. sizes come from `uncompressed_size`.
4. `resolve_content_format(1, "pdf")` returns the PDF path.
5. `resolve_content_format(1, "EPUB")` matches case-insensitively and returns the EPUB path.
6. returned paths differ by extension/content row as expected.
7. `storage_mode == Some("reference")`.
8. missing format -> `None`.
9. missing book -> empty formats / `None` content.
10. metadata-only book -> empty formats / `None`.
11. primary `resolve_content(1)` still returns the smallest-`data.id` PDF.
12. `LibraryBook` and summary primary format/path behavior remains unchanged.
13. malformed case-duplicate formats deduplicate deterministically by lowest `data.id`.
14. unsafe format-specific `data.name` is rejected.
15. unsafe format-specific `data.format` is rejected.
16. source metadata bytes remain unchanged.

## 10. Tests — configured Database backend

At minimum prove:

1. one canonical book format is returned through `list_formats`;
2. it is lowercase;
3. matching requested format resolves existing content;
4. case-insensitive request works;
5. another format returns `None`;
6. missing book returns empty/None;
7. existing copy/reference content-selection semantics are preserved.

## 11. Performance / tracing

These are per-book operations.

Do not full-scan the Calibre library.

Use direct indexed/book-scoped queries.

If existing tracing conventions make it easy, trace unusually slow per-book format queries at the same style as current Calibre backend operations. Do not add a metrics subsystem.

## Architecture constraints

- source-neutral domain types in `caliberate-library`;
- no Axum/OPDS/GUI types in the service;
- no Calibre SQL outside the Calibre adapter;
- no DB-domain records leaked to callers;
- no source mutation;
- no Calibre process;
- no migrations against `metadata.db`;
- preserve static/UNC source open behavior from `0016.1/0016.2`.

## Explicit non-goals

Do **not**:

- change OPDS acquisition links yet;
- add HTTP/JSON routes;
- add page-level batched formats yet;
- add covers API;
- change sorting/filter semantics;
- change primary format selection;
- change the Caliberate managed DB schema;
- infer extra formats from filenames;
- touch GUI behavior;
- add write/overlay support;
- access the user's real library in automated work.

## Expected files

Likely:

- `crates/library/src/catalog.rs`
- `crates/library/src/calibre/mod.rs`
- `crates/library/src/calibre/tests.rs`
- focused existing library catalog tests
- `docs/work/reports/0017.md`
- move task to `docs/work/done/0017-library-all-formats.md`

Keep changes bounded.

## Validation

Run on native Windows:

```text
cargo fmt --check
cargo test -p caliberate-library
cargo test -p caliberate-server
cargo check --workspace --locked
cargo test --workspace --locked
```

All must pass.

## Handoff

Write `docs/work/reports/0017.md` with:

- new domain types/methods;
- attached-Calibre all-format semantics;
- configured-Database compatibility limitation;
- primary-format compatibility preservation;
- files changed;
- validation actually run;
- risks/unverified behavior;
- explicit statement that OPDS/JSON exposure is deferred.

Move this task to:

`docs/work/done/0017-library-all-formats.md`

Commit and push exactly one bounded implementation branch:

`codex/0017-library-all-formats`

Do not work on any other task.
