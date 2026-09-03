# Calibre-Class Capability Roadmap

This is the breadth map for Caliberate. `docs/project/priorities.md` controls current priority, and `docs/roadmaps/roadmap-visual-library-platform.md` controls the current P0 execution direction.

The project target is broad practical replacement, not exhaustive historical parity.

## Priority model

- **P0 — visual library platform**: the current product identity and infrastructure priority.
- **P1 — reader/content depth**: important capabilities built on the P0 library platform.
- **P2 — secondary Calibre utilities**: useful long-term parity areas that must not distract from P0/P1.

## P0 — visual library platform

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

### Reusable library/query/content service

- [ ] frontend-independent library descriptors
- [ ] `BookSummary`-style collection data
- [ ] detailed book metadata model
- [ ] format/content descriptors
- [ ] shared query/filter/sort/pagination semantics
- [ ] category/facet enumeration
- [ ] saved searches / virtual libraries
- [ ] cover/content resolution
- [ ] read-only service used by GUI and servers
- [ ] later controlled mutation surface

### Calibre-like visual desktop GUI

- [ ] primary action toolbar
- [ ] global search and advanced search
- [ ] left Tag/category browser
- [ ] detailed list/table mode
- [ ] cover grid
- [ ] cover browser / visual browse mode
- [ ] right Book details panel
- [ ] virtual library controls
- [ ] persistent layout/view state
- [ ] sorting/filtering/multi-select/keyboard/context actions
- [ ] large-library lazy cover/details loading

### Search and organization

- [ ] common metadata fields
- [ ] covers
- [ ] tags/categories
- [ ] series
- [ ] identifiers
- [ ] custom fields/columns
- [ ] search/filter/sort
- [ ] saved searches
- [ ] virtual libraries/collections
- [ ] category/facet browser
- [ ] deterministic pagination/range queries
- [ ] large-library performance tests

### Service/protocol adapters

- [ ] in-process Rust API
- [ ] HTTP/JSON API
- [ ] OPDS browse/search/acquisition
- [ ] shared semantics across GUI/JSON/OPDS
- [ ] authentication/access control where network-facing
- [ ] streaming covers/book content
- [ ] capability/version discovery
- [ ] additional protocols only when real consumers require them

### Standalone requirement

- [ ] no Calibre installation required for library management
- [ ] no Calibre process required for attached Calibre-library read/index mode
- [ ] protocol/API consumers do not depend on Calibre executables
- [ ] generic consumers do not need direct SQLite access

## P1 — reader, TTS, conversion, and metadata depth

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

### Conversion

- [ ] practical cross-format conversion
- [ ] format-specific settings/profiles
- [ ] batch conversion
- [ ] background jobs/progress/cancel
- [ ] metadata/cover preservation
- [ ] no final runtime dependency on Calibre conversion executables

### Metadata depth

- [ ] complete common metadata editing
- [ ] bulk editing
- [ ] metadata extraction
- [ ] online metadata/cover providers
- [ ] metadata embedding/export where formats permit
- [ ] richer custom fields/columns behavior

### CLI and automation

Maintain scriptable equivalents for major P0/P1 product domains:

- [ ] library/query operations
- [ ] metadata
- [ ] conversion
- [ ] server/protocol management
- [ ] export/catalog basics
- [ ] reader/TTS helpers where practical

The CLI is a testability and agent-development surface, not merely a power-user extra.

## P2 — secondary Calibre utilities

These remain in long-term scope but are intentionally low priority unless they become necessary for a P0/P1 consumer.

### Devices

- [ ] Windows device discovery
- [ ] Linux device discovery
- [ ] send/remove/list books
- [ ] sync/reconcile
- [ ] device path/template configuration

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

### News / web acquisition

- [ ] RSS/feed acquisition
- [ ] declarative/recipe source definitions
- [ ] webpage/article extraction
- [ ] webpage/feed -> ebook generation
- [ ] scheduled acquisition
- [ ] optional automatic library import/delivery

### Email / sharing extras

- [ ] email/SMTP delivery
- [ ] provider-specific delivery integrations only when useful
- [ ] advanced send/share workflows beyond ordinary export/download

### Plugins / extensibility

- [ ] metadata provider API
- [ ] format adapter API where safe
- [ ] device-driver API
- [ ] catalog-generator API
- [ ] news/acquisition recipe API
- [ ] explicit permissions/sandbox model

Exact Calibre plugin API compatibility is not required.

### Catalog generation

- [ ] CSV/JSON/XML catalogs
- [ ] selected/search-filtered catalogs
- [ ] ebook-form catalog generation where worthwhile
- [ ] GUI + CLI access

### Writable Calibre-library compatibility

- [ ] evaluate actual need
- [ ] compatibility fixtures across representative Calibre libraries
- [ ] safe metadata mutations
- [ ] safe format add/remove if supported
- [ ] corruption/recovery tests
- [ ] interoperability validation with Calibre itself

Writable compatibility remains separate from attach/read/index because mutating another application's live database/layout carries materially more risk.

## Explicitly not required for product identity

- pixel-for-pixel Calibre theming;
- exact parity with every Calibre plugin API;
- every legacy/obsolete format;
- every historical command-line flag;
- dependence on Calibre executables to fill missing native implementations.

The GUI should still be recognizably Calibre-like in layout and visual library workflow even though exact pixels/themes are not mandatory.

## Roadmap governance

- Current priority: `docs/project/priorities.md`.
- Current P0 execution: `docs/roadmaps/roadmap-visual-library-platform.md`.
- Product breadth: this file and `docs/project/product-scope.md`.
- Architecture: `ARCHITECTURE.md` and `docs/project/library-platform-architecture.md`.
- A Codex work item should implement one narrow slice rather than an entire capability family.
