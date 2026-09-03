# 0005 — Add library-domain content locator

## Why

Task `0004` moved OPDS list/get/search reads onto `LibraryCatalog`, but `opds_book_download` still knows database asset details directly.

Before migrating the download handler, the library layer needs one small read-only content-resolution API that preserves the existing storage-selection behavior without moving server authorization/policy into the library crate.

The existing OPDS download selection behavior is:

1. fetch the logical book;
2. list assets for that book;
3. prefer the first asset whose `storage_mode == "copy"`;
4. otherwise use the first asset, if any;
5. otherwise fall back to `book.path`;
6. carry the logical book's `format` for MIME/content-type handling.

This task encodes that existing behavior in the library domain. Do not redesign storage.

## Scope

Work only in the library crate and task/report state.

### `crates/library/src/catalog.rs`

Add a public DTO with this exact semantic shape (field names may match exactly unless Rust requires a small adjustment):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryContent {
    pub book_id: i64,
    pub format: String,
    pub path: String,
    pub storage_mode: Option<String>,
}
```

Add this method to `LibraryCatalog`:

```rust
pub fn resolve_content(&self, book_id: i64) -> CoreResult<Option<LibraryContent>>
```

Required behavior:

- Call the existing database APIs; do not add SQL in `caliberate-library`.
- If the book does not exist, return `Ok(None)`.
- For an existing book, call `Database::list_assets_for_book(book_id)`.
- If one or more assets have `storage_mode == "copy"`, choose the first such asset in database result order.
- Otherwise, if any asset exists, choose the first asset in database result order.
- For an asset-backed result:
  - `book_id` is the logical book id;
  - `format` is the logical book format;
  - `path` is `asset.stored_path`;
  - `storage_mode` is `Some(asset.storage_mode)`.
- If no asset exists, fall back to the logical book record:
  - `path` is `book.path`;
  - `storage_mode` is `None`.

Keep the existing `LibraryBook`, `list_books`, `get_book`, and `search_books` behavior unchanged.

### Tests

Add focused unit tests in the library crate using temporary databases and existing DB helper methods such as `add_book`, `add_asset`, and `list_assets_for_book` as needed.

Tests must explicitly prove:

1. copied asset is preferred even when a non-copy/reference asset was inserted first;
2. first asset is used when there is no copied asset;
3. logical `book.path` is used when there are no assets;
4. missing book returns `None`;
5. returned `format` comes from the logical book.

Use platform-neutral temporary/test paths. Do not require real ebook files on disk; this is metadata resolution only.

## Non-goals

- Do not change `crates/server` in this task.
- Do not migrate `opds_book_download` yet.
- Do not move download authorization, `download_allow_external`, library-root containment, max-size checks, MIME mapping, or HTTP behavior into `caliberate-library`.
- Do not add filesystem existence checks.
- Do not add new SQL to the library crate.
- Do not redesign the asset schema.
- Do not implement multi-format logical books in this task.
- Do not introduce traits, async APIs, source adapters, or a new service framework.
- Do not change GUI code.

## Acceptance criteria

1. `LibraryContent` exists as a library-domain DTO with book id, format, path, and optional storage mode.
2. `LibraryCatalog::resolve_content` preserves the current OPDS asset-selection rule exactly.
3. No database-layer `AssetRow` or `BookRecord` is exposed in the public return type.
4. The five behaviors listed under Tests are covered.
5. Existing library behavior and workspace tests remain green.
6. No server behavior changes occur in this task.

## Validation

Run exactly:

```text
cargo fmt --check
cargo test -p caliberate-library
cargo check --workspace --locked
cargo test --workspace --locked
```

If adding the already-present `caliberate-db` dependency requires no lockfile change, do not touch `Cargo.lock` unnecessarily.

## Repository handoff

- Move this file from `docs/work/ready/` to `docs/work/active/` when starting.
- Write `docs/work/reports/0005.md` containing summary, files changed, exact validation results, risks/caveats, and deviations.
- Move the task to `docs/work/done/` only if every acceptance criterion passes; otherwise move it to `docs/work/blocked/` and explain why.
- Commit all task/code/report state.
- Push branch exactly:

```text
codex/0005-library-content-locator
```

Do not ask the human maintainer to relay the patch or report to the architect. The architect will inspect the pushed branch directly.
