# 0004 — Route OPDS catalog reads through `LibraryCatalog`

## Why

Task `0003` introduced the first read-only library-domain facade:

- `caliberate_library::catalog::LibraryCatalog`
- `caliberate_library::catalog::LibraryBook`

The current OPDS protocol layer still calls database catalog methods directly in `crates/server/src/opds.rs`:

- `Database::list_books()` in `opds_books`
- `Database::get_book()` in `opds_book_entry`
- `Database::search_books()` in `opds_search`

The visual-library architecture requires GUI, HTTP/JSON, OPDS, and other consumers to converge on the same library-domain service rather than each consuming database records directly.

This task is the smallest migration step: use `LibraryCatalog` for those three read operations while preserving all current OPDS behavior.

## Scope

Modify only what is necessary to make these handlers consume `LibraryCatalog`:

- `opds_books`
- `opds_book_entry`
- `opds_search`

Expected implementation shape:

1. Add `caliberate-library = { path = "../library" }` to `crates/server/Cargo.toml`.
2. In each handler, continue opening the existing `Database` exactly as today.
3. Borrow that database with `LibraryCatalog::new(&db)`.
4. Replace the direct database catalog call with the corresponding facade method:
   - `catalog.list_books()`
   - `catalog.get_book(id)`
   - `catalog.search_books(&term)`
5. Continue building the same OPDS feed/link response from the returned `LibraryBook` fields.

`LibraryBook` currently exposes the same four fields these handlers need: `id`, `title`, `format`, and `path`.

## Non-goals

Do **not**:

- redesign `ServerState`;
- keep a persistent `Database` or `LibraryCatalog` inside `ServerState`;
- introduce traits, async abstractions, dependency injection, Arc/Mutex wrappers, or a new service framework;
- add an HTTP/JSON API;
- change OPDS routes, XML structure, link relations, status codes, authentication, URL-prefix behavior, or content types;
- change `opds_book_download` except for imports required to keep compilation working;
- move asset lookup/download-path selection into the library crate;
- remove the server crate's direct `caliberate-db` dependency yet, because `opds_book_download` still uses database asset APIs;
- modify database schema or database methods;
- expand `LibraryCatalog` in this task;
- refactor unrelated OPDS helpers or formatting.

## Constraints

- This is a behavior-preserving dependency migration.
- Database opening/error handling should remain semantically the same.
- The three migrated handlers must not call `Database::list_books`, `Database::get_book`, or `Database::search_books` directly after this task.
- `opds_book_download` may continue calling `Database::get_book` and `Database::list_assets_for_book`; content resolution will receive its own later task.
- Do not convert existing warning backlog into task scope.

## Acceptance criteria

1. `crates/server` depends on `caliberate-library`.
2. `opds_books` uses `LibraryCatalog::list_books()`.
3. `opds_book_entry` uses `LibraryCatalog::get_book()` for the catalog-entry lookup.
4. `opds_search` uses `LibraryCatalog::search_books()`.
5. Those three handlers no longer call their corresponding database catalog methods directly.
6. `opds_book_download` retains existing behavior and may remain database-backed.
7. OPDS output semantics are unchanged.
8. No new SQL, schema changes, server-state redesign, or unrelated refactor is introduced.
9. Server/package tests and the full locked workspace validation pass.

## Validation

Run at minimum:

```text
cargo fmt --check
cargo test -p caliberate-server
cargo check --workspace --locked
cargo test --workspace --locked
```

If existing integration tests elsewhere exercise OPDS, they should also pass through the workspace test command. Record exact commands/results in the report.

## Repository handoff

- Move this file from `docs/work/ready/` to `docs/work/active/` when starting.
- Write `docs/work/reports/0004.md` with changes, exact validation results, and deviations/caveats.
- Move the task to `docs/work/done/` only if acceptance criteria are satisfied; otherwise move it to `docs/work/blocked/`.
- Commit all implementation + task/report state.
- Push the result to remote branch `codex/0004-opds-use-library-catalog`.
- Do not ask the human maintainer to relay the implementation or report to ChatGPT.

## Human verification

No special manual runtime verification is required for this bounded dependency migration if all existing server/workspace tests pass. The architect will inspect the pushed branch directly.
