# Project Philosophy

## What Caliberate is

Caliberate is a Rust-native, cross-platform ebook platform intended to become a practical **Calibre replacement**, not merely a reader inspired by Calibre.

Its core product is the complete lifecycle around an ebook library: own/index books, manage metadata, organize/search them, read them, speak them, convert them, serve them, and send/synchronize them to devices.

It does not need exhaustive one-for-one parity with every historical Calibre feature before it is useful, but it should cover most high-value Calibre capability families deeply enough that a user can choose Caliberate as the primary application.

The goal is not superficial feature-count parity. A button, menu item, or state field does not count as a feature unless the underlying behavior actually works.

## Core beliefs

### Standalone first

Caliberate must work without Calibre installed and without a Calibre process running.

Compatibility with Calibre libraries is valuable because users already own data in that ecosystem. Runtime dependence on Calibre executables is not acceptable as the finished architecture.

### Respect existing files and libraries

A user should not be forced to surrender an existing directory layout merely to use Caliberate.

First-class workflows include:

- a Caliberate-managed library;
- an arbitrary directory-backed/reference library where files remain in place;
- attaching to an existing Calibre library directory and reading/indexing its contents without launching Calibre.

Writable Calibre-library compatibility should be introduced cautiously after read/index/overlay behavior is reliable.

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

Caliberate may temporarily use compatibility bridges while incomplete, but the finished architecture should own its core reader, library, metadata, persistence, and conversion behavior.

## Engineering style

- Rust stable and current compatible dependencies.
- Strong crate/module boundaries.
- Tests for regressions and platform-sensitive behavior.
- Extensive but useful `tracing` rather than noisy prints.
- Small, reviewable iterations with preserved behavior during structural refactors.
- No broad rewrite merely because legacy code is ugly.
- No architectural drift hidden inside implementation chores.
- Implementation work should be decomposed enough that a low-cost/low-reasoning coding agent can execute it without reconstructing architecture by guesswork.

## Product scope authority

`docs/project/product-scope.md` defines the durable capability families and supported library workflows. Roadmaps choose implementation order; they do not narrow that product scope unless the project explicitly changes direction.

## Near-term product direction

The next major product arc is:

1. establish reliable native Windows/Linux baseline for the existing codebase;
2. make standalone library modes and existing-library compatibility explicit architectural targets;
3. dismantle GUI concentration without changing behavior;
4. introduce a normalized document model;
5. make EPUB a real reader format;
6. add HTML and DOCX;
7. add native Windows speech through a generic speech abstraction;
8. synchronize speech, navigation, and highlighting through stable document anchors;
9. tackle PDF as a distinct fixed-layout/text-extraction problem;
10. deepen library/metadata/conversion/device/server behavior until Caliberate is genuinely useful as the primary Calibre-class application.
