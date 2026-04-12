# Tranche 054

## Scope (30 items)
- [x] Add `comment_preview_html` toggle state to `LibraryView`
- [x] Add `identifier_io_buffer` state to `LibraryView`
- [x] Add `cover_history` state to `LibraryView`
- [x] Add `cover_favorites` state to `LibraryView`
- [x] Add `cover_restore_history` state to `LibraryView`
- [x] Add `metadata_download` dialog state to `LibraryView`
- [x] Initialize all new metadata/cover dialog states in `LibraryView::new`
- [x] Add `LibraryView::open_download_metadata()` action entrypoint
- [x] Add `LibraryView::open_download_cover()` action entrypoint
- [x] Wire `AppAction::OpenDownloadMetadata` to `LibraryView::open_download_metadata`
- [x] Wire `AppAction::OpenDownloadCover` to `LibraryView::open_download_cover`
- [x] Render metadata download dialog in the main `LibraryView` dialog pass
- [x] Add cover browser collapsible with selectable history entries
- [x] Add cover favorites collapsible with quick path restore buttons
- [x] Add removed-cover history collapsible with restore-path buttons
- [x] Record cover history on manual cover import
- [x] Record cover history on generated cover updates
- [x] Record removed cover paths in bounded restore history
- [x] Add bounded `record_cover_history` helper for deduped history ordering
- [x] Add per-field reset controls in metadata editor for title/authors/author_sort/tags/series/timestamp/pubdate/rating
- [x] Add metadata editor “Undo all” behavior based on baseline snapshot
- [x] Add identifier quick-add actions for ISBN/ASIN/DOI
- [x] Add identifier dedupe cleanup action
- [x] Add identifier copy action and import/export buffer workflow
- [x] Add identifier validation badges (valid/invalid counters)
- [x] Add external-link actions for ISBN/ASIN/DOI identifiers
- [x] Add half-star slider and normalized half-star display formatting
- [x] Add comment markdown toolbar (bold/italic/heading/link inserts)
- [x] Add markdown/HTML preview toggle in metadata editor
- [x] Add before/after metadata diff view in edit dialog

## Notes
- This tranche completes a broad first-pass metadata-editor parity slice and converts the metadata-download actions from placeholders into functional GUI workflows.
- Metadata provider integration remains stub-level and is tracked as remaining roadmap work.
