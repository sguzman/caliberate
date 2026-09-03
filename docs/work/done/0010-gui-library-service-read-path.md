# 0010 — Move visible GUI library reads onto the library service

## Goal

Move the **actual visible library browsing read path** in `caliberate-gui` onto the library-domain APIs built in tasks `0003`–`0009`.

The central table/grid/shelf book rows must be populated from `LibraryCatalog::query_summary_page`, and the left/right category browser must be populated from `LibraryCatalog::list_facets`.

This is an incremental read-path migration. Preserve the GUI's current behavior and presentation. Do not attempt to migrate the editor, detail panel, mutations, device/news flows, saved-search persistence, or every advanced filter/sort into the library service in this task.

The current GUI intentionally keeps richer local filtering/sorting for now. Pagination UI and fully server-side GUI query semantics are follow-up work.

## Existing baseline

`LibraryView::refresh_books` currently:

1. refreshes `MetadataCache`;
2. calls `Database::list_books()` or `Database::search_books()`;
3. loops every returned book through `build_row()`;
4. `build_row()` obtains metadata through `MetadataCache::get_book_details` and flattens it into `BookRow` strings;
5. `refresh_browser()` calls raw database category-count methods.

The GUI therefore bypasses the new reusable library read model for its most visible path.

The library crate now provides:

- `LibraryCatalog::search_books`
- `LibraryCatalog::query_summary_page`
- `LibraryCatalog::list_facets`
- `LibraryQuery`
- `LibraryFacetKind`
- `LibraryFacetValue`
- `LibraryBookSummary`
- `LibrarySummaryPage`

`LibraryBookSummary` already carries the structured data required by the existing `BookRow` presentation.

## Scope

### 1. Use library-domain imports for visible reads

In `crates/gui/src/views.rs`, import the library read APIs needed for this task, for example:

```rust
use caliberate_library::catalog::LibraryCatalog;
use caliberate_library::query::{LibraryFacetKind, LibraryFacetValue, LibraryQuery};
use caliberate_library::summary::LibraryBookSummary;
```

Keep existing DB imports that are still genuinely needed by details/editing/mutations/management dialogs.

Do not broadly remove `Database` from `LibraryView`; write paths and detail paths still use it in this task.

### 2. Replace visible category-browser DB DTOs

Change only the main library browser facet fields:

- `browser_authors`
- `browser_tags`
- `browser_series`
- `browser_publishers`
- `browser_ratings`
- `browser_languages`

to:

```rust
Vec<LibraryFacetValue>
```

Update the category-browser rendering helper to accept `LibraryFacetValue` rather than DB `CategoryCount`.

`ManageTagsDialogState`, `ManageSeriesDialogState`, and other management/editor code may continue using DB `CategoryCount` in this task. Do not migrate those unrelated write-oriented dialogs.

### 3. Route `refresh_browser` facet reads through `LibraryCatalog`

Within `refresh_browser`, create a short-lived `LibraryCatalog::new(&self.db)` and map exactly:

- Authors -> `LibraryFacetKind::Authors`
- Tags -> `LibraryFacetKind::Tags`
- Series -> `LibraryFacetKind::Series`
- Publishers -> `LibraryFacetKind::Publishers`
- Ratings -> `LibraryFacetKind::Ratings`
- Languages -> `LibraryFacetKind::Languages`

The six main browser category lists must no longer call these raw DB category methods directly.

Keep saved-search persistence/loading on the existing DB API for now because no library-domain saved-search service exists yet.

Use the returned facet values to populate autocomplete source lists where practical:

- tags
- languages
- publishers

without adding new DB reads merely to duplicate data already present in the facets.

### 4. Add a pure summary -> `BookRow` presentation mapping

Replace `build_row(&BookRecord)` for the main visible refresh path with a small mapping from `LibraryBookSummary` to the existing GUI `BookRow`.

Preserve the current presentation exactly:

- `authors` -> comma+space joined
- `tags` -> comma+space joined
- `series` -> `"{name} ({index})"` when present, otherwise empty
- `rating` -> decimal string when present, otherwise empty
- `publisher` -> value or empty
- `languages` -> comma+space joined
- `has_cover` unchanged
- `date_added` from library summary `date_added`, default empty
- `date_modified` from library summary `date_modified`, default empty
- `pubdate` default empty
- title/format/path/id unchanged

Keep this mapping presentation-only. Do not move comma joining or GUI formatting back into the library crate.

### 5. Load the current full GUI working set through bounded library summary pages

The GUI currently keeps `all_books` as a full in-memory working set because several existing local filters, sort modes, stats, and reader-library-search features depend on it.

Preserve that behavior in this migration, but obtain the rows through **bounded calls** to `LibraryCatalog::query_summary_page` rather than one unbounded summary query.

Use a private constant no larger than 500, for example:

```rust
const LIBRARY_SUMMARY_CHUNK_SIZE: usize = 500;
```

Load successive pages with:

- deterministic ID ascending order;
- `limit = LIBRARY_SUMMARY_CHUNK_SIZE`;
- increasing offset;
- stop when the returned page is empty or the accumulated offset reaches `page.total`.

Do **not** issue an unbounded `query_summary_page` call for the whole library. Task `0009` uses page-ID `IN (...)` queries and the GUI must keep each batch bounded.

It is acceptable that `query_summary_page` computes the total on each chunk in this first migration. Do not invent a new library API solely to optimize that in this task.

### 6. Preserve current search semantics while using the library service

`SearchScope::All` currently uses `Database::search_books(&query)`, which can use the existing FTS/simple-search behavior.

Preserve that behavior through the library facade:

```rust
LibraryCatalog::search_books(&query)
```

Do not reimplement all-fields search in GUI string matching.

A safe sequence is:

1. load the full summary working set through bounded summary pages;
2. if `SearchScope::All` has a non-empty query, obtain matching IDs through `LibraryCatalog::search_books` and retain those summary rows;
3. for Title/Authors/Tags/Series scopes, preserve the existing local field-specific matching for now.

If match ordering from `search_books` matters for stable local tie behavior, preserve it with an ID -> rank map rather than silently changing the order.

Do not change FTS/simple-search semantics in this task.

### 7. Preserve the existing local filters and sorts

After the service-backed rows are built, keep the current behavior of:

- `format_filter`
- browser include/exclude filters
- `news_only_filter`
- local primary/secondary sort modes
- grouping
- table/grid/shelf rendering
- selection
- status/search history

Do not delete `apply_filters`, `sort_rows`, or the richer GUI sort enums yet.

The purpose of this task is to migrate the **data source**, not to pretend the library query API already supports every GUI operation.

### 8. Remove `MetadataCache` from `LibraryView` if it is now unused

Before editing, search the file for all `MetadataCache`, `self.cache`, and `cache.` uses.

If the cache is used only by the old visible refresh/build-row path, remove:

- the import;
- the `cache` field;
- initialization/refresh calls;
- the obsolete `build_row` path.

If another real feature still uses it, keep only the uses that remain necessary and report them. Do not delete unrelated cache behavior blindly.

## Tests

Add compact tests in `crates/gui/src/views.rs` or a focused GUI test module.

At minimum prove:

1. `LibraryBookSummary` maps to `BookRow` with authors/tags/languages joined exactly as before;
2. series formatting preserves name + index;
3. missing optional rating/publisher/dates map to empty presentation strings;
4. the bounded summary loading helper works across multiple chunks (seed more books than a deliberately tiny test chunk size, such as 1 or 2, and verify all rows arrive once in ID order);
5. service-backed `SearchScope::All` candidate filtering preserves the IDs returned by the library search facade, if that logic is extracted into a testable helper.

Do not build snapshot/UI pixel tests in this task.

## Explicit non-goals

Do **not**:

- add GUI pagination controls yet;
- change the visible table/grid/shelf layout;
- change column definitions;
- migrate the detail panel or `BookDetails` to library-domain types;
- migrate metadata editing or mutation methods away from `Database`;
- migrate add/remove/convert/save-to-disk/device/news flows;
- migrate management dialogs;
- redesign saved searches or virtual libraries;
- remove local include/exclude browser-filter semantics;
- add new library query fields;
- add new DB SQL/schema/migrations;
- add new library service APIs solely for this task;
- change OPDS/server/HTTP behavior;
- refactor the whole `views.rs` god file;
- clean unrelated warnings.

A later task will expand service query semantics and introduce real GUI pagination without breaking advanced filtering.

## Files expected to change

Expected:

- `crates/gui/src/views.rs`
- `docs/work/reports/0010.md`
- move this task from `docs/work/ready/` to `docs/work/done/`

No dependency or lockfile change should be necessary; `caliberate-gui` already depends on `caliberate-library`.

If this task appears to require DB schema changes, new library APIs, broad GUI restructuring, or disabling existing controls, STOP and report the blocker instead of broadening scope.

## Validation

Run on native Windows:

```text
cargo fmt --check
cargo test -p caliberate-gui
cargo check -p caliberate-gui --locked
cargo check --workspace --locked
cargo test --workspace --locked
```

All must pass. Existing unrelated GUI warnings may remain.

### Runtime smoke test

Also launch the GUI on native Windows if the environment permits and verify at minimum:

- the Library view opens;
- books still appear in table mode;
- switching to grid/shelf does not panic;
- category browser counts appear;
- a simple All search still filters books;
- selecting a book still loads the existing detail panel.

If GUI launch is not possible in the Codex environment, state that explicitly in the report; do not fabricate runtime validation.

## Handoff

Write `docs/work/reports/0010.md` with:

- summary;
- exact visible read paths migrated;
- whether `MetadataCache` was fully removed from `LibraryView` and why;
- validation actually run and results;
- runtime smoke-test result or explicit inability to run it;
- risks/unverified behavior;
- deviations/blockers.

Move this task to:

- `docs/work/done/0010-gui-library-service-read-path.md`

Commit and push exactly one bounded implementation branch:

- `codex/0010-gui-library-service-read-path`

Do not work on any other task.