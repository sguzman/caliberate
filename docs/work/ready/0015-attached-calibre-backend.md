# 0015 — Add a read-only attached-Calibre library backend

## Goal

Implement a production `LibraryBackend` that can expose an existing modern Calibre library **directly from its library folder and `metadata.db`**, without importing/copying books into Caliberate and without running Calibre.

This is the first source-native backend behind the seam added by task `0014`.

The intended user flow in a later task is:

```text
calibre-server --calibre-library <existing Calibre library folder>
```

but **do not add server/source-selection CLI in this task**. This task implements and tests the backend itself.

## Non-negotiable source safety

The attached Calibre library is an external source of truth.

This backend must be read-only.

It must never:

- call `caliberate_db::Database::open*` on Calibre's `metadata.db`;
- run Caliberate migrations against the source;
- create Caliberate schema tables in the source;
- execute INSERT/UPDATE/DELETE/DDL against the source;
- rename/move/delete source ebooks;
- rewrite Calibre metadata;
- create an imported shadow copy as part of normal operation.

Use a direct read-only SQLite connection.

Open `<library_root>/metadata.db` with SQLite read-only flags and enable connection-level query-only protection.

Do not use the user's real Calibre library in automated tests.

## Canonical modern Calibre schema relevant to this adapter

The backend should target the modern Calibre schema represented by these tables/columns:

```text
books:
  id
  title
  timestamp
  pubdate
  series_index
  author_sort
  path
  uuid
  has_cover
  last_modified

data:
  id
  book
  format
  uncompressed_size
  name
  UNIQUE(book, format)

authors:
  id
  name

books_authors_link:
  id
  book
  author

tags:
  id
  name

books_tags_link:
  id
  book
  tag

series:
  id
  name

books_series_link:
  id
  book
  series

publishers:
  id
  name

books_publishers_link:
  id
  book
  publisher

ratings:
  id
  rating

books_ratings_link:
  id
  book
  rating

languages:
  id
  lang_code

books_languages_link:
  id
  book
  lang_code
  item_order

identifiers:
  id
  book
  type
  val
```

Do not depend on Calibre views that require Calibre-specific SQLite user functions such as `books_list_filter`, `concat`, or `title_sort`.

Query the base tables directly.

## Placement / modularity

Do not put this implementation into `catalog.rs`.

Add a dedicated module, preferably:

```text
crates/library/src/calibre/
    mod.rs
    query.rs        # if query construction becomes substantial
```

A single `calibre.rs` is acceptable only if it remains compact.

Avoid a new god file. If the adapter/query code is approaching several hundred lines, split source-specific query construction/path helpers into narrow modules.

Export the backend from `caliberate-library` as a library-domain capability, for example:

```rust
caliberate_library::calibre::CalibreLibraryBackend
```

## Dependency

`caliberate-library` may add a **direct** `rusqlite` dependency matching the version already used by `caliberate-db`:

```toml
rusqlite = { version = "0.39.0", features = ["bundled"] }
```

Do not introduce a different SQLite crate or version.

No other new dependency should be necessary.

## Backend construction

Provide a constructor equivalent to:

```rust
CalibreLibraryBackend::open(library_root: impl AsRef<Path>) -> CoreResult<Self>
```

The backend should retain source identity as paths/configuration, not require callers to pass a Caliberate `Database`.

At construction:

1. normalize/store the library root;
2. require `metadata.db` to exist as a file;
3. open it read-only;
4. validate the required modern Calibre tables/columns needed by this backend;
5. return a clear validation error when the folder is not a compatible Calibre library.

Do not require a Caliberate config file.

### Connection lifetime

Prefer a backend that stores source paths and opens a read-only connection per operation/helper rather than storing a long-lived `rusqlite::Connection` if that keeps the backend easy to use later in an Axum server state.

Do not add pooling in this task.

## Read-only SQLite behavior

Every connection must be opened with `SQLITE_OPEN_READ_ONLY`.

Also enable:

```sql
PRAGMA query_only = ON
```

for defense in depth.

Do not set persistent PRAGMAs.

Do not initialize/migrate schema.

Map SQLite errors through existing `CoreError` conventions with useful operation labels.

## Calibre book-file resolution

Modern Calibre stores the relative book directory in:

```text
books.path
```

and format files in:

```text
data.name
data.format
```

Resolve an attached format as:

```text
<library_root>/<books.path>/<data.name>.<lowercase(data.format)>
```

Example:

```text
books.path = "A. Author/Some Book (123)"
data.name  = "Some Book - A. Author"
data.format = "EPUB"

=> <root>/A. Author/Some Book (123)/Some Book - A. Author.epub
```

Do not recursively scan the library to discover files when the `data` table already describes them.

### Path safety

Treat database path components as untrusted source metadata.

Reject a stored relative book path or data name that would escape the configured library root through:

- absolute/rooted paths;
- parent traversal;
- platform path prefixes;
- embedded path separators in the data filename where a filename is expected.

Do not silently resolve content outside `library_root`.

Normal Unicode, spaces, punctuation, and nested relative `books.path` components must work.

## Current single-format domain projection

The existing library-domain `LibraryBook`, `LibraryBookSummary`, and `LibraryContent` expose one `format/path` value even though Calibre supports multiple formats per logical book.

Do **not** redesign those domain types in this task.

For this compatibility projection:

- select the `data` row with the smallest `data.id` for a book;
- normalize its `format` to lowercase;
- use that row consistently for `LibraryBook.format`, `LibraryBook.path`, summary `format/path`, and `resolve_content`.

If a Calibre book has no `data` row:

- the metadata book may still appear in list/query/summary results;
- use empty `format` and empty `path`;
- `resolve_content` returns `None`.

Document this as a temporary single-format compatibility projection. A later service task will expose all Calibre formats explicitly.

Do not pick a different format depending on the current query.

## Implement the full LibraryBackend read surface

Implement every method of `LibraryBackend` for `CalibreLibraryBackend` with source-native SQL.

### list_books

Return every Calibre book ordered by ID.

Populate:

- id;
- title;
- primary projected format;
- resolved primary projected content path.

Do not omit metadata-only books with no `data` row.

### get_book

Same mapping for one Calibre book ID.

Missing ID returns `None`.

### search_books

Preserve the existing broad non-FTS fallback semantics:

case-insensitive contains over:

- title;
- author name;
- tag name;
- series name.

Return distinct books ordered deterministically by ID.

Do not depend on Calibre FTS plugins/views.

### query_books / query_page

Support the complete current `LibraryQuery` surface:

Legacy optional fields:

- title
- author
- tag
- series
- publisher
- language
- identifier
- format

Structured metadata filters from task `0012`:

- Authors
- Tags
- Series
- Publishers
- Ratings
- Languages
- Include
- Exclude
- all active filters ANDed

Paging:

- limit
- offset
- full filtered total

Sorting from task `0011`:

- Id
- Title
- Authors
- Series + series_index
- Tags
- Format
- Rating
- Publisher
- Languages
- DateAdded
- DateModified
- PubDate

Preserve the established deterministic semantics:

- Authors: first alphabetically case-insensitive, missing empty;
- Tags: first alphabetically case-insensitive, missing empty;
- Series: series name case-insensitive, missing empty, then `books.series_index` in requested direction;
- Format: current primary projected format;
- Rating: numeric, missing zero;
- Publisher: missing empty;
- Languages: first by `item_order`, then link ID, missing empty;
- dates: missing empty;
- requested direction applies to semantic sort key;
- non-ID sorts finish with `books.id ASC`;
- ID sort uses requested direction on ID.

Do not accept free-form SQL from callers.

### Structured string-filter semantics

For Authors/Tags/Series/Publishers/Languages:

- case-insensitive **literal substring** matching;
- `%`, `_`, and the chosen LIKE escape character must be escaped;
- use a bound parameter and explicit SQL `ESCAPE` clause.

Include uses correlated `EXISTS`.

Exclude uses correlated `NOT EXISTS`.

Missing relations fail Include and pass Exclude.

Ratings:

- exact integer comparison;
- supported range 0..=10;
- invalid value returns the same style of deterministic validation error as the existing Database backend.

### format legacy filter

Because the current domain projection is single-format, apply the legacy `format` query field to the same primary projected `data` row, not to arbitrary secondary formats.

This keeps the projected `LibraryBook.format` and query semantics internally consistent until all-format domain support is added later.

### query_summary_page

Return `LibraryBookSummary` with:

- id
- title
- primary projected format/path
- all authors
- all tags
- optional series + `books.series_index`
- optional rating
- optional publisher
- languages in `books_languages_link.item_order`, then link ID
- `has_cover`
- timestamp -> date_added
- last_modified -> date_modified
- pubdate

Preserve the order from the structured book query.

Avoid N+1 one-query-per-book metadata loading.

Use bounded/bulk relation queries for the selected page IDs or an equivalent row-safe strategy.

### list_facets

Implement existing global facet semantics over the Calibre source for:

- Authors
- Tags
- Series
- Publishers
- Ratings
- Languages

Return:

- source table ID;
- visible name/value;
- distinct/appropriate linked-book count.

Ratings should surface the numeric rating as the facet name string.

Do not make facets filter-sensitive in this task.

### resolve_content

For an existing book:

- choose the same smallest-`data.id` primary format row used elsewhere;
- resolve the safe source path under `library_root`;
- return:
  - book_id;
  - lowercase format;
  - resolved path string;
  - `storage_mode = Some("reference")`.

For:

- missing book -> `None`;
- metadata-only book with no data row -> `None`.

Do not require the file to exist merely to construct the logical content locator; the server/content consumer can report a missing file separately.

Do not open/read ebook contents here.

## Schema validation

Validate only the base tables/columns actually required by this backend.

Do not require unrelated Calibre tables, triggers, views, custom columns, annotations, saved searches, etc.

At minimum validate the tables/columns listed in the canonical schema section above.

A database missing `data.name` should be rejected as an unsupported legacy Calibre schema rather than guessed.

Error text should identify:

- incompatible Calibre metadata schema;
- missing table/column.

## Tests — use a synthetic modern Calibre fixture

Create a temporary directory that looks like a small modern Calibre library:

```text
<tmp>/
  metadata.db
  Author A/
    Book One (1)/
      Book One - Author A.epub
      Book One - Author A.pdf
  Author B/
    Book Two (2)/
      Book Two - Author B.azw3
```

Create the minimal canonical Calibre tables directly with `rusqlite`.

Important:

- the fixture's `books` table must **not** contain Caliberate-only columns like `format` or `created_at`;
- the fixture must use Calibre's separate `data` table;
- do not create `schema_migrations`;
- this proves the adapter is not accidentally relying on Caliberate's schema.

Seed enough metadata for meaningful query/facet tests.

At minimum prove:

1. constructor accepts compatible modern fixture;
2. constructor rejects a folder without `metadata.db`;
3. constructor rejects missing required Calibre table/column;
4. list/get preserve Calibre IDs/titles;
5. primary format uses smallest `data.id` consistently across list/get/summary/content;
6. format is normalized lowercase;
7. metadata-only book remains visible but has empty format/path and no resolved content;
8. content locator resolves the expected file path under the root;
9. unsafe `books.path` traversal is rejected when resolving content;
10. unsafe `data.name` path separators/traversal are rejected;
11. broad search matches title, author, tag, and series;
12. every structured sort field works and deterministic tie behavior is preserved;
13. Include/Exclude metadata filters match task 0012 semantics;
14. literal `%` and `_` metadata filters do not become SQL wildcards;
15. multiple filters are ANDed;
16. legacy field + structured filter are both applied;
17. paging total is the full filtered total;
18. summary page contains authors/tags/series/rating/publisher/languages/dates/cover;
19. facets return correct values/counts for all six facet kinds;
20. database file contents are unchanged after backend read operations.

For source-safety coverage:

- hash/read the fixture `metadata.db` bytes before and after representative backend operations and assert equality;
- assert no Caliberate `schema_migrations` table appears;
- no test should invoke `Database::open*` on the fixture.

## Performance / SQL shape

This backend is intended for the user's entire Calibre library, not a 100-book smoke import.

Avoid:

- loading the full database merely to return one page;
- N+1 metadata queries per result row;
- broad joins that multiply logical books before paging.

Prefer:

- correlated scalar/EXISTS subqueries for filter/sort keys;
- paging logical book IDs first;
- bulk-loading page metadata by selected IDs.

Do not add indexes to the source.

Do not modify Calibre's schema.

## Tracing

Add useful tracing at backend boundaries without logging every row.

At least log/debug enough to diagnose:

- attached library root/metadata path on open;
- query page size/offset/total and elapsed time if existing tracing style makes this straightforward;
- schema incompatibility errors.

Do not log ebook contents or huge metadata payloads.

## Explicit non-goals

Do **not**:

- add `calibre-server --calibre-library` yet;
- modify `ServerState`;
- add JSON HTTP endpoints;
- modify OPDS behavior;
- wire the GUI to this backend;
- import/copy books;
- scan the source recursively;
- expose all formats yet;
- add cover download APIs;
- add write support;
- add overlay metadata;
- parse custom columns;
- parse Calibre saved searches;
- rely on Calibre Python/user-defined SQLite functions;
- run Calibre binaries;
- mutate source files;
- add connection pooling;
- add async traits;
- refactor unrelated library/DB code.

If the real modern Calibre schema requires a materially different assumption than specified here, STOP and report the mismatch rather than silently broadening scope.

## Expected files

Expected:

- `crates/library/Cargo.toml`
- `crates/library/src/lib.rs`
- new `crates/library/src/calibre/...`
- focused Calibre-backend tests, preferably beside/in that module or a dedicated integration test
- `docs/work/reports/0015.md`
- move this task to `docs/work/done/0015-attached-calibre-backend.md`

Do not change GUI/server behavior.

No lockfile change is expected if Cargo resolves the already-present rusqlite version unchanged; if Cargo legitimately changes `Cargo.lock`, document why.

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

Write `docs/work/reports/0015.md` with:

- backend architecture;
- exact read-only guarantees;
- modern Calibre schema assumptions;
- primary-format compatibility projection;
- query/filter/sort/facet semantics implemented;
- content path resolution rules;
- files changed;
- validation actually run and results;
- performance characteristics not yet measured on the user's real library;
- risks/unverified behavior;
- deviations/blockers.

Move this task to:

- `docs/work/done/0015-attached-calibre-backend.md`

Commit and push exactly one bounded implementation branch:

- `codex/0015-attached-calibre-backend`

Do not work on any other task.
