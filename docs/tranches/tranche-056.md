# Tranche 056

## Scope (30 items)
- [x] Add `arboard` dependency in GUI crate to support clipboard-driven cover workflows
- [x] Add `[metadata_download]` config keys for merge defaults (`merge_tags_default`, `merge_identifiers_default`)
- [x] Add `[metadata_download]` config keys for overwrite defaults (`overwrite_title_default`, `overwrite_authors_default`)
- [x] Add `[metadata_download]` config keys for overwrite defaults (`overwrite_publisher_default`, `overwrite_language_default`, `overwrite_pubdate_default`, `overwrite_comment_default`)
- [x] Add new metadata-download default fields to `MetadataDownloadConfig` in core config model
- [x] Add default value constructors for all new metadata-download merge/overwrite policy keys
- [x] Add validation guard ensuring metadata-download merge/overwrite defaults are not all disabled
- [x] Add new metadata-download merge/overwrite keys to `config/control-plane.toml`
- [x] Add new metadata-download merge/overwrite keys to `crates/core/tests/fixtures/control-plane.toml`
- [x] Add `series_sort` field to edit state model
- [x] Populate `series_sort` from DB extras in metadata edit baseline
- [x] Add `series_sort` editing control to metadata editor dialog
- [x] Add `series_sort` reset control in metadata editor dialog
- [x] Persist `series_sort` edits via `db.update_book_sort(...)`
- [x] Include `series_sort` in metadata diff view rows
- [x] Add `CustomEditField` model for per-book custom-column editing in metadata editor
- [x] Add `edit_custom_fields` state to `LibraryView`
- [x] Load editable custom-column values when entering metadata edit mode
- [x] Reload editable custom-column values on metadata edit cancel/undo reset
- [x] Render custom metadata field editor section inside metadata edit dialog
- [x] Persist custom metadata edits via `db.set_custom_value(...)` during metadata save
- [x] Add `Paste cover` action button in details cover section
- [x] Add new detail action variant for clipboard-based cover apply
- [x] Implement clipboard image extraction workflow (`arboard` image payload)
- [x] Implement clipboard text fallback workflow (clipboard path to local image file)
- [x] Persist clipboard-derived cover images into configured cover storage paths
- [x] Add merge-rules UI section to metadata download dialog
- [x] Add runtime dialog defaults reset for merge/overwrite policy toggles when opening metadata download workflows
- [x] Apply merge-rule policy toggles in metadata download apply logic (title/authors/publisher/language/pubdate/comment)
- [x] Apply merge-rule policy toggles in metadata download apply logic (tags/identifiers merge switches)

## Notes
- This tranche closes the remaining concrete metadata roadmap items for series sort, custom metadata field editing, cover paste from clipboard, and bulk merge rules UI.
- The top-level parity umbrella item in metadata roadmap remains open by design until broader Calibre field coverage is fully decomposed and completed.
