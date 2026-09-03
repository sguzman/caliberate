# 0006 — Route OPDS download through library content locator

## Why

Task `0005-library-content-locator` introduced `LibraryCatalog::resolve_content(book_id)`, which centralizes the existing content-selection rule used by OPDS downloads:

1. prefer the first asset whose `storage_mode` is `copy`;
2. otherwise use the first asset in database order;
3. otherwise use the logical book path;
4. return `None` when the logical book does not exist.

`crates/server/src/opds.rs` still duplicates that selection logic inside `opds_book_download` by calling `Database::get_book` and `Database::list_assets_for_book` directly. The protocol layer should consume the library-domain content locator instead of understanding asset rows.

## Scope

Modify only the OPDS download path needed to consume the existing library facade.

In `crates/server/src/opds.rs`:

- keep opening the `Database` exactly as today;
- construct `LibraryCatalog` from that database;
- replace the direct `Database::get_book(id)` + `Database::list_assets_for_book(id)` + copy/first/book-path selection block inside `opds_book_download` with `LibraryCatalog::resolve_content(id)`;
- use the returned `LibraryContent.path`, `LibraryContent.storage_mode`, and `LibraryContent.format` in the existing downstream path-policy, metadata, file-open, and MIME logic;
- preserve the existing `404 NOT_FOUND` behavior when the logical book/content does not exist;
- preserve `500 INTERNAL_SERVER_ERROR` behavior for resolver/database errors;
- preserve all existing download authorization and server policy.

Keep the server's `caliberate-db` dependency. This task does not redesign server state or database lifetime.

## Required behavior to preserve

Do not change:

- `server.download_enabled` handling;
- `is_path_allowed` semantics;
- reference/external path policy;
- `download_max_bytes` enforcement;
- filesystem metadata checks;
- file streaming behavior;
- `CONTENT_TYPE` mapping;
- `CONTENT_LENGTH` behavior;
- OPDS routes or feed structure;
- authentication middleware;
- status codes except where necessary to preserve current behavior through the facade.

## Non-goals

- Do not change `LibraryCatalog::resolve_content` or its selection rules unless a failing regression test proves a task-specific bug; if such a conflict appears, stop and report it instead of broadening scope.
- Do not add HTTP/JSON APIs.
- Do not redesign the server state to hold a long-lived database/catalog.
- Do not remove `caliberate-db` from `caliberate-server`.
- Do not add new content abstractions, traits, async layers, storage modes, MIME types, or filesystem policy.
- Do not change GUI, reader, ingest, assets, database schema, configuration, or roadmaps.
- Do not perform unrelated cleanup in `opds.rs`.

## Acceptance criteria

1. `opds_book_download` calls `LibraryCatalog::resolve_content(id)` for book/content selection.
2. `opds_book_download` no longer calls `Database::get_book(id)` or `Database::list_assets_for_book(id)` for selection.
3. Existing OPDS/server tests pass unchanged unless a focused assertion is needed to prove preserved behavior.
4. Download selection and policy behavior remain equivalent: copied assets are preferred by the library facade; reference/external policy is still enforced by the server.
5. No unrelated protocol or storage behavior changes.

## Validation

Run:

```text
cargo fmt --check
cargo test -p caliberate-server
cargo check --workspace --locked
cargo test --workspace --locked
```

If `Cargo.lock` is already current, do not regenerate it unnecessarily.

## Repository handoff

- Move this file from `docs/work/ready/` to `docs/work/active/` when starting.
- Write `docs/work/reports/0006.md` with summary, files changed, exact validation results, risks, and deviations.
- Move the task to `docs/work/done/` only when all acceptance criteria are satisfied; otherwise use `docs/work/blocked/` and explain why.
- Commit all task/code/report state.
- Push branch `codex/0006-opds-download-use-content-locator`.
- Do not require the human maintainer to deliver diffs or reports to the architect.
