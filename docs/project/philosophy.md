# Project Philosophy

## What Caliberate is

Caliberate is a Rust-native, cross-platform ebook library and reader intended to grow into a practical Calibre-class desktop application with unusually strong reading and text-to-speech capabilities.

The goal is not superficial feature-count parity. A button, menu item, or state field does not count as a feature unless the underlying behavior actually works.

## Core beliefs

### Real behavior over parity theater

Prefer a smaller number of end-to-end features that genuinely work over a broad shell of controls backed by placeholders. Documentation must distinguish implemented behavior from UI/state scaffolding.

### Correctness first, then ergonomics

Build reliable parsing, persistence, path handling, library operations, and state transitions before optimizing polish. Once foundations are trustworthy, quality-of-life should become a major differentiator.

### Windows and Linux are peers

The application should not be "Linux code that happens to compile on Windows" or vice versa. Platform-specific capabilities belong behind explicit boundaries and both desktop targets should remain buildable throughout development.

### TTS is a first-class reader capability

Speech is not an afterthought bolted onto rendered text. The document model, stable anchors, selection model, progress tracking, and reader state should be designed so speech and visual reading operate over the same underlying document representation.

### Normalize documents, do not duplicate readers

EPUB, HTML, DOCX, PDF, and later MOBI/AZW should not each create an independent GUI/TTS pipeline. Format-specific loaders should preserve source semantics while adapting into a common reader-facing model.

### Modularity is a feature

Avoid god files, giant catch-all state structs, and hidden cross-subsystem coupling. The codebase should remain legible enough that an agent or human can reason about one subsystem without loading the entire application into working memory.

### Declarative configuration and explicit state

Prefer TOML/configuration and explicit data models over behavior hidden in scattered constants. Defaults should be visible and platform policy should be intentional.

### Observability by default

Use structured logging and evidence-producing tests. Bugs should leave enough information to reproduce or localize them. "Works on my machine" is not a completion criterion.

### Local/offline capability matters

Core library management and reading should not require network services. Online metadata providers and optional speech providers may exist, but local books and native platform TTS should remain useful without cloud dependencies.

### Own the core product

Caliberate may temporarily use compatibility bridges while incomplete, but the finished architecture should not depend on a local Calibre installation for core reading or conversion behavior.

## Engineering style

- Rust stable and current compatible dependencies.
- Strong crate/module boundaries.
- Tests for regressions and platform-sensitive behavior.
- Extensive but useful `tracing` rather than noisy prints.
- Small, reviewable iterations with preserved behavior during structural refactors.
- No broad rewrite merely because legacy code is ugly.
- No architectural drift hidden inside implementation chores.

## Near-term product direction

The next major product arc is:

1. establish reliable native Windows parity for the existing codebase;
2. dismantle GUI concentration without changing behavior;
3. introduce a normalized document model;
4. make EPUB a real reader format;
5. add HTML and DOCX;
6. add a native Windows speech backend through a generic speech abstraction;
7. synchronize speech, navigation, and highlighting through stable document anchors;
8. tackle PDF as a distinct fixed-layout/text-extraction problem rather than pretending it is reflowable EPUB;
9. continue deeper Calibre/library/conversion parity from a cleaner base.
