# 0024 — Adopt a legacy reference format into Caliberate-managed storage

## Goal

Add the first explicit progressive content offramp:

> Take one existing canonical logical format whose current bytes live in an external/reference asset, copy it into Caliberate-owned managed storage, optionally zstd-compress it using the existing asset policy, verify it, register the managed representation, and keep the legacy reference as fallback.

This task must make one imported legacy format independently serviceable from Caliberate-managed storage without deleting or mutating the original Calibre file.

Task 0023 already guarantees that a preferred zstd-managed copy is transparently served as the original logical EPUB/PDF/etc. bytes.

## Product invariant

Adoption changes **physical representation ownership**, not logical book identity or logical format identity.

Before:

```text
canonical book
  logical EPUB
    -> reference asset
       source = legacy Calibre
       ownership = external
```

After:

```text
canonical book
  logical EPUB
    -> managed copy          # preferred
       ownership = Caliberate
       encoding = identity or zstd
    -> reference asset       # retained fallback/provenance
       source = legacy Calibre
       ownership = external
```

Do not create a new book.
Do not create a duplicate logical format.
Do not delete the legacy reference.

## 1. Managed content-addressed object store

The old `LocalAssetStore` writes copied files flat by filename under `paths.library_dir`. Do **not** use that flat destination layout for adoption.

Introduce a focused managed object-store primitive in `caliberate-assets`, for example:

```text
crates/assets/src/managed.rs
```

Preferred public shape, naming may vary:

```rust
pub struct ManagedObjectStore {
    root: PathBuf,
    compress: bool,
    compression_level: i32,
}

pub struct ManagedObject {
    pub stored_path: PathBuf,
    pub logical_size_bytes: u64,
    pub stored_size_bytes: u64,
    pub checksum_sha256: String,
    pub is_compressed: bool,
    pub reused_existing: bool,
}

impl ManagedObjectStore {
    pub fn from_config(config: &ControlPlane) -> Self;
    pub fn adopt_file(&self, source: &Path, logical_format: &str) -> CoreResult<ManagedObject>;
}
```

### Storage layout

Use SHA-256 content addressing.

Required logical layout:

```text
<paths.library_dir>/
  objects/
    sha256/
      <first-two-hex>/
        <full-sha256>.<format>[.zst]
```

Example:

```text
A:\Data\Books\managed\objects\sha256\7f\7fabc...123.epub.zst
```

Requirements:

- checksum is SHA-256 of the **logical original ebook bytes**;
- lowercase logical format in the filename;
- zstd suffix only when compressed;
- no title/author-derived path component;
- no source filename collision problem;
- create parent directories as needed;
- do not expose this physical layout through JSON/OPDS.

This v1 object layout is deliberately independent of future pack/chunk storage. Later packing may move the physical representation without changing the book/format identity.

## 2. Compression policy

Use existing:

```text
config.assets.compress_raw_assets
config.assets.compression_level
```

Behavior:

- if compression disabled:
  - store original logical bytes;
  - filename `<sha>.<format>`;
  - `is_compressed = false`;
- if compression enabled:
  - zstd-compress;
  - filename `<sha>.<format>.zst`;
  - `is_compressed = true`.

Use existing compression helpers where practical.

Do not add pack/chunk/archive storage in this task.

## 3. Verification and atomicity

Adoption must never register an unverified managed representation.

### Source verification

Before writing:

- source must be an existing regular file;
- compute SHA-256 of original logical bytes;
- read size from the actual source file.

### Atomic object creation

Write a new object through a temporary file in the final object's directory, then atomically rename/replace into the final content-addressed path only after the write completes successfully.

Do not stream directly into the final filename.

If the final content-addressed object already exists:

- do not overwrite blindly;
- verify it represents the same logical SHA-256:
  - identity object -> hash stored bytes;
  - zstd object -> hash decoded bytes using existing zstd hash helper;
- if verification succeeds, reuse it;
- if verification fails, return an error and do not touch the existing object.

### Post-write verification

After creating a new object:

- verify decoded/logical SHA-256 equals source SHA-256;
- verify decoded/logical byte count equals source logical size;
- only then return success.

If verification fails:

- delete only the newly-created temporary/final object from this attempt;
- never delete the source/reference file.

## 4. Source selection from canonical DB

Add a focused library/service adoption operation rather than embedding DB + filesystem orchestration in the CLI.

Preferred module:

```text
crates/library/src/adopt.rs
```

Preferred public shape, naming may vary:

```rust
pub struct AdoptFormatRequest {
    pub book_id: i64,
    pub format: String,
    pub reference_asset_id: Option<i64>,
}

pub struct AdoptFormatResult {
    pub book_id: i64,
    pub format: String,
    pub source_asset_id: i64,
    pub managed_asset_id: i64,
    pub stored_path: PathBuf,
    pub logical_size_bytes: u64,
    pub stored_size_bytes: u64,
    pub checksum_sha256: String,
    pub is_compressed: bool,
    pub reused_existing_object: bool,
    pub already_adopted: bool,
}

pub fn adopt_format(
    db: &Database,
    store: &ManagedObjectStore,
    request: AdoptFormatRequest,
) -> CoreResult<AdoptFormatResult>;
```

Exact naming may vary.

### Logical format

- book must exist;
- requested logical format must exist in `book_formats`;
- format matching is case-insensitive and normalized lowercase.

### Candidate source reference

Candidate assets must:

- belong to that exact `book_format_id`;
- have `storage_mode == "reference"`;
- point to an existing regular file.

If `reference_asset_id` is provided:

- it must be one of those candidates;
- otherwise return a clear error.

If no asset ID is provided:

- choose the lowest asset ID reference deterministically.

Do not use `books.path` fallback as the adoption source when a canonical logical format has no linked reference asset. Adoption operates on explicit physical representations.

### Already-managed representation

Before copying:

- inspect existing assets linked to the same logical format with `storage_mode == "copy"`.

If a valid managed copy already exists:

- verify its physical file exists;
- verify its logical checksum when a checksum is recorded;
- return `already_adopted = true`;
- do not create another asset row/object.

If an existing copy row is broken/missing/corrupt:

- do not silently delete or replace it in this task;
- return a clear error.

This keeps adoption idempotent and conservative.

## 5. Register the managed asset

After the managed object has been successfully created/reused and verified:

insert one new asset linked to the existing logical format using:

```text
storage_mode = "copy"
book_format_id = existing logical format ID
source_id = NULL
stored_path = managed object path
source_path = original reference asset path
size_bytes = actual logical source size
stored_size_bytes = actual physical managed-object size
checksum = SHA-256 logical checksum
is_compressed = object encoding
```

Use `Database::add_asset_for_format` or a narrowly improved canonical API.

The old reference asset remains untouched, including its `source_id`.

Because managed content resolution already prefers `copy`, no additional preference field is required.

## 6. DB/file failure ordering

Required ordering:

1. validate DB book/format/reference candidate;
2. build + verify managed object;
3. insert managed asset row.

If DB insertion fails after this task created a brand-new object:

- remove that new object if safe;
- do not remove a pre-existing/reused content-addressed object;
- do not alter the reference asset.

A crash between object creation and DB insertion may leave an orphan content-addressed object. That is acceptable for now because a future object-store GC can reconcile unreferenced objects. Do not create a complicated journal in this task.

## 7. CLI

Add a first-class command under existing format management.

Preferred syntax:

```text
calibredb --config <CONFIG> formats adopt \
  --id <CANONICAL_BOOK_ID> \
  --format <FORMAT> \
  [--asset-id <REFERENCE_ASSET_ID>]
```

Use the config's:

```text
db.sqlite_path
paths.library_dir
assets.compress_raw_assets
assets.compression_level
```

Do not add a second `--managed-root` override in this task. One config should define the canonical DB and its managed object root consistently for CLI/server use.

Human acceptance will use a disposable/materialized config whose:

```text
db.sqlite_path = A:\Data\Books\db\caliberate.sqlite
paths.library_dir = A:\Data\Books\managed
```

### CLI output

Print at least:

```text
book_id
format
source_asset_id
managed_asset_id
stored_path
logical_size_bytes
stored_size_bytes
checksum_sha256
compressed
reused_existing_object
already_adopted
```

Human-readable is sufficient.

Do not mutate the source Calibre file.

## 8. Existing `formats add` behavior

Do not rewrite or remove existing `formats add` / `formats remove` behavior in this task.

Adoption is specifically a transition of an existing canonical format from external-only representation toward Caliberate-owned representation.

## 9. Synthetic tests — object store

Use temporary files only.

Prove:

1. identity mode stores:
   ```text
   objects/sha256/<prefix>/<sha>.<format>
   ```;
2. zstd mode stores:
   ```text
   objects/sha256/<prefix>/<sha>.<format>.zst
   ```;
3. checksum is of logical original bytes;
4. zstd decoded bytes equal source bytes;
5. logical/stored sizes are correct;
6. repeated adoption of identical bytes reuses existing verified object;
7. same filename with different content produces different object paths;
8. different source filenames with identical content+format converge to the same object path;
9. a pre-existing corrupt object at the expected path causes error rather than overwrite;
10. no temporary file remains after success;
11. source file bytes remain unchanged.

## 10. Synthetic tests — adoption

Create a canonical DB with:

- one book;
- one EPUB logical format;
- one external reference asset;
- source file containing known bytes.

Prove first adoption:

- adds exactly one managed copy asset;
- managed asset links to same `book_format_id`;
- `source_id == NULL`;
- reference asset remains;
- managed asset checksum/size/compression fields are correct;
- `LibraryCatalog::resolve_content_format` now selects the managed copy;
- reference asset remains the second/fallback representation.

Prove repeat adoption:

- returns `already_adopted = true`;
- does not duplicate managed asset row;
- does not duplicate managed object;
- does not modify source reference.

## 11. Multi-format isolation

Create one book with EPUB and PDF references.

Adopt EPUB only.

Prove:

- EPUB gains a managed copy;
- PDF remains reference-only;
- EPUB content resolution chooses managed;
- PDF content resolution still chooses reference;
- logical format inventory is unchanged.

## 12. Missing/bad source tests

Prove clear failure without DB mutation when:

- book missing;
- logical format missing;
- no reference asset exists;
- explicit `--asset-id` belongs to another format/book;
- source reference file missing;
- source reference path is a directory;
- existing managed asset row points to missing/corrupt object.

No silent fallback.

## 13. Server integration test

Using a synthetic configured Database:

1. create external reference EPUB;
2. adopt EPUB into compressed managed storage;
3. configure:
   ```text
   download_enabled = true
   download_allow_external = false
   paths.library_dir = managed object root
   ```
4. request:
   ```text
   /api/v1/books/{id}/content
   /api/v1/books/{id}/content/epub
   /opds/books/{id}/download
   ```
5. prove:
   - HTTP 200;
   - exact logical original bytes;
   - source reference remains present;
   - the request succeeds even though external-reference serving is disabled.

That last point proves the preferred content is genuinely managed.

## 14. Documentation

Update:

```text
docs/project/library-ownership-and-storage.md
docs/project/current-status.md
```

Document the new progressive-adoption state:

```text
external-only -> hybrid (managed preferred + external fallback)
```

Explicitly state that:

- source retirement is not yet allowed merely because some books are adopted;
- reference deletion/detachment is future work;
- pack/chunk representation is future work;
- content-addressed per-object storage is the current managed v1 representation.

## Architecture constraints

- logical identity is stable;
- source provenance is retained;
- adoption is additive;
- managed copy becomes preferred through existing copy-before-reference rule;
- no source mutation;
- no metadata resync;
- no reference deletion;
- physical managed store uses SHA-256 addressing, not flat filenames;
- server and protocols remain storage-agnostic.

## Explicit non-goals

Do **not**:

- delete legacy reference assets;
- delete/move Calibre files;
- detach a source;
- implement source-retirement readiness;
- implement bulk adoption;
- implement all-books migration;
- implement pack/chunk/archive storage;
- implement source resync;
- change JSON/OPDS schemas;
- change GUI product behavior;
- access the user's real library in automated tests.

## Expected files

Likely:

- `crates/assets/src/managed.rs`
- `crates/assets/src/lib.rs` or module wiring
- `crates/library/src/adopt.rs`
- `crates/library/src/lib.rs` or module wiring
- `crates/app/src/bin/calibredb.rs`
- focused tests
- docs
- `docs/work/reports/0024.md`
- move task to:
  `docs/work/done/0024-adopt-reference-format.md`

Do not grow unrelated god files substantially.

## Validation

Run on native Windows:

```text
cargo fmt --check
cargo test -p caliberate-assets
cargo test -p caliberate-db
cargo test -p caliberate-library
cargo test -p caliberate-server
cargo test -p caliberate-app --bin calibredb
cargo test -p caliberate-app --bin calibre-server
cargo check --workspace --locked
cargo test --workspace --locked
```

All must pass.

## Handoff

Write:

```text
docs/work/reports/0024.md
```

with:

- object-store layout and addressing;
- compression behavior;
- verification/atomicity behavior;
- adoption source-selection semantics;
- managed asset registration;
- idempotence;
- multi-format isolation;
- server integration evidence with external serving disabled;
- CLI syntax;
- validations actually run;
- explicit statement that real legacy adoption is pending human acceptance.

Commit and push exactly one bounded implementation branch:

```text
codex/0024-adopt-reference-format
```

Return the checkout to `main` before exit.

Do not work on any other task.
