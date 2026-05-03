# Caliberate

Caliberate is a Rust workspace aimed at rebuilding or rethinking a Calibre-like ebook and library-management stack as a modular set of crates.

## Intent

Split the problem into core domains such as metadata, ingest, storage, conversion, device integration, server behavior, GUI parity, and plugins so the system can evolve as a coherent platform instead of a monolith.

## Ambition

This ambition is strongly supported by the inventory and roadmap docs: the project appears to be targeting broad Calibre parity or a Calibre-inspired successor with cleaner Rust boundaries across CLI, library management, GUI, device, and conversion workflows.

## Current Status

The workspace structure and a large amount of planning/inventory documentation are already present. The implementation is still early enough that the repo reads more like a serious platform build-out than a finished user-facing application.

## Core Capabilities Or Focus Areas

- Workspace split across app, core, DB, library, assets, conversion, server, device, plugin, GUI, job, metadata, formats, and archive support crates.
- Inventory docs that map desired behavior back to Calibre reference surfaces.
- Roadmap and tranche documents that break the platform into implementation slices.
- A root workspace that is already organized for incremental subsystem development.
- Configuration and data directories for local development.

## Project Layout

- `crates/app/`: top-level application crate for wiring the platform into a runnable product surface.
- `crates/core/`: shared core abstractions such as configuration, errors, and logging foundations.
- `crates/db/`: database-facing code for persistence and query behavior.
- `crates/library/`: library-domain logic around books, records, and collection management.
- `crates/assets/`: asset management support for bundled and generated resources.
- `crates/conversion/`: ebook format conversion workflows and parity-oriented conversion logic.
- `crates/server/`: server-side behaviors analogous to Calibre content or service surfaces.
- `crates/device/`: device integration and sync-related functionality.
- `crates/plugins/`: extension/plugin system work.
- `crates/gui/`: GUI parity and desktop-facing user interface work.
- `crates/jobs/`: background job orchestration and task execution.
- `crates/metadata/`: metadata normalization, fetch, and update logic.
- `crates/formats/`: format-specific parsing/serialization support.
- `crates/zpaq/`: archive/compression support for ingest and storage workflows.
- `config/`: checked-in runtime configuration and configuration examples.
- `crates/`: workspace member crates grouped by subsystem.
- `data/`: sample data, working data, or local development artifacts.
- `docs/`: project documentation, reference material, and roadmap notes.
- `library/`: sample or working library data used during parity and ingest development.
- `Cargo.toml`: crate or workspace manifest and the first place to check for package structure.

## Setup And Requirements

- Rust toolchain.
- Patience for an in-progress workspace rather than a finished replacement product.
- Any external tools or datasets required by the specific subsystem you are developing.

## Build / Run / Test Commands

```bash
cargo build --workspace
cargo test --workspace
cargo check --workspace
```

## Notes, Limitations, Or Known Gaps

- This repository is still defining parity targets, so implementation completeness varies significantly by crate.
- The docs are a major part of the product direction, not just incidental notes.

## Next Steps Or Roadmap Hints

- Keep each parity slice tied to the corresponding inventory docs so scope stays controlled.
- Promote individual crates from scaffold to production-grade modules one subsystem at a time.
