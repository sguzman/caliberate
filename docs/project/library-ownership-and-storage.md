# Canonical Catalog, Sources, and Storage

## Product invariant

Caliberate owns the canonical mutable catalog for the library it manages.

External systems and filesystem trees are **sources**, not the permanent catalog authority.

This means:

- Caliberate can create and maintain its own SQLite catalog at a user-selected location such as `A:\Data\Books\db\caliberate.sqlite`;
- metadata imported from an existing Calibre library is materialized into the Caliberate catalog;
- imported content may remain physically in the legacy Calibre tree as read-only references;
- newly added books may use Caliberate-owned storage;
- the same logical library may therefore contain heterogeneous physical representations;
- Caliberate remains useful for metadata/search/organization even when an external content source is temporarily unavailable.

## Core separation

```text
                   Caliberate canonical catalog
                    (mutable, user-owned DB)
                              |
               +--------------+--------------+
               |                             |
          provenance                    content locations
               |                             |
      +--------+---------+          +--------+---------+
      |                  |          |        |         |
 Calibre source    directory source |        |         |
                                  external  managed  archive/
                                  file ref   file     compressed
```

The application must distinguish three concepts that older code sometimes conflates:

1. **Logical book** — metadata identity in the Caliberate catalog.
2. **Logical format** — EPUB/PDF/MOBI/etc. belonging to that logical book.
3. **Physical representation** — where/how the bytes are stored.

## Source provenance

A source describes where imported knowledge/content originated.

Examples:

- attached Calibre library;
- arbitrary directory tree;
- future remote/import source.

A source record is not itself a library backend forever. It is provenance and synchronization state for data materialized into the canonical catalog.

A Calibre source should eventually support:

```text
attach/read
    -> materialize/import into Caliberate DB
    -> query/edit Caliberate DB normally
    -> optionally resync source changes explicitly
```

The source remains read-only by default.

## Native books

Books added directly to Caliberate are canonical native records.

They do not require an external-source mapping.

Their content may be:

- copied into managed storage;
- stored compressed;
- referenced in place;
- later stored as an archive member or another structured locator.

## Legacy Calibre import

For a legacy Calibre source:

```text
Calibre metadata.db
       |
       | import/sync metadata
       v
Caliberate catalog
       |
       +--> title/authors/tags/series/etc. are local mutable catalog data
       |
       +--> provenance remembers source + external Calibre book ID
       |
       +--> format/content records can reference the original Calibre files
```

No ebook file copy is required for reference import.

The imported Caliberate database must be queryable without reopening Calibre's `metadata.db` for ordinary catalog operations.

Actual content access may still require the legacy source path to be mounted.

## Storage ownership

Physical content storage is orthogonal to metadata provenance.

Representative states:

```text
legacy Calibre EPUB
  provenance: calibre source
  storage: external/reference file
  ownership: external/read-only

new Caliberate PDF
  provenance: native
  storage: managed file
  ownership: Caliberate

new Caliberate EPUB
  provenance: native
  storage: compressed managed representation
  ownership: Caliberate

future archived format
  provenance: native or imported
  storage: archive member
  ownership: depends on container/source
```

Consumers such as the server, reader, conversion pipeline, device sender, and TTS should ask the library/storage layer to resolve content rather than assuming a filesystem path.

Logical format is independent of physical encoding. Managed zstd-compressed
representations are decoded by the content service while being served, so
consumers receive the original logical ebook bytes. Legacy reference adoption
remains a separate explicit operation.

The first adoption operation creates the progressive hybrid state: an
external-only logical format gains a SHA-256 content-addressed managed copy
that is preferred while the external reference is retained. Adoption does not
permit source retirement or reference deletion; the current managed v1 object
layout is independent of future pack/chunk storage.

## Deletion semantics

Deleting a canonical Caliberate record and deleting underlying bytes are separate operations.

For externally owned/reference content, default removal must not delete source files.

For Caliberate-owned content, deletion may remove managed bytes when explicitly requested by the operation/policy.

External source mutation is a separate capability and must never be an accidental side effect of canonical catalog editing.

## Synchronization

Import and sync are explicit operations.

Future source sync should use persisted provenance such as:

- source ID;
- external book ID;
- external UUID when available;
- source last-modified value/fingerprint;
- imported/synced timestamps.

Local edits must not be silently overwritten by a background source query.

Conflict/overlay policy will be designed before writable synchronization is implemented.

## Existing implementation pieces to preserve

The repository already contains useful foundations:

- Caliberate-owned mutable `books` and metadata-relation tables;
- `assets` with copy/reference storage modes;
- compression/checksum/storage statistics;
- ingest by copy/reference;
- metadata editing and deletion APIs;
- source-neutral `LibraryCatalog`;
- read-only attached-Calibre backend;
- JSON and OPDS protocol adapters.

The next work should extend these pieces rather than creating a second competing database or asset subsystem.

## Near-term implementation sequence

1. Formalize source provenance and format-aware physical assets in the canonical DB.
2. Materialize an attached Calibre source into the canonical Caliberate DB without copying source files.
3. Make the managed `Database` backend expose imported multi-format reference assets natively.
4. Add explicit incremental source resync/reconciliation.
5. Expand managed storage policies, including structured archive/compressed representations.
6. Add canonical write/mutation APIs to the headless service after ownership semantics are stable.

The materialization path is now an explicit metadata operation: it reads a
Calibre source in bounded keyset pages, writes each page in one canonical DB
transaction, and records metadata-derived reference assets. It does not scan,
stat, hash, or copy ebook files. Re-running the operation resumes from
`source_books` mappings without overwriting canonical edits; source resync and
managed-storage adoption remain separate operations.

Source retirement is now measurable without reopening the source. The
read-only source audit derives `catalog_ready` exclusively from canonical
source mappings, logical formats, and owned managed-copy assets. An explicit
managed-verification pass checks only preferred Caliberate-owned files under
the configured managed root, producing the stronger `retirement_ready` result.
Source detachment/deletion remains future work; the audit never probes or
mutates legacy reference files or a source `metadata.db`.

## Non-goal

Direct attached-Calibre querying remains valuable for inspection/bootstrap/testing, but it is not the long-term canonical runtime model for a maintained Caliberate library.
