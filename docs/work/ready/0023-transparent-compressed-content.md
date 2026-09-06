# 0023 — Transparent managed compressed-content serving

## Goal

Make a Caliberate-managed compressed physical asset remain transparently consumable as its original logical ebook format.

This is the prerequisite for progressive legacy-content adoption.

Current state:

- the canonical DB can represent multiple physical assets for one logical format;
- managed content resolution prefers `storage_mode == "copy"` over reference assets;
- `LocalAssetStore` can zstd-compress copied assets and records `is_compressed = true`;
- however `LibraryContent` currently exposes only a path/storage mode;
- the server currently opens that path and streams the physical bytes directly.

Therefore a preferred managed `.zst` copy would currently be served as zstd bytes while claiming to be EPUB/PDF/etc.

Fix that representation seam before any real legacy book is adopted.

## Product invariant

Logical content format and physical storage encoding are different concepts.

Example:

```text
logical format:
  epub

physical representation A:
  external reference
  encoding = identity

physical representation B:
  Caliberate-managed copy
  encoding = zstd
```

Consumers ask for EPUB bytes.

They must not need to know that the preferred physical representation is zstd-compressed.

Do not bake `.zst` filename conventions into protocol code.

## 1. Source-neutral content encoding

Extend the library-domain content descriptor with an explicit encoding abstraction.

Preferred shape:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryContentEncoding {
    Identity,
    Zstd,
}

pub struct LibraryContent {
    pub book_id: i64,
    pub format: String,
    pub path: String,
    pub storage_mode: Option<String>,
    pub encoding: LibraryContentEncoding,
    pub size_bytes: Option<u64>,
    pub stored_size_bytes: Option<u64>,
}
```

Exact naming may vary slightly.

Semantics:

- `format` is the logical ebook format;
- `encoding` describes how the physical bytes at `path` must be decoded;
- `size_bytes` is the logical decoded byte size when known;
- `stored_size_bytes` is physical stored size when known.

Do not expose DB `is_compressed` directly as the library-domain API if a clearer encoding enum is practical.

Do not add archive/chunk encoding variants yet.

## 2. Managed Database content resolution

For managed `Database`:

### Asset-backed content

When `resolve_content_format` selects an `AssetRow`:

- `asset.is_compressed == false` -> `Identity`;
- `asset.is_compressed == true` -> `Zstd`;
- propagate `asset.size_bytes`;
- propagate `asset.stored_size_bytes`.

Keep existing representation preference:

1. `storage_mode == "copy"`;
2. then lowest asset ID.

Do not prefer based on compression alone.

A copied compressed asset must therefore remain preferred over an earlier external reference asset for the same logical format.

### Legacy/fallback logical book path

When falling back to `books.path` without an asset row:

- encoding = `Identity`;
- size fields = None.

### Attached Calibre backend

Attached-Calibre content is always:

- encoding = `Identity`.

Propagate Calibre `uncompressed_size` as logical/stored size when it is already available cheaply in the existing content query; otherwise leave size fields None rather than adding per-book filesystem metadata work.

Do not change attached-source path safety.

## 3. Transparent HTTP streaming

Update the shared server content streamer.

Current identity behavior must remain unchanged.

### Identity

For `Identity`:

- authorize path exactly as today;
- use Tokio file streaming exactly as today;
- physical file metadata may supply content length if library-domain logical size is absent;
- existing download limits remain.

### Zstd

For `Zstd`:

- authorize the physical compressed path using the same content path policy;
- open the stored compressed file;
- stream-decompress it asynchronously;
- HTTP body must contain the original logical ebook bytes;
- Content-Type must still be based on logical `content.format`, not `.zst`;
- Content-Length should be the logical `size_bytes` when known;
- never expose the physical `.zst` path or encoding through JSON/OPDS protocol shapes.

Preferred implementation:

- use a bounded async zstd reader such as `async-compression` with Tokio support if that keeps the server path simple and non-blocking;
- add only the narrow dependency/features needed;
- use the latest compatible crate version resolved by Cargo;
- do not perform whole-file decompression into RAM;
- do not create a permanent decompressed copy.

If another existing dependency can provide correct asynchronous streaming more cleanly, use it.

## 4. Download size policy

For compressed content, the download size limit must apply to the **logical decoded size**, not merely the compressed physical file size.

Because current managed assets record `size_bytes`, use that value for preflight.

Required:

- if `size_bytes > download_max_bytes`, return 413 before streaming;
- do not permit a 1 MB compressed representation of a 1 GB logical asset through a 100 MB logical download limit.

For identity content:

- preserve current physical metadata-length behavior when no logical size is available;
- if a trustworthy logical size is available, it may be used consistently.

Do not implement a generic decompression-bomb framework beyond this known-size managed-asset case.

## 5. Stored-file existence and errors

For both encodings:

- missing physical file -> 404;
- authorization failure -> existing 403 behavior;
- invalid/corrupt zstd stream must terminate cleanly and must not panic.

If an error occurs after response streaming has begun, use normal body-stream error propagation; do not buffer the whole ebook solely to manufacture a late HTTP status.

## 6. JSON and OPDS compatibility

Do not change public JSON or OPDS response schemas/hrefs in this task.

Existing:

```text
/api/v1/books/{id}/content
/api/v1/books/{id}/content/{format}
/opds/books/{id}/download
/opds/books/{id}/download/{format}
```

must transparently return logical bytes regardless of physical encoding.

No `encoding`, `is_compressed`, or storage path should leak into wire metadata.

## 7. Tests — library domain

Add/update tests proving:

1. reference EPUB asset inserted first;
2. compressed managed/copy EPUB asset inserted later;
3. `resolve_content_format(..., "epub")` selects the managed copy;
4. returned logical format remains `epub`;
5. returned encoding is `Zstd`;
6. logical/stored sizes are propagated;
7. uncompressed copy returns `Identity`;
8. legacy book-path fallback returns `Identity`;
9. attached-Calibre content returns `Identity`.

Adapt fake backend/test descriptors explicitly rather than hiding new fields behind accidental defaults.

## 8. Tests — server byte parity

Create a synthetic configured-Database server fixture with:

- one logical EPUB;
- original uncompressed test bytes;
- one external reference representation;
- one managed zstd-compressed copy using the repository's real zstd compressor;
- the compressed copy selected as preferred.

Prove for both:

```text
/api/v1/books/{id}/content
/api/v1/books/{id}/content/epub
```

that:

- HTTP 200;
- body bytes exactly equal the original uncompressed EPUB test bytes;
- body bytes do not equal the zstd file bytes;
- Content-Type is EPUB MIME;
- Content-Length equals logical decoded size when present.

Also prove the OPDS download path still returns the original logical bytes for at least the primary route.

## 9. Tests — logical size limit

With compressed physical bytes smaller than the configured max but decoded logical size larger than max:

- response must be 413.

Then raise the limit and prove normal decoded streaming succeeds.

## 10. Tests — corrupt compressed representation

Create a managed asset marked compressed whose physical bytes are invalid zstd.

Prove:

- server does not panic;
- request/body processing terminates as an error;
- no unrelated source/reference file is mutated.

Do not silently fall back to the external reference after selecting a corrupt preferred copy in this task. Silent runtime fallback policy deserves separate design.

## 11. Existing behavior preservation

All current real/reference semantics remain:

- configured external references still require `download_allow_external`;
- attached Calibre root containment remains;
- managed copy paths still require normal managed-path authorization when external downloads are not allowed;
- format-specific selection rules remain;
- JSON/OPDS route shapes remain;
- no source mutation.

## 12. Documentation

Update:

```text
docs/project/library-ownership-and-storage.md
docs/project/current-status.md
```

only as needed to document:

- logical format vs physical encoding;
- zstd-managed representation is now transparently consumable;
- actual legacy-reference adoption is still the next task.

## Explicit non-goals

Do **not**:

- copy/adopt a real legacy book yet;
- add a migration/adopt CLI yet;
- implement pack/chunk/archive storage;
- delete reference assets;
- implement source retirement;
- implement source resync;
- change public API wire schemas;
- change GUI product behavior;
- access the user's real library in automated tests.

## Expected files

Likely:

- `crates/library/src/catalog.rs`
- bounded attached-Calibre descriptor adaptations
- `crates/server/src/content.rs`
- `crates/server/Cargo.toml`
- `Cargo.lock`
- server/library tests
- docs
- `docs/work/reports/0023.md`
- move task to:
  `docs/work/done/0023-transparent-compressed-content.md`

Keep implementation modular. Do not grow unrelated god files.

## Validation

Run on native Windows:

```text
cargo fmt --check
cargo test -p caliberate-assets
cargo test -p caliberate-library
cargo test -p caliberate-server
cargo test -p caliberate-app --bin calibre-server
cargo check --workspace --locked
cargo test --workspace --locked
```

All must pass.

## Handoff

Write:

```text
docs/work/reports/0023.md
```

with:

- content encoding domain changes;
- managed/attached resolution behavior;
- streaming implementation;
- logical download-limit behavior;
- byte-parity tests;
- corrupt-zstd behavior;
- exact dependency change if any;
- validations actually run;
- explicit statement that real legacy adoption is still pending the next task.

Commit and push exactly one bounded implementation branch:

```text
codex/0023-transparent-compressed-content
```

Return the checkout to `main` before exit.

Do not work on any other task.
