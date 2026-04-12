# GUI Metadata Roadmap

## Metadata Editor
- [ ] Implement full Calibre metadata editor field set
- [x] Add per-field reset controls for primary metadata fields
- [x] Add global undo-all reset in metadata editor dialog
- [x] Add metadata diff view (before/after)
- [ ] Add bulk metadata merge rules UI
- [x] Add author sort field editing and persistence
- [ ] Add series sort field editing and persistence
- [x] Add publication date and timestamp fields with persistence
- [x] Add rating editor with half-star increments
- [ ] Add custom metadata fields editor (custom columns)

## Metadata Download
- [x] Add metadata download dialog with source selection
- [x] Add metadata download results comparison UI
- [x] Add cover download chooser grid
- [x] Add metadata download merge/replace options
- [x] Add metadata download progress view per book
- [x] Add metadata download source configuration UI
- [x] Add metadata download retry for failed items
- [x] Wire metadata download dialog to real provider-backed fetch pipeline
- [x] Add queued multi-book metadata download execution with per-row statuses
- [x] Add failed-row retry workflow in metadata queue
- [x] Add per-book apply target selection for downloaded metadata results

## Covers + Comments
- [x] Add cover browser with history and favorites
- [x] Add cover download + replace workflow
- [x] Add comment markdown editor toolbar
- [x] Add markdown/HTML preview toggle in editor
- [ ] Add cover paste from clipboard
- [x] Add cover remove/restore history list

## Identifiers + Links
- [x] Add identifier quick add for common types (ISBN, ASIN, DOI)
- [x] Add external link buttons (open in browser)
- [x] Add identifier validation status badges
- [x] Add identifier import/export (buffer + clipboard copy)
- [x] Add identifier cleanup/dedupe action
