# Caliberate Restart Roadmap — September 2026

This roadmap supersedes old tranche ordering for the restarted development loop. Older subsystem roadmaps remain reference material, but this file defines current priority.

`docs/project/product-scope.md` defines the durable product target: Caliberate is a standalone Rust Calibre replacement covering most high-value Calibre capability families. This roadmap orders that work; it does not reduce the scope to reader/TTS alone.

The implementation model is deliberately incremental: architecture and task boundaries are set by the architect; Codex executes bounded work items; the architect integrates accepted work; the human validates local/runtime behavior when needed.

## Phase 0 — Re-establish trustworthy baseline

Goal: make the existing repository a reliable cross-platform starting point before feature expansion.

### Objectives

- Fix the confirmed Windows same-path conversion regression.
- Add/restore repeatable validation commands for Windows and Linux.
- Establish Windows CI alongside Linux CI.
- Inventory platform assumptions: filesystem identity, device roots, process launching, shelling out, path separators, permissions, and system-open behavior.
- Classify the existing warning backlog instead of mixing it into unrelated feature tasks.

### Exit gate

- workspace builds on Windows and Linux;
- `cargo test --workspace` is green on both, or every remaining failure is explicitly documented/accepted;
- GUI launches on native Windows;
- known OS-specific assumptions have owners/tasks.

## Phase 1 — Establish standalone library modes

Goal: ensure the core application model does not assume every book is copied into one Caliberate-owned store and does not require Calibre.

### Objectives

- Define/implement explicit library-source/storage boundaries for:
  - Caliberate-managed libraries;
  - arbitrary directory-backed/reference libraries;
  - existing Calibre-library attachment.
- Support a flat directory or arbitrary directory tree of ebooks as a valid indexed/reference workflow.
- Keep source files in place for reference mode.
- Add rescan/reconciliation behavior for directory-backed sources.
- Recognize an existing Calibre library root (`metadata.db` plus book files) without launching Calibre.
- Start Calibre compatibility as read/index/attach mode rather than risky source mutation.
- Keep Caliberate-owned reading state, annotations, tags/overrides, and indexes separate from externally owned source data.

### Exit gate

- Caliberate can create/use its own managed library;
- Caliberate can index/read books from an arbitrary source directory without copying them;
- Caliberate can attach to a representative existing Calibre library and enumerate/open books with Calibre absent;
- no generic library/GUI code must know Calibre's database schema directly.

This phase can be implemented incrementally and does not need every future writable-compatibility feature before reader work proceeds.

## Phase 2 — Decompose GUI concentration without changing behavior

Goal: stop `crates/gui/src/views.rs` from being the place every new feature lands.

### Objectives

- Identify coherent seams already present in `views.rs`.
- Extract reader state/view first because reader/TTS is a major near-term product area.
- Extract other high-churn concerns such as library, metadata, conversion, and device UI incrementally.
- Keep tests/build green after every extraction.
- Avoid redesigning behavior during moves unless a separate task authorizes it.

### Exit gate

- reader code has a dedicated module tree;
- library-facing UI code has a clear seam for multiple library-source modes;
- new reader/library work no longer requires adding unrelated code to `views.rs`;
- no functional regression in existing GUI startup/basic navigation.

## Phase 3 — Normalized document foundation

Goal: create one source of truth for reading, search, annotations, and speech.

### Objectives

- Add `crates/document`.
- Define sections, blocks, text ranges, resources, links, TOC nodes, and stable anchors.
- Make the model independent of egui and platform APIs.
- Define loader-facing errors/diagnostics.
- Add tests for anchor/range invariants before real format complexity arrives.

### Exit gate

- a synthetic/document-test fixture can drive reader navigation/search without format-specific GUI code;
- persistence-oriented anchors do not depend on pagination/font layout.

## Phase 4 — Real EPUB reader

Goal: make EPUB the first genuinely supported rich reader format.

### Objectives

- Implement EPUB adapter in `crates/formats` into `crates/document`.
- Preserve EPUB spine order, metadata, resources, native TOC/navigation, internal links, and useful source anchors.
- Render reflowable content in the GUI reader.
- Make search/bookmarks/highlights operate against normalized locations rather than page-number-only state.
- Add representative fixtures/tests.

### Exit gate

- a normal EPUB can be opened from managed, directory-backed, or attached-library sources;
- native TOC navigation/search works across sections;
- pagination/layout changes do not invalidate semantic reading position.

## Phase 5 — HTML and DOCX adapters

Goal: extend the same reader pipeline rather than creating separate readers.

### Objectives

- HTML -> normalized document.
- DOCX -> normalized document.
- Preserve headings, links, paragraph structure, lists, and images where available.
- Reuse the same reader/search/annotation code as EPUB.

### Exit gate

- EPUB, HTML, and DOCX all exercise the same reader core with format-specific behavior confined to adapters.

## Phase 6 — Speech architecture and native Windows TTS

Goal: make TTS a first-class reader subsystem.

### Objectives

- Add `crates/speech` and a platform-neutral engine contract.
- Implement native Windows voice enumeration and speech playback behind a Windows backend.
- Expose stop/pause/resume/rate/voice controls.
- Feed speech from normalized document ranges.
- Add progress/position events sufficient for synchronized visual highlighting when supported by the backend.
- Keep Windows-specific API types out of generic GUI state.

### Exit gate

- user can choose a Windows voice and read normalized EPUB/HTML/DOCX content aloud;
- stopping/changing document position reliably cancels obsolete speech;
- speech lifecycle emits useful tracing;
- Calibre is not required for speech or reading.

## Phase 7 — Reader/TTS integration hardening

Goal: make speech and visual reading behave like one system rather than two features.

### Objectives

- click-to-jump/start speech from a semantic range;
- synchronized highlighting;
- robust pause/resume/stop across chapter boundaries;
- auto-follow/scroll policies;
- persist last reading/speech position;
- persist bookmarks/highlights/notes against robust anchors;
- protect against stale async events after document/voice changes;
- internal links/footnotes/back navigation.

### Exit gate

- long-form reading is stable across navigation, pause/resume, chapter transitions, layout changes, and reopening a book.

## Phase 8 — PDF as a distinct document problem

Goal: support PDF without forcing fixed-layout documents through reflowable assumptions.

### Objectives

- evaluate/choose a Rust-compatible PDF text/rendering strategy;
- preserve page identity and extracted text anchors;
- support page rendering/zoom and text search/extraction where possible;
- define what TTS means when PDF text extraction is incomplete or reading order is ambiguous;
- surface limitations rather than fabricating structure.

### Exit gate

- common text PDFs can be opened, navigated, searched, and spoken with explicit handling of extraction limitations.

## Phase 9 — Library, metadata, and organization depth

Goal: make Caliberate useful for maintaining a serious ebook collection, not merely opening files.

### Objectives

- multiple libraries and clean switching;
- multiple formats per logical book;
- robust add/remove/export/restore workflows;
- duplicate detection/merge workflows;
- metadata editing including authors, series, tags, ratings, identifiers, languages, publisher/dates, comments, covers, and custom fields;
- bulk metadata editing;
- metadata extraction and optional online metadata/cover providers;
- saved searches, virtual libraries/collections, category browsing/facets, sorting/filtering;
- FTS/indexing and large-library performance;
- integrity checks, orphan/missing-file detection, and repair tooling;
- deepen existing Calibre-library compatibility and decide whether writable compatibility is worth supporting.

### Exit gate

- Caliberate can serve as the day-to-day manager of a nontrivial ebook library without falling back to Calibre for ordinary organization/metadata work.

## Phase 10 — Conversion depth

Goal: replace passthrough-only behavior with practical native conversion.

### Objectives

- implement useful cross-format conversion for priority ebook formats;
- format-specific conversion settings/profiles;
- batch conversion and job progress;
- metadata/cover preservation where formats allow;
- error diagnostics and cancellation;
- optional external Calibre bridge may exist during transition but is never required for final core workflows.

### Exit gate

- common user conversion workflows work with Calibre absent.

## Phase 11 — Devices, content server, jobs, and extensibility

Goal: deepen the remaining Calibre-class workflows already represented in the workspace.

### Devices

- Windows/Linux device discovery abstractions;
- send/remove/sync books;
- path/template configuration;
- orphan cleanup and useful failure reporting.

### Content server / OPDS

- robust browse/search/download catalog behavior;
- authentication/access control;
- local-network usefulness across all library-source modes.

### Jobs

- real background scheduling/state for ingest, indexing, conversion, metadata download, and device operations;
- cancellation, progress, failure history, and tracing.

### Plugins / extensibility

- preserve a native extension boundary with explicit permissions/interfaces;
- do not require compatibility with every Calibre plugin API.

### News/acquisition helpers

- deepen only after core library/reader workflows are strong unless a concrete user need raises priority.

## Later format breadth

MOBI/AZW-family reading/conversion remains in scope after the initial EPUB/HTML/DOCX/PDF pipeline is stable. Additional formats can be prioritized by actual library usage and implementation quality.

## Rules across all phases

- No god-file growth.
- No UI-only feature claims.
- No hidden dependency on Calibre executables for finished core workflows.
- Managed, directory-backed, and attached-Calibre library sources are architectural peers where practical.
- Windows/Linux regressions block forward motion unless explicitly documented and accepted.
- Every Codex task is bounded under `docs/work/` and written so a weak implementation agent can execute it literally.
- Architectural ambiguity returns to the architect rather than being silently solved inside implementation grunt work.
- Codex pushes code/report evidence to GitHub; ChatGPT reviews and integrates it; the human is not the courier.
- Human runtime verification is preserved as evidence when needed, preferably in files/artifacts rather than terminal scrollback.
