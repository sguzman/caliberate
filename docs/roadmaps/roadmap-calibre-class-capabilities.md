# Calibre-Class Capability Roadmap

This roadmap is the breadth map for Caliberate. `roadmap-restart-2026-09.md` controls near-term sequencing; this file prevents major Calibre-class capability families from disappearing merely because they are not immediate work.

The project target is broad practical replacement, not exhaustive historical parity.

## Priority model

- **P0 — identity/core**: required for Caliberate to make sense as a standalone primary application.
- **P1 — strong replacement**: major Calibre-class workflow that should exist for serious day-to-day use.
- **P2 — secondary depth**: useful capability that can follow after core workflows are mature.

Priority is about product importance, not implementation order. Existing partial code may make a P1 item cheaper to finish than a P0 item.

## P0 — standalone library core

### Library sources and storage

- [ ] Caliberate-managed library
- [ ] arbitrary directory-backed/reference library
- [ ] flat-directory workflow
- [ ] existing Calibre-library attach/read/index mode
- [ ] Caliberate overlay state for externally owned libraries
- [ ] multi-format logical books
- [ ] duplicate detection/merge
- [ ] rescan/reconciliation
- [ ] integrity/missing/orphan repair
- [ ] multiple libraries / switching

### Metadata and organization

- [ ] complete common metadata editing
- [ ] covers
- [ ] tags/categories
- [ ] series
- [ ] identifiers
- [ ] custom fields/columns
- [ ] bulk editing
- [ ] metadata extraction
- [ ] search/filter/sort
- [ ] saved searches
- [ ] virtual libraries/collections
- [ ] category/facet browser

### Reader

- [ ] EPUB
- [ ] HTML
- [ ] DOCX
- [ ] PDF
- [ ] later MOBI/AZW family
- [ ] native TOC/internal navigation
- [ ] search
- [ ] bookmarks
- [ ] highlights/notes
- [ ] persistent semantic reading position
- [ ] typography/reflow/page/zoom controls appropriate to format

### TTS

- [ ] generic speech engine abstraction
- [ ] native Windows voices
- [ ] voice/rate selection
- [ ] stop/pause/resume
- [ ] start from semantic position
- [ ] synchronized highlighting where backend timing permits
- [ ] robust cancellation and document/chapter transitions

### Standalone requirement

- [ ] no Calibre installation required for library management
- [ ] no Calibre installation required for supported-format reading
- [ ] no Calibre process required for existing Calibre-library attach mode
- [ ] no Calibre installation required for native Windows TTS

## P1 — strong Calibre replacement

### Conversion

- [ ] practical cross-format conversion
- [ ] format-specific settings/profiles
- [ ] batch conversion
- [ ] background jobs/progress/cancel
- [ ] metadata/cover preservation
- [ ] no final runtime dependency on Calibre conversion executables

### Devices

- [ ] Windows device discovery
- [ ] Linux device discovery
- [ ] send books
- [ ] remove/list books
- [ ] sync/reconcile
- [ ] device path/template configuration

### Content server / OPDS

- [ ] browse/search libraries
- [ ] download formats
- [ ] authentication/access control
- [ ] all library-source modes supported
- [ ] browser reading where practical
- [ ] optional mutation APIs only after consistency/auth are sound

### Background jobs

- [ ] ingest
- [ ] indexing
- [ ] metadata download
- [ ] conversion
- [ ] device operations
- [ ] news acquisition
- [ ] observable history/progress/failures
- [ ] cancellation where meaningful

### CLI

Maintain scriptable equivalents for major product domains:

- [ ] library/database management
- [ ] metadata
- [ ] conversion
- [ ] server
- [ ] device operations
- [ ] catalog/export
- [ ] metadata fetching
- [ ] reader/editor helpers where practical

The CLI is also a testability and agent-development surface, not just a power-user feature.

### Export / backup / sharing

- [ ] portable library export
- [ ] backup/restore
- [ ] arbitrary export/copy templates
- [ ] email/SMTP delivery where useful
- [ ] send/share abstraction independent of one device/provider

## P2 — secondary Calibre-class depth

### Ebook editing / polishing

- [ ] EPUB-family internal file browser
- [ ] HTML/CSS editing
- [ ] live preview
- [ ] cross-file search/replace
- [ ] resource add/remove/replace
- [ ] rename/merge/reorder internal files
- [ ] structural validation/fix tools
- [ ] compare ebook revisions
- [ ] non-destructive polish operations

### Catalog generation

- [ ] CSV/JSON/XML catalogs
- [ ] selected/search-filtered catalogs
- [ ] ebook-form catalog generation where worthwhile
- [ ] GUI + CLI access

### Plugins / extensibility

- [ ] metadata provider API
- [ ] format adapter API where safe
- [ ] device-driver API
- [ ] catalog-generator API
- [ ] news/acquisition recipe API
- [ ] explicit permissions/sandbox model

Exact Calibre plugin API compatibility is not required.

### News / web acquisition

- [ ] RSS/feed acquisition
- [ ] declarative/recipe source definitions
- [ ] webpage/article extraction
- [ ] webpage/feed -> ebook generation
- [ ] scheduled acquisition
- [ ] optional automatic library import/delivery

### Writable Calibre-library compatibility

- [ ] evaluate actual need
- [ ] compatibility fixtures across representative Calibre libraries
- [ ] safe metadata mutations
- [ ] safe format add/remove if supported
- [ ] corruption/recovery tests
- [ ] interoperability validation with Calibre itself

This remains separate from attach/read/index mode because mutating another application's live database/layout carries materially more risk.

## Explicitly not required for product identity

- pixel-for-pixel Calibre GUI imitation;
- exact parity with every Calibre plugin API;
- every legacy/obsolete format;
- every historical command-line flag;
- dependence on Calibre executables to fill missing native implementations.

## Roadmap governance

- Product breadth belongs here and in `docs/project/product-scope.md`.
- Near-term ordering belongs in `roadmap-restart-2026-09.md`.
- Detailed subsystem design belongs in architecture/subsystem roadmaps.
- A Codex work item should normally implement one narrow slice from these broader goals rather than an entire checkbox family at once.
