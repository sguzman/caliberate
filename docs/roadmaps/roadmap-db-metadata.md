# DB & Metadata Roadmap

- [x] Inventory Calibre DB schemas in Calibre reference source for metadata, notes, and FTS
- [x] Define Rust-side schema compatibility targets and versioning
- [x] Implement SQLite connection management and busy-timeout behavior
- [x] Implement metadata schema migration runner with minimal books table
- [ ] Expand metadata schema migrations to full Calibre parity
- [x] Add metadata relation tables (authors, tags, series, identifiers, comments)
- [x] Add metadata relation APIs (authors, tags, series, identifiers, comments)
- [x] Expand basic search to include metadata relations
- [x] Implement database API (open, migrate, add, list, search)
- [x] Implement database API for book lookup and deletion
- [x] Implement database API for listing assets by book
- [x] Implement asset tracking schema and APIs for storage policies
- [ ] Implement notes store schema migrations
- [x] Implement FTS schema and indexing pipeline
- [x] Implement FTS triggers and rebuild flow
- [x] Implement FTS search API with result limits
- [ ] Implement metadata cache layer parity behaviors
- [ ] Implement search/query API compatibility surface
