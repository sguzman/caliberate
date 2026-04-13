# Tranche 059

## Scope (30 items)
- [x] Extend `ConversionConfig` with input/output profile controls in `crates/core/src/config.rs`
- [x] Extend `ConversionConfig` with heuristic conversion toggles in `crates/core/src/config.rs`
- [x] Extend `ConversionConfig` with page margin and font embedding controls in `crates/core/src/config.rs`
- [x] Extend `ConversionConfig` with cover policy and warning toggles in `crates/core/src/config.rs`
- [x] Extend `ConversionConfig` with save-to-disk template and conflict policy fields in `crates/core/src/config.rs`
- [x] Extend `ConversionConfig` with save-to-disk preset map in `crates/core/src/config.rs`
- [x] Extend `ConversionConfig` with conversion job history/log path and retention limits in `crates/core/src/config.rs`
- [x] Add validation for conversion profile lists and selected defaults in `crates/core/src/config.rs`
- [x] Add validation for conversion margin and policy values in `crates/core/src/config.rs`
- [x] Add validation for conversion job history retention bound in `crates/core/src/config.rs`
- [x] Add default providers for all new conversion config properties in `crates/core/src/config.rs`
- [x] Ensure conversion job log directories are created during runtime path setup in `crates/core/src/paths.rs`
- [x] Ensure conversion job history parent directory is created during runtime path setup in `crates/core/src/paths.rs`
- [x] Add new conversion control-pane keys to `config/control-plane.toml`
- [x] Extend `ConversionSettings` with profile, heuristic, page setup, font, and cover policy fields in `crates/conversion/src/settings.rs`
- [x] Add builder methods in `ConversionSettings` for profile/heuristic/page-setup/cover policy overrides in `crates/conversion/src/settings.rs`
- [x] Extend convert dialog state with profile, heuristic, page setup, cover policy, and preset fields in `crates/gui/src/views.rs`
- [x] Extend save-to-disk dialog state with template, conflict policy, and preset fields in `crates/gui/src/views.rs`
- [x] Implement conversion preset save/load actions in convert dialog in `crates/gui/src/views.rs`
- [x] Implement save-to-disk preset save/load actions in export dialog in `crates/gui/src/views.rs`
- [x] Add per-format options panel scaffold (EPUB/MOBI/PDF/AZW3) in convert dialog in `crates/gui/src/views.rs`
- [x] Add input/output profile selector controls in convert dialog in `crates/gui/src/views.rs`
- [x] Add heuristic and page/font option controls in convert dialog in `crates/gui/src/views.rs`
- [x] Add conversion warning panel for unsupported option combinations in `crates/gui/src/views.rs`
- [x] Wire advanced conversion dialog settings into conversion execution settings in `crates/gui/src/views.rs`
- [x] Add export path template and conflict policy controls in save-to-disk dialog in `crates/gui/src/views.rs`
- [x] Add export preview list generation in save-to-disk dialog in `crates/gui/src/views.rs`
- [x] Add template-based destination resolution and conflict handling for exports in `crates/gui/src/views.rs`
- [x] Add conversion/save-to-disk job details + log viewer + retry/clone + queue reorder + open-output actions in jobs panel in `crates/gui/src/views.rs`
- [x] Add conversion job history persistence and per-job log-file writing in `crates/gui/src/views.rs`

## Notes
- Completed the full open item set in `docs/roadmaps/gui/conversion.md` while keeping conversion behavior config-driven in control plane.
- Added tracing-backed instrumentation around convert/export execution boundaries to improve runtime diagnostics.
