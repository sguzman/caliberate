# 0003 — Introduce read-only library catalog facade

## Why

Caliberate's current database already exposes useful read primitives such as `Database::list_books`, `Database::get_book`, and `Database::search_books`, but callers consume `caliberate_db::BookRecord` directly.

The current P0 architecture requires a reusable library/query core that can later serve the egui GUI, HTTP/JSON, OPDS, and other consumers without each protocol/view owning its own book-query semantics.

This task creates the **smallest read-only library-domain seam** over the existing database. It must not redesign searching, storage, server behavior, or the GUI.

## Scope

Work only in the library crate and its manifest/tests.

1. Add `caliberate-db` as a path dependency of `crates/library`.
2. Add a new module:

```text
crates/library/src/catalog.rs
```

3. Re-export/expose it from `crates/library/src/lib.rs`.
4. Define a library-domain DTO named `LibraryBook` with exactly these fields for this task:

```rust
pub struct LibraryBook {
    pub id: i64,
    pub title: String,
    pub format: String,
    pub path: String,
}
```

Derive traits that are useful for tests/debugging, including at least `Debug`, `Clone`, `PartialEq`, and `Eq`.

5. Define a borrowed read-only facade:

```rust
pub struct LibraryCatalog<'a> {
    db: &'a caliberate_db::database::Database,
}
```

It should have a constructor and these methods:

```rust
pub fn new(db: &'a Database) -> Self;
pub fn list_books(&self) -> CoreResult<Vec<LibraryBook>>;
pub fn get_book(&self, id: i64) -> CoreResult<Option<LibraryBook>>;
pub fn search_books(&self, query: &str) -> CoreResult<Vec<LibraryBook>>;
```

Use the existing database methods; do not duplicate their SQL in `crates/library`.

6. Keep conversion from `caliberate_db::BookRecord` to `LibraryBook` inside the library crate. A simple private conversion helper or `From<BookRecord>` implementation is acceptable.

7. Add focused tests in the library crate using a temporary SQLite database created with the existing `Database::open_path` API. Seed records with the existing `Database::add_book` API.

Tests must cover at least:

- listing multiple books through `LibraryCatalog`;
- fetching one existing book and one missing ID;
- searching through `LibraryCatalog` and receiving library-domain DTOs.

## Non-goals

- Do not change `crates/db` behavior or schema.
- Do not add new SQL queries.
- Do not change `BookRecord`.
- Do not introduce a generic repository trait yet.
- Do not add async abstractions, connection pools, `Arc`, locks, or caching.
- Do not refactor OPDS/server code yet.
- Do not refactor GUI code yet.
- Do not implement structured/faceted queries yet.
- Do not implement directory-backed or Calibre-library source adapters yet.
- Do not change ingest/storage semantics.
- Do not add pagination/sorting abstractions yet.
- Do not touch `config/control-plane.toml`.
- Do not perform unrelated warning cleanup or formatting churn.

## Constraints

- This is deliberately a thin facade over existing behavior.
- `LibraryBook` is a library-domain type; consumers of the facade should not need to import `caliberate_db::BookRecord`.
- The facade borrows an already-open `Database`; this task does not decide database lifetime/connection pooling for servers.
- Preserve existing database result ordering and search semantics exactly.
- Do not leak GUI, HTTP, OPDS, or platform types into the library crate.

## Acceptance criteria

1. `crates/library` depends on `caliberate-db` and builds without a dependency cycle.
2. `LibraryBook` exists in the library domain with the exact four fields above.
3. `LibraryCatalog::list_books` delegates to `Database::list_books` and returns `Vec<LibraryBook>`.
4. `LibraryCatalog::get_book` delegates to `Database::get_book` and preserves `Some`/`None` behavior.
5. `LibraryCatalog::search_books` delegates to `Database::search_books` and returns `Vec<LibraryBook>`.
6. No SQL is added to `crates/library`.
7. Focused facade tests pass.
8. Full workspace formatting/check/tests pass.
9. No server, GUI, database-schema, or config behavior changes.

## Validation

Run at minimum:

```text
cargo fmt --check
cargo test -p caliberate-library
cargo check --workspace --locked
cargo test --workspace --locked
```

Record exact commands and results in the report.

## Repository handoff

- Move this file from `docs/work/ready/` to `docs/work/active/` when starting.
- Write `docs/work/reports/0003.md`.
- Move the task to `docs/work/done/` only if all acceptance criteria are satisfied; otherwise move it to `docs/work/blocked/` and explain the blocker.
- Commit all code + task/report state.
- Push to remote branch:

```text
codex/0003-library-catalog-facade
```

- Do not ask the human maintainer to relay the patch/report to ChatGPT. The architect will inspect the pushed branch directly.

## Human verification

None required for this task beyond the automated/local Rust validation. This is a non-UI, non-device, non-TTS domain seam.
