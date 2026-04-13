# Tranche 063

## Scope (30 items)
- [x] Add plugin manager dialog state (`PluginManagerDialogState`) to `LibraryView` in `crates/gui/src/views.rs`
- [x] Add plugin record model (`PluginEntry`) with status/settings/log fields in `crates/gui/src/views.rs`
- [x] Seed plugin manager with default built-in plugin records in `PluginManagerDialogState::default` in `crates/gui/src/views.rs`
- [x] Add `open_manage_plugins` workflow in `crates/gui/src/views.rs`
- [x] Wire plugin manager entry point into Manage section (`Plugins…`) in `crates/gui/src/views.rs`
- [x] Add plugin manager window renderer (`plugins_dialog`) in `crates/gui/src/views.rs`
- [x] Implement plugin list with enable/disable toggles in `plugins_dialog` in `crates/gui/src/views.rs`
- [x] Implement plugin search/filter control in `plugins_dialog` in `crates/gui/src/views.rs`
- [x] Implement plugin details panel (version/latest/author/description/status) in `plugins_dialog` in `crates/gui/src/views.rs`
- [x] Implement plugin update-check action and status/log updates in `plugins_dialog` in `crates/gui/src/views.rs`
- [x] Implement plugin install workflow UI + action in `plugins_dialog` in `crates/gui/src/views.rs`
- [x] Implement plugin remove workflow UI + action in `plugins_dialog` in `crates/gui/src/views.rs`
- [x] Implement per-plugin settings panel with apply action in `plugins_dialog` in `crates/gui/src/views.rs`
- [x] Implement plugin error/status rendering in details panel in `plugins_dialog` in `crates/gui/src/views.rs`
- [x] Implement dependency conflict warning rendering for missing dependencies in `plugins_dialog` in `crates/gui/src/views.rs`
- [x] Implement plugin log viewer section in `plugins_dialog` in `crates/gui/src/views.rs`
- [x] Add reader state for search scope/highlighting/results cursor in `ReaderState` in `crates/gui/src/views.rs`
- [x] Add reader state for TOC/bookmarks/highlights/annotations in `ReaderState` in `crates/gui/src/views.rs`
- [x] Add reader state for go-to page/percent + continuous/paged + fit/image zoom controls in `ReaderState` in `crates/gui/src/views.rs`
- [x] Add reader state for font family/margins/preset/justification/hyphenation preferences in `ReaderState` in `crates/gui/src/views.rs`
- [x] Add reader supporting enums/models (`ReaderSearchScope`, `ReaderSearchResult`, `ReaderTocEntry`, `ReaderBookmark`, `ReaderHighlightColor`, `ReaderAnnotation`, `ReaderFitMode`, `ReaderFontFamily`, `ReaderPreset`) in `crates/gui/src/views.rs`
- [x] Implement TOC rebuild workflow from loaded text content via `rebuild_toc` in `ReaderState` in `crates/gui/src/views.rs`
- [x] Implement chapter navigation controls (`next_chapter`, `prev_chapter`) in `ReaderState` in `crates/gui/src/views.rs`
- [x] Implement bookmark add/remove behavior (`add_bookmark`, `remove_bookmark`) in `ReaderState` in `crates/gui/src/views.rs`
- [x] Implement go-to page/percent behavior (`go_to_page`, `go_to_percent`) in `ReaderState` in `crates/gui/src/views.rs`
- [x] Implement highlight/annotation add behavior (`add_annotation`) and note editing in reader UI in `crates/gui/src/views.rs`
- [x] Implement reader search results capture and result cursor navigation in `ReaderState::find_next` and `reader_dialog` in `crates/gui/src/views.rs`
- [x] Implement library-scope reader search result mode in `reader_dialog` in `crates/gui/src/views.rs`
- [x] Implement reader UI controls for continuous scroll, fit mode, and image zoom in `reader_dialog` in `crates/gui/src/views.rs`
- [x] Implement styled reader rendering helper with font family/justification/hyphenation options via `render_text_with_highlight_and_style` in `crates/gui/src/views.rs`

## Notes
- Completed all items in `docs/roadmaps/gui/plugins.md` and `docs/roadmaps/gui/reader.md`.
- Reader UI now includes TOC, bookmarks, search results panel, highlight/annotation panel, and richer reading preferences controls.
