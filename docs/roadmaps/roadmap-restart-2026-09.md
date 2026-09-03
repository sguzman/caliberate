# Caliberate Restart Roadmap — September 2026

This roadmap supersedes old tranche ordering for the restarted development loop. Older subsystem roadmaps remain reference material, but this file defines current priority.

The implementation model is deliberately incremental: architecture and task boundaries are set by the architect; Codex executes bounded work items; the human validates platform/runtime behavior.

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

## Phase 1 — Decompose GUI concentration without changing behavior

Goal: stop `crates/gui/src/views.rs` from being the place every new feature lands.

### Objectives

- Identify coherent seams already present in `views.rs`.
- Extract reader state/view first, because the reader is the next major product area.
- Extract other high-churn concerns such as library, metadata, conversion, and device UI incrementally.
- Keep tests/build green after every extraction.
- Avoid redesigning behavior during moves unless a separate task authorizes it.

### Exit gate

- reader code has a dedicated module tree;
- new reader work no longer requires adding unrelated code to `views.rs`;
- no functional regression in existing GUI startup/basic navigation.

## Phase 2 — Normalized document foundation

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

## Phase 3 — Real EPUB reader

Goal: make EPUB the first genuinely supported rich reader format.

### Objectives

- Implement EPUB adapter in `crates/formats` into `crates/document`.
- Preserve EPUB spine order, metadata, resources, native TOC/navigation, internal links, and useful source anchors.
- Render reflowable content in the GUI reader.
- Make search/bookmarks/highlights operate against normalized locations rather than page-number-only state.
- Add representative fixtures/tests.

### Exit gate

- a normal EPUB can be opened, navigated by native TOC, searched, and read across sections;
- pagination/layout changes do not invalidate semantic reading position.

## Phase 4 — HTML and DOCX adapters

Goal: extend the same reader pipeline rather than creating separate readers.

### Objectives

- HTML -> normalized document.
- DOCX -> normalized document.
- Preserve headings, links, paragraph structure, lists, and images where available.
- Reuse the same reader/search/annotation code as EPUB.

### Exit gate

- EPUB, HTML, and DOCX all exercise the same reader core with format-specific behavior confined to adapters.

## Phase 5 — Speech architecture and native Windows TTS

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
- speech lifecycle emits useful tracing.

## Phase 6 — Reader/TTS integration hardening

Goal: make speech and reading behave like one system rather than two features.

### Objectives

- click-to-jump/start speech from a semantic range;
- synchronized highlighting;
- robust pause/resume/stop across chapter boundaries;
- auto-follow/scroll policies;
- persist last reading/speech position;
- protect against stale async events after document/voice changes.

### Exit gate

- long-form reading is stable across navigation, pause/resume, chapter transitions, and reopening a book.

## Phase 7 — PDF as a distinct document problem

Goal: support PDF without forcing fixed-layout documents through reflowable assumptions.

### Objectives

- evaluate/choose a Rust-compatible PDF text/rendering strategy;
- preserve page identity and extracted text anchors;
- support page rendering/zoom and text search/extraction where possible;
- define what TTS means when PDF text extraction is incomplete or reading order is ambiguous;
- surface limitations rather than fabricating structure.

### Exit gate

- common text PDFs can be opened, navigated, searched, and spoken with explicit handling of extraction limitations.

## Phase 8 — Broader Calibre-class capability

Return to deeper library/conversion/device/plugin/server work from the cleaner architecture.

Priority should be based on real user value and implementation depth, not parity checkbox count.

## Rules across all phases

- No god-file growth.
- No UI-only feature claims.
- Windows/Linux regressions block forward motion unless explicitly documented and accepted.
- Every Codex task is bounded under `docs/work/`.
- Architectural ambiguity returns to the architect rather than being silently solved inside grunt work.
- Human runtime verification is preserved as evidence, preferably in files/artifacts rather than terminal scrollback.
