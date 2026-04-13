# Tranche 057

## Scope (30 items)
- [x] Add `uuid` crate dependency to GUI for metadata UUID generation and validation
- [x] Add `last_modified` field to metadata edit state model
- [x] Populate `last_modified` from DB extras when loading edit state
- [x] Add last-modified control row to metadata editor dialog
- [x] Add last-modified quick action (`Now`) in metadata editor
- [x] Persist `last_modified` edits via `db.update_book_last_modified(...)`
- [x] Add UUID controls (edit/reset/generate/copy) to metadata editor dialog
- [x] Persist UUID edits via `db.update_book_uuid(...)`
- [x] Add UUID validation in metadata save workflow
- [x] Add date/datetime validation for `timestamp` in metadata save workflow
- [x] Add date/datetime validation for `pubdate` in metadata save workflow
- [x] Add date/datetime validation for `last_modified` in metadata save workflow
- [x] Add metadata validation summary panel in editor dialog
- [x] Add title sort derive helper and wire it into metadata editor UI
- [x] Rename series-sort editor label to title-sort semantics in metadata editor UI
- [x] Include last-modified and UUID in metadata diff view rows
- [x] Add normalization action for author list in metadata editor
- [x] Add normalization action for tag list in metadata editor
- [x] Add normalization action for language list in metadata editor
- [x] Add normalization action for identifier lines in metadata editor
- [x] Add identifier normalization helper for lowercase type canonicalization
- [x] Add datatype-aware custom field widgets (bool/int/float/text) in metadata editor
- [x] Load custom field values when details are loaded via `load_details(...)`
- [x] Ensure custom field values reload correctly during begin-edit and cancel-edit flows
- [x] Add active-source guard in metadata download dialog to auto-switch to first enabled source
- [x] Add metadata-download merge-rule presets (`Conservative`, `Balanced`, `Replace`)
- [x] Keep metadata-download merge-rule toggles synchronized from config defaults when opening dialogs
- [x] Extend metadata roadmap with concrete metadata-editor parity sub-items (done + pending)
- [x] Add unchecked parity decomposition items for remaining metadata-editor completion scope
- [x] Maintain buildability after tranche with full fmt + targeted test suite verification

## Notes
- This tranche focused on deepening metadata-editor parity and reducing edit-time data quality issues through validation and normalization.
- Remaining metadata parity work is now better decomposed in `docs/roadmaps/gui/metadata.md` rather than hidden behind a single umbrella checkbox.
