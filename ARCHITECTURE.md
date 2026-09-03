# Caliberate Architecture

This document defines the current architectural direction. It is intentionally higher-level than subsystem implementation notes and older tranche documents.

## System goal

Caliberate is a native Rust, cross-platform, standalone Calibre-class ebook platform. It should eventually cover most high-value Calibre capability families deeply enough to serve as the primary application for an ebook library.

It is not a pixel-for-pixel Calibre clone and it is not a collection of parity checkboxes. The architecture should make real library management, metadata, search, reading, text-to-speech, conversion, server, device, and background-job behavior possible without concentrating the system in one application crate or GUI file.

Caliberate must work without Calibre installed or running. Existing Calibre data is an interoperability target, not a runtime dependency.

Windows and Linux are first-class desktop targets.

## Existing workspace

The existing crates remain meaningful boundaries:

- `app`: executable wiring and bootstrap
- `core`: configuration, logging, metrics, shared runtime policy
- `db`: SQLite persistence and metadata queries
- `library`: ingest and library-domain operations
- `assets`: content-addressed/local asset storage concerns
- `conversion`: conversion orchestration
- `server`: HTTP/OPDS service
- `device`: device discovery and synchronization
- `plugins`: plugin manifests and permissions
- `gui`: eframe/egui presentation and interaction
- `jobs`: background job infrastructure
- `metadata`: metadata extraction/providers/archive metadata
- `formats`: format-specific parsing/adaptation
- `zpaq`: ZPAQ support

These boundaries should be improved incrementally rather than replaced wholesale.

## Library architecture

Library storage and library indexing must be decoupled enough to support multiple first-class workflows.

### Library modes

Caliberate should converge on an explicit library-source/storage abstraction that can represent at least:

1. **Caliberate-managed library**
   - Caliberate owns layout, metadata persistence, file lifecycle, and assets.
   - Copy/move ingest is normal.

2. **Directory-backed/reference library**
   - User points at an arbitrary directory tree of ebooks.
   - Source files remain in place.
   - Caliberate keeps its own index/metadata/reading state separately.
   - Rescan/reconciliation discovers additions, removals, and moves where practical.
   - A flat directory of ebook files is valid.

3. **Existing Calibre-library source**
   - User points at a Calibre library root containing `metadata.db` and its book directory tree.
   - Caliberate can discover/index/read the library with Calibre completely absent.
   - Initial compatibility should be attach/read/index only, then overlay Caliberate-owned state, then carefully tested writable compatibility if desirable.

Generic library-domain code should operate over logical books/formats/metadata rather than assuming every file lives in Caliberate's managed asset store.

### Library source vs Caliberate state

External source trees must not be mutated merely because they are indexed.

Caliberate-owned state may include:

- normalized/indexed metadata;
- search indexes;
- reading position;
- bookmarks/highlights/notes;
- user tags/overrides;
- cached covers/resources;
- reconciliation identity needed to track source files.

For directory-backed or attached Calibre libraries, this state should live in Caliberate-controlled storage unless an explicit writable-compatibility mode authorizes source mutation.

### Calibre interoperability boundary

Calibre library compatibility must not mean shelling out to Calibre.

If Caliberate reads `metadata.db`, it should do so through its own compatibility adapter. Calibre-specific schema/layout details should be isolated behind a library-source adapter rather than leaking through generic library, GUI, or reader code.

Writable Calibre compatibility is higher risk than read/index compatibility and should be a separate architectural milestone with fixtures and corruption/recovery tests before normal use.

## Reader architecture

The reader should converge on four distinct layers.

### 1. Normalized document model

Add a dedicated `crates/document` crate when reader-format implementation begins.

It owns format-independent concepts such as:

- document metadata needed by the reader
- ordered sections/chapters
- block structure
- text spans/ranges
- images/resources
- links and destinations
- table-of-contents nodes
- stable source anchors used for bookmarks, notes, highlights, search, and speech position

The normalized model must not depend on egui, Windows APIs, or a specific input format.

### 2. Format adapters

`crates/formats` owns format-specific loading and transforms source files into the normalized document model.

Initial reader priorities:

1. EPUB
2. HTML
3. DOCX
4. PDF

MOBI/AZW remain valid later format targets.

Format-specific TOCs, links, section structure, and source location information should be preserved when the source provides them. The GUI must not reconstruct an EPUB/PDF TOC by scanning rendered text for Markdown-like headings.

PDF is allowed to retain fixed-layout/page-specific information because it is structurally different from reflowable formats, but that information still enters the reader through the document abstraction rather than direct GUI parsing.

### 3. Speech subsystem

Add a `crates/speech` crate when TTS work begins.

The generic interface should cover at least:

- enumerate voices
- select voice
- speak a bounded text/range request
- pause
- resume
- stop
- rate control
- completion/progress events
- position information sufficient to synchronize highlighting when the backend supports it

Backends are platform or provider modules behind the same interface. The first required backend is native Windows TTS. Other backends may follow, but generic reader state must not call Windows APIs directly.

### 4. GUI reader

`crates/gui` owns presentation and interaction only:

- viewport/pagination/reflow presentation
- navigation controls
- selection
- highlight visualization
- annotation UI
- search UI
- speech controls
- reader preferences

It consumes normalized documents and the speech abstraction.

The current giant `crates/gui/src/views.rs` is legacy concentration and must be decomposed incrementally without behavior rewrites. New reader work should move toward a structure like:

```text
crates/gui/src/
  reader/
    mod.rs
    state.rs
    view.rs
    navigation.rs
    search.rs
    annotations.rs
    speech.rs
  library/
  metadata/
  conversion/
  devices/
  widgets/
```

Exact filenames may evolve, but subsystem ownership should remain explicit.

## Dependency direction

Preferred direction for reader work:

```text
formats  ---> document
speech   ---> document (only if stable text/range types are needed)
gui      ---> document
gui      ---> speech
app      ---> gui / formats / speech for composition
```

`document` must not depend on `formats`, `speech`, or `gui`.

Format parsers must not depend on GUI types.

Platform speech implementations must not leak platform-specific types into GUI state.

Preferred direction for library compatibility work:

```text
calibre-compat adapter ---> generic library/domain types
directory scanner      ---> generic library/domain types
managed storage        ---> generic library/domain types
gui/app                ---> library/domain API
```

The exact crate placement of Calibre compatibility can be decided when implemented, but Calibre-specific SQLite/layout knowledge must remain isolated from generic GUI and reader code.

## Persistence

Reader persistence belongs in database/domain layers, not ad-hoc GUI serialization.

Eventually persist:

- last reading position
- bookmarks
- highlights
- notes
- reader profile/preferences where appropriate

Persistent anchors should refer to normalized/source document locations and survive ordinary pagination, font-size, margin, and viewport changes.

Library persistence must distinguish Caliberate-owned state from externally owned source data. Reference/compatibility modes should never silently convert into managed-storage semantics.

## Cross-platform policy

- Windows and Linux must both build continuously.
- OS-specific code belongs in narrowly scoped modules.
- Path identity, process launching, filesystem semantics, device enumeration, and speech are explicitly platform-sensitive areas.
- Do not assume Unix mount roots, separators, shell commands, or canonical path spelling in generic code.
- Tests should encode platform-independent intent where possible and include targeted platform regressions when necessary.

## Modularity policy

Caliberate deliberately rejects god files and catch-all modules.

- A module should represent one coherent responsibility.
- Independent state machines, loaders, backends, adapters, and dialogs deserve separate modules.
- Hand-maintained files crossing roughly 1,000 lines require scrutiny and a reason not to split.
- Multi-thousand-line hand-maintained files are architectural debt and should shrink as nearby work occurs.
- Refactors should be staged so behavior remains testable after each step.

## Conversion boundary

Core conversion should ultimately be native/owned by Caliberate. An external Calibre `ebook-convert` invocation may be used only as an explicitly optional compatibility bridge during development; it must not become an architectural dependency for the finished application.

The program must remain able to launch, manage libraries, read supported formats, and use native TTS with no Calibre executable present.

## Server and device boundaries

The content server/OPDS layer should consume generic library APIs, not know whether books come from managed storage, a referenced directory, or an attached Calibre library.

Device synchronization should likewise operate on logical book/format selections while platform-specific device discovery remains isolated behind OS-specific adapters.

## Observability

Meaningful operations should emit structured `tracing` events with enough context to diagnose failures without attaching a debugger. Platform/backend selection, library-source reconciliation, Calibre-library compatibility, document loading, reader navigation failures, speech lifecycle transitions, and conversion jobs are especially important.

## Change rule

Changes to these architectural boundaries are architect-owned. Implementation tasks may expose a needed change, but Codex should report the conflict rather than silently redesigning the system.
