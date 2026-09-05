# 0016 — Serve an attached Calibre library headlessly

## Goal

Wire the read-only `CalibreLibraryBackend` from task `0015` into `calibre-server` so Caliberate can run headlessly against an existing Calibre library folder.

After this task, the intended server launch is:

```text
calibre-server --calibre-library <existing Calibre library folder>
```

The server must serve that source through the existing OPDS endpoints without importing/copying the library into Caliberate's own database and without running Calibre.

This task is server/source selection only.

Do not add the JSON API yet.

## Required behavior

### 1. Add explicit server library source state

Introduce a small server-domain source selector, for example:

```rust
pub enum ServerLibrarySource {
    ConfiguredDatabase,
    AttachedCalibre(CalibreLibraryBackend),
}
```

Exact naming is flexible.

`ServerState` must carry:

- the existing `ControlPlane`;
- the selected library source.

Preserve a clean constructor for the existing default configured-Database source so tests/callers do not need to know implementation details.

The attached variant may store the already-opened/validated `CalibreLibraryBackend`.

Do not store a `rusqlite::Connection` in Axum state.

### 2. Centralize LibraryCatalog access

Current OPDS handlers each do:

```rust
Database::open_with_fts(...)
LibraryCatalog::new(&db)
```

Remove that duplication.

Add one bounded server-state/source helper equivalent to:

```rust
state.with_catalog(|catalog| ...)
```

or another clear source-neutral seam.

Semantics:

- ConfiguredDatabase:
  - open the existing Caliberate Database using current config/FTS behavior;
  - create `LibraryCatalog::new(&db)`;
  - invoke the requested operation.

- AttachedCalibre:
  - create `LibraryCatalog::new(&attached_backend)`;
  - do not open Caliberate's Database at all.

All OPDS library operations must use this seam:

- books list;
- one book entry;
- search;
- content resolution/download.

The OPDS layer must not branch on Calibre schema details.

### 3. Preserve existing default server behavior

Existing:

```rust
caliberate_server::run(&config)
```

must continue serving the configured Caliberate Database exactly as before.

If a new entry point is needed, prefer an additive shape such as:

```rust
run_with_source(&config, source)
```

while keeping `run(&config)` as the default-Database convenience.

Do not break existing callers.

### 4. Add `--calibre-library <PATH>` to the calibre-server binary

Add:

```text
--calibre-library <PATH>
```

to `crates/app/src/bin/calibre-server.rs`.

When launching the server with this flag:

1. open/validate `CalibreLibraryBackend` before binding the listening socket;
2. fail clearly if the path is not a compatible Calibre library;
3. select the attached-Calibre server source;
4. do not modify `config.db.sqlite_path`;
5. do not create/import a Caliberate DB for the attached source;
6. run the same HTTP/OPDS server with that source.

Without the flag, keep the existing configured Database source.

### 5. CheckConfig must validate the attached source

For:

```text
calibre-server --calibre-library <PATH> check-config
```

validate both:

- normal server/config parsing;
- the attached Calibre library root/schema.

Return success only if the Calibre source can be opened by `CalibreLibraryBackend`.

Do not bind a server socket in `check-config`.

Do not touch the source.

### 6. Preserve client-style subcommands

Existing client commands such as:

- `health`
- `opds-root`
- `opds-books`
- `opds-search`
- `download`

remain HTTP clients to an already-running server.

Do not make these commands directly read the local `--calibre-library` path.

Document in help/report that `--calibre-library` selects the source for server launch and source validation in `check-config`; it does not change which remote server the client subcommands contact.

### 7. Attached Calibre download authorization

Task `0015` returns attached content as:

```text
storage_mode = "reference"
```

The existing server rejects all `reference` content unless:

```text
download_allow_external = true
```

That policy is correct for arbitrary external references in the normal Caliberate DB, but wrong for the selected attached Calibre source.

Required behavior:

- For `ConfiguredDatabase`:
  - preserve current behavior;
  - arbitrary `reference` content remains forbidden when `download_allow_external = false`;
  - managed/internal content under configured library dir remains allowed.

- For `AttachedCalibre`:
  - a content path resolved by that backend under its canonical `library_root()` is a trusted source path;
  - allow its download when `download_enabled = true` even if `download_allow_external = false`;
  - do not globally flip `download_allow_external`;
  - do not allow paths outside the attached root.

Keep the policy source-aware and centralized.

### 8. Protect against symlink/path escape at streaming time

The attached backend validates the logical locator lexically under its canonical source root.

Before streaming an existing file:

1. canonicalize the actual content file path;
2. for AttachedCalibre, require the canonical file path to remain under the canonical attached root;
3. then open/stream that canonical path.

This prevents an in-library symlink from causing the server to stream a target outside the selected library root.

For ConfiguredDatabase, preserve existing external-reference semantics. Do not broaden access.

If the resolved file does not exist/cannot canonicalize, return the existing not-found behavior.

### 9. Source identity logging

At server startup, log which source is active without dumping large metadata:

```text
configured database
```

or:

```text
attached Calibre library root=<...>
```

Do not log book contents or entire catalogs.

## OPDS semantics

Do not redesign OPDS in this task.

Existing endpoints remain:

```text
/health
/opds
/opds/books
/opds/books/{id}
/opds/books/{id}/download
/opds/search?q=...
```

The only change is which `LibraryBackend` supplies them.

Do not add OPDS pagination yet.

Do not change feed XML shape.

Do not add JSON routes yet.

## Tests

Use synthetic temporary Calibre fixtures only.

Do not access the user's real Calibre library.

At minimum prove:

1. existing configured-Database OPDS books test still passes through the new source seam;
2. existing configured-Database download test still passes;
3. existing configured-Database external-reference blocking test still returns FORBIDDEN;
4. attached-Calibre source can serve `/opds/books` and the feed contains the attached fixture title;
5. attached-Calibre `/opds/search?q=...` searches the attached source;
6. attached-Calibre `/opds/books/{id}` returns the attached title;
7. attached-Calibre download returns the actual fixture bytes when:
   - `download_enabled = true`;
   - `download_allow_external = false`;
8. attached-Calibre server source does not read books from the configured Caliberate DB;
   seed a distinct DB book and assert attached responses contain only the Calibre fixture book;
9. attached-Calibre metadata.db bytes are unchanged after representative HTTP requests;
10. invalid attached-Calibre root is rejected before server launch/source construction;
11. CLI parsing accepts `--calibre-library <PATH>`;
12. `check-config` source-resolution helper validates a synthetic compatible Calibre source and rejects an incompatible one without binding a socket;
13. source-aware path authorization rejects a path outside the attached root;
14. a symlink inside the attached root that points outside is not streamed.
    - If symlink creation is not available on the current Windows test environment, isolate the canonical-path policy in a pure/helper test and document the platform limitation rather than making the entire suite privileged.

Keep test fixtures compact.

A minimal Calibre fixture still needs every table/column required by `CalibreLibraryBackend::open`.

Factor that fixture helper sensibly rather than pasting a huge schema into multiple tests.

## Architecture constraints

- GUI untouched.
- No changes to `LibraryBackend` semantics unless a compile blocker is discovered.
- No Calibre schema knowledge in OPDS handlers.
- No Caliberate migration/open call against Calibre `metadata.db`.
- No source import.
- No source mutation.
- No server-global mutable singleton.
- No async trait conversion.
- No connection pool.
- No new dependency unless clearly required; none is expected.

## Explicit non-goals

Do **not**:

- add HTTP JSON API;
- expose all Calibre formats yet;
- add OPDS pagination;
- change query/filter/sort semantics;
- wire the desktop GUI to attached Calibre;
- add directory-scanning source mode;
- add write/overlay support;
- add filesystem watchers;
- import Calibre books;
- mutate Calibre metadata;
- run Calibre executables;
- optimize full-library OPDS feed size yet;
- access the user's real library in automated work.

## Expected files

Likely:

- `crates/server/src/lib.rs`
- `crates/server/src/opds.rs`
- optionally one small `crates/server/src/library_source.rs` if it keeps source policy clean
- `crates/server/tests/opds.rs` and/or focused source tests
- `crates/app/src/bin/calibre-server.rs`
- focused binary/unit tests
- `docs/work/reports/0016.md`
- move this task to `docs/work/done/0016-headless-attached-calibre-server.md`

Do not modify GUI files.

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

Write `docs/work/reports/0016.md` with:

- server source architecture;
- exact CLI behavior;
- default Database compatibility;
- attached-Calibre download authorization semantics;
- canonical-path streaming safety;
- files changed;
- validation actually run and results;
- runtime behavior not yet tested against the user's real library;
- risks/unverified behavior;
- deviations/blockers.

Move this task to:

- `docs/work/done/0016-headless-attached-calibre-server.md`

Commit and push exactly one bounded implementation branch:

- `codex/0016-headless-attached-calibre-server`

Do not work on any other task.
