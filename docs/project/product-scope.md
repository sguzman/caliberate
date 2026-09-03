# Product Scope

## Product definition

Caliberate is intended to become a practical Rust-native Calibre replacement: a standalone desktop ebook library, reader, metadata manager, converter, content server, device-management application, and ebook utility suite.

It does **not** need to reproduce every historical Calibre feature or every plugin ecosystem detail before it is useful. The target is broad functional coverage of the parts that make Calibre valuable, implemented deeply enough that Caliberate can be used as the primary application rather than as a companion shell around Calibre.

Caliberate must work when Calibre is not installed and when no Calibre process is running.

## First-class library workflows

Caliberate should support multiple library modes without forcing one storage philosophy on the user.

### 1. Caliberate-managed library

Caliberate owns the library layout, metadata database, assets, and file lifecycle.

Expected capabilities:

- create/open a library;
- add ebooks by copy or move;
- maintain multiple formats for one logical book;
- edit metadata, covers, tags, series, ratings, identifiers, comments, custom fields, and related book data;
- search/filter/sort/browse large collections;
- export/send/copy books without exposing internal storage details;
- detect missing/orphaned/corrupt files and repair library state where possible.

### 2. Directory-backed library

The user can point Caliberate at an arbitrary directory tree containing ebook files and use it without surrendering file ownership or forcing a Calibre/Caliberate directory layout.

This mode should support a reference/index workflow:

- files remain where they are;
- Caliberate indexes them and stores application metadata separately;
- rescans reconcile additions, removals, and moves where practical;
- the application can read, search, tag, annotate, and speak books from the indexed directory;
- no Calibre installation is required.

A flat directory of ebook files is therefore a valid workflow.

### 3. Existing Calibre-library compatibility

The user can point Caliberate at an existing Calibre library directory and use the books without running Calibre.

A Calibre library commonly contains `metadata.db` plus the author/title directory tree and format files. Caliberate should recognize that layout and preserve access to the existing content.

Compatibility should be staged deliberately:

1. **attach/read/index mode** — discover the library, consume useful Calibre metadata, and read books without modifying Calibre-owned structures;
2. **overlay mode** — allow Caliberate-specific annotations, reading state, and metadata extensions without requiring destructive changes to Calibre's database/layout;
3. **writable compatibility mode** — only after behavior is well-tested, support safe mutations intended to remain usable by Calibre where feasible.

The architecture must not require Calibre to be installed, launched, or invoked for this workflow.

## Core product capability families

These are durable product areas even if their implementation order changes.

### Library management

- multiple libraries;
- add/remove/restore/export books;
- copy/reference/directory-backed ingest;
- duplicate detection and merge workflows;
- multiple formats per book;
- covers and extra assets;
- library integrity checking and repair;
- backup/restore and portable export workflows;
- large-library performance.

### Metadata

- title/authors/author sort;
- series/index;
- tags/categories;
- ratings;
- publishers/dates/languages;
- identifiers such as ISBN;
- comments/descriptions;
- custom columns/fields;
- bulk metadata editing;
- metadata extraction from files;
- optional online metadata and cover download;
- embed/export metadata where formats permit.

### Search and organization

- full-text metadata search;
- saved searches;
- virtual libraries/collections;
- category browser/facets;
- sorting and filtering;
- book-details inspection;
- duplicate and quality checks;
- reusable template/filter concepts where they materially improve library workflows.

### Reader

- EPUB, HTML, DOCX, PDF, and eventually MOBI/AZW-family support;
- native TOC/navigation where the source provides it;
- reflow, typography, themes, margins, line spacing, zoom/page controls as appropriate;
- search;
- bookmarks;
- highlights and notes;
- internal links/footnotes/back navigation;
- persistent reading position;
- robust source anchors independent of viewport/font changes.

### Text to speech

- first-class reader integration;
- native Windows voices as a required backend;
- platform/provider abstraction for additional backends;
- voice selection, rate, stop/pause/resume;
- click/jump/start-from-position behavior;
- synchronized visual highlighting when backend timing information permits;
- reliable cancellation and chapter/document transitions.

### Conversion

- practical conversion among major ebook/document formats where technically sensible;
- format-specific options and profiles over time;
- batch conversion;
- conversion jobs/progress/logging;
- metadata/cover preservation where possible;
- optional external compatibility bridges are permitted during development, but final core conversion must not require Calibre.

### Ebook editing and polishing

Calibre includes a real editor for EPUB/AZW3-family books with HTML/CSS editing, live preview, search/replace, resource management, and automated cleanup. Caliberate does not need this early, but ebook editing/repair is part of the long-term Calibre-class scope.

Possible capability set:

- inspect/edit internal HTML/CSS/resources;
- live preview;
- search/replace across book files;
- rename/merge/reorder internal files;
- add/remove/replace images/fonts/stylesheets;
- validate/fix common EPUB structure problems;
- compare ebook revisions;
- non-destructive polish operations such as metadata/cover/font cleanup where practical.

### Devices

- discover common connected ebook devices where the OS exposes them;
- send/remove/synchronize books;
- configurable device paths/templates;
- platform-specific discovery behind explicit abstractions;
- no Unix-only mount-root assumptions in generic code.

### Sharing and delivery

- copy/export books to chosen destinations;
- send-to-device workflows;
- optional email/SMTP delivery where useful;
- preserve a clean abstraction so delivery is not tied to one provider or device family.

### Content server / OPDS

- serve library catalogs and book files;
- search/browse endpoints;
- authentication/access control;
- browser-based book reading where practical;
- useful local-network workflow without requiring a cloud service;
- eventually support library mutations through the server only if access control and consistency are sound.

### Catalog generation

- export library catalogs in useful formats such as CSV/JSON/XML and, where worthwhile, ebook catalog formats;
- filter catalogs by search/selection;
- expose catalog generation through GUI and CLI surfaces.

### Jobs and background work

- background ingest, conversion, metadata download, indexing, news acquisition, and device operations;
- observable job state and failures;
- cancellation where meaningful.

### Command-line interface

Calibre exposes a comprehensive CLI across most of its functionality. Caliberate should preserve a serious CLI as a first-class automation/testing surface rather than treating the GUI as the only product.

The exact executable names do not need perfect one-for-one compatibility, but major domains should remain scriptable:

- library/database management;
- conversion;
- metadata inspection/editing;
- server management;
- ebook viewing/editing/polishing where practical;
- metadata fetching;
- device operations;
- catalog/export helpers.

CLI coverage is especially valuable because it provides deterministic integration points for tests and agent-driven development.

### Plugins / extensibility

Caliberate should retain an extensibility boundary, but exact Calibre plugin API compatibility is not a requirement. Native Caliberate extensions should use explicit permissions/interfaces rather than allowing plugin concerns to infect core architecture.

Useful extension families may include:

- metadata/cover providers;
- format import/export adapters;
- device drivers;
- catalog generators;
- acquisition/news recipes;
- UI actions where a safe extension boundary exists.

### News / web acquisition

Calibre has a mature recipe-based news acquisition system. Existing Caliberate news scaffolding may evolve into:

- RSS/feed/article acquisition;
- recipe/declarative source definitions;
- webpage-to-ebook generation;
- scheduled/background acquisition;
- optional automatic library import and delivery.

This remains lower priority than core library, reader, metadata, conversion, TTS, and device workflows.

## Compatibility principle

"Calibre replacement" means the user can perform the normal lifecycle of owning and using an ebook library without keeping Calibre available as a hidden runtime dependency.

Interoperability with Calibre data is valuable; dependency on Calibre executables is not.

## Scope principle

The project should implement **most high-value Calibre capability families**, but does not promise exhaustive one-for-one parity. Features earn priority by user value, architectural fit, and implementation depth rather than by checkbox count.

Calibre's current documentation remains a useful reference inventory for capability discovery, but Caliberate is free to provide cleaner Rust-native architecture and different UX where that is better.
