# Current Status

Baseline date: 2026-09-03.

This file records the restart baseline and should be updated when the architectural phase changes materially. Detailed historical parity/tranche documents remain useful context but may overstate runtime completeness.

## Native Windows restart baseline

Observed on Windows 11 with the stable MSVC Rust toolchain:

- the workspace compiled natively on Windows;
- `caliberate-gui.exe` built and launched;
- runtime directories under `.cache/caliberate` initialized successfully;
- the surviving test log showed one integration failure in the main CLI suite and 51 passing tests in that suite;
- the GUI build emitted a substantial warning backlog (57 warnings in the observed run), including deprecated egui calls, dead fields, and unused rendering helpers.

### Known failing Windows test

`ebook_convert_rejects_input_equals_output` fails because input and output path identity is compared by `PathBuf` spelling after only the input has been canonicalized.

On Windows an existing path may canonicalize to an extended path such as `\\?\A:\...` while the output retains ordinary `A:\...` spelling. The two paths can therefore refer to the same file while comparing unequal as strings/`PathBuf`s.

This is the first confirmed native-Windows portability bug and should be fixed before broader Windows feature work.

## Library reality

The existing library/asset code already has meaningful ingest and copy/reference behavior, but the restarted product target is broader than the current storage model.

Not yet established as first-class end-to-end workflows:

- arbitrary directory-backed libraries with persistent rescan/reconciliation while leaving files in place;
- treating a completely flat ebook directory as a normal library source;
- attaching to an existing Calibre library root and consuming `metadata.db`/book layout with Calibre absent;
- clearly separating Caliberate-owned overlay state from externally owned source-library state;
- safe writable Calibre-library compatibility.

These are now explicit architectural/product targets in `ARCHITECTURE.md` and `docs/project/product-scope.md`.

## Reader reality

The current GUI contains a large amount of reader shell/state behavior, but the runtime loader is far narrower than the advertised ingest format list.

At restart, the GUI reader's `ReaderContent::from_path` meaningfully loads only:

- `txt`
- `md`
- `markdown`

EPUB, PDF, DOCX, MOBI/AZW, and HTML are not real GUI reader loaders yet.

The existing TOC logic for text content reconstructs headings from Markdown-like text rather than preserving source-native navigation structures.

## TTS reality

No reader TTS implementation was found in the existing GUI code at restart. There is therefore no legacy speech subsystem that must be preserved. This is an opportunity to introduce the speech abstraction cleanly.

## Conversion reality

The conversion CLI and orchestration exist, but practical cross-format conversion remains largely unimplemented beyond passthrough behavior. Caliberate may use optional compatibility bridges during development, but finished core conversion must not require a Calibre installation.

## Structural debt

The GUI source is heavily concentrated. At restart:

- `crates/gui/src/views.rs` is roughly 493 KB;
- `crates/gui/src/app.rs` is roughly 65 KB;
- `crates/gui/src/preferences.rs` is roughly 68 KB.

`views.rs` contains multiple unrelated subsystems and is the most obvious god-file target. Refactoring should be incremental and behavior-preserving rather than a rewrite.

## Platform-sensitive areas already identified

- conversion/path identity and canonicalization;
- device defaults that include Unix mount roots such as `/media` and `/run/media`;
- process launching/system-open behavior in GUI code;
- removable-device discovery;
- future native TTS backend;
- any hidden assumptions about Unix separators, shells, permissions, or filesystem identity.

## Immediate priorities

1. Fix the known Windows same-path conversion regression.
2. Establish a repeatable cross-platform validation baseline and Windows CI.
3. Establish/validate the library-source abstraction for managed, directory-backed, and attached-Calibre workflows.
4. Decompose `views.rs` along existing responsibility seams without behavior changes.
5. Add the normalized document architecture described in `ARCHITECTURE.md`.
6. Implement real EPUB loading first.
7. Add Windows TTS through `crates/speech` rather than GUI-local API calls.

The broader Calibre-class scope remains active beyond these immediate priorities; see `docs/project/product-scope.md` and `docs/roadmaps/roadmap-restart-2026-09.md`.

## Completion standard

A roadmap checkbox, UI control, struct field, or stub does not establish feature completion. Completion requires executable behavior plus evidence appropriate to the feature: tests, runtime validation, or both.
