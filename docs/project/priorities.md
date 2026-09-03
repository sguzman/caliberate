# Product Priorities

This file defines current priority. It is intentionally narrower than `docs/project/product-scope.md`, which records long-term breadth.

## P0 — Visual library platform

The immediate product is a **Calibre-like visual library database and service platform** that can be reused by other projects.

### Desktop visual experience

The GUI should recreate the useful spatial model of Calibre's main library window while remaining native egui rather than imitating Qt internals.

P0 visual elements:

- primary action toolbar;
- always-available library search bar plus advanced search construction;
- left category/tag browser for authors, tags, series, publishers, formats, ratings, custom fields, etc.;
- central book collection that can switch between detailed table/list and visual cover-oriented modes;
- cover grid as a first-class browsing mode;
- cover browser / single-row visual browser where useful;
- right-side book details/cover/metadata panel;
- virtual libraries / saved restrictions;
- layout toggles and persistent pane/view state;
- fast selection, sorting, filtering, keyboard navigation, and context actions;
- visually useful empty/loading/error states rather than debug-shell presentation.

The target is recognizably Calibre in information architecture and workflow, not necessarily pixel-perfect theming.

### Library/query core

The GUI must not become the primary owner of library semantics. Build a reusable core that other frontends/projects can consume.

P0 capabilities:

- managed, arbitrary-directory/reference, and attached-Calibre library sources;
- logical books with multiple formats;
- metadata and covers;
- category/facet enumeration;
- rich search/filter/sort;
- saved searches / virtual libraries;
- deterministic pagination/range queries for large libraries;
- content/format lookup and stream/path resolution;
- rescan/reconciliation/integrity operations;
- explicit read APIs that do not expose raw SQLite assumptions to consumers.

### Service boundary and protocol adapters

The library/query/content service is the product core. Protocols are adapters.

P0 adapter targets:

1. **in-process Rust API** for Caliberate GUI and other Rust projects;
2. **HTTP/JSON API** for generic local/network consumers;
3. **OPDS** for ebook clients, building on the current server;
4. room for additional adapters without changing library semantics.

Potential later adapters include WebDAV or other protocols when an actual consuming project requires them. Do not implement protocols merely for checkbox parity.

Protocol handlers must not independently reimplement database queries, search rules, or path-selection logic. They should call the same library service used elsewhere.

## P1 — Reader, TTS, conversion, metadata depth

These remain important and should build on the P0 library platform rather than block it.

- real EPUB/HTML/DOCX/PDF reader pipeline;
- normalized document model;
- Windows native TTS and synchronized highlighting;
- deeper metadata editing/download/embedding;
- practical cross-format conversion;
- annotations/reading state persistence;
- stronger CLI coverage over the same library service.

## P2 — Secondary Calibre utilities

Useful, but not allowed to distract from visual library/database/service work:

- device integration;
- ebook editor/polishing;
- news/feed acquisition;
- email/SMTP delivery;
- plugin ecosystem;
- catalog-generation depth beyond simple exports;
- broad writable compatibility with Calibre's live library schema.

Existing code in these areas should be preserved and kept buildable, but new feature investment is low priority unless it directly enables P0/P1 work.

## Priority test

When choosing between two tasks, prefer the task that makes Caliberate better at one of these questions:

1. Can I visually browse/search/manage a large ebook library like I can in Calibre?
2. Can another program consume the same library/query/content model without scraping GUI state or talking directly to SQLite?
3. Can the same library be served through useful interfaces without duplicating business logic?
4. Can the user operate on arbitrary directories and existing Calibre libraries with Calibre absent?

If a task does not materially advance one of those questions, it is probably not P0.
