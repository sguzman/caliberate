# Tranche 062

## Scope (30 items)
- [x] Add `ColumnPresetScope` enum for current-view vs global preset saves in `crates/gui/src/views.rs`
- [x] Add `ViewMode::preset_scope_key()` helper in `crates/gui/src/views.rs`
- [x] Add `column_preset_scope` runtime state in `LibraryView` in `crates/gui/src/views.rs`
- [x] Initialize `column_preset_scope` default in `LibraryView::new` in `crates/gui/src/views.rs`
- [x] Add `scoped_column_preset_name` helper for prefixed preset names in `crates/gui/src/views.rs`
- [x] Add `visible_column_preset_names` helper to filter presets by current view/global in `crates/gui/src/views.rs`
- [x] Keep backward compatibility for legacy unscoped preset names in `visible_column_preset_names` in `crates/gui/src/views.rs`
- [x] Update `save_column_preset` to persist scoped preset names in `crates/gui/src/views.rs`
- [x] Add column preset scope picker (`Current view`/`All views`) in layout UI in `crates/gui/src/views.rs`
- [x] Update column preset dropdown to use scoped filtered names in `crates/gui/src/views.rs`
- [x] Add custom-column edit fields (`edit_*`) to `ManageCustomColumnsDialogState` in `crates/gui/src/views.rs`
- [x] Add metadata custom-field filter state (`value_filter`) to `ManageCustomColumnsDialogState` in `crates/gui/src/views.rs`
- [x] Initialize custom-column edit/filter fields in `ManageCustomColumnsDialogState::default` in `crates/gui/src/views.rs`
- [x] Add `selected_column_label` helper for default selection in custom-column manager in `crates/gui/src/views.rs`
- [x] Add `select_custom_column_for_edit` helper to load selected column into edit controls in `crates/gui/src/views.rs`
- [x] Add `save_custom_column_edits` helper with validation in `crates/gui/src/views.rs`
- [x] Add tracing log on custom-column metadata update in `save_custom_column_edits` in `crates/gui/src/views.rs`
- [x] Extend manage custom columns dialog with selectable list rows in `crates/gui/src/views.rs`
- [x] Extend manage custom columns dialog create datatype choices to include `date` and `series` in `crates/gui/src/views.rs`
- [x] Add edit panel controls (name/datatype/display/editable/multiple/normalized) in custom-column manager in `crates/gui/src/views.rs`
- [x] Add `Save edits` action handling in custom-column manager in `crates/gui/src/views.rs`
- [x] Keep delete/edit selection synchronized through `delete_label` assignment in `crates/gui/src/views.rs`
- [x] Refresh custom-column edit selection after reload in `refresh_manage_custom_columns` in `crates/gui/src/views.rs`
- [x] Add DB API `update_custom_column` in `crates/db/src/database.rs`
- [x] Persist `editable` in `update_custom_column` SQL update in `crates/db/src/database.rs`
- [x] Persist `is_multiple` in `update_custom_column` SQL update in `crates/db/src/database.rs`
- [x] Persist `normalized` in `update_custom_column` SQL update in `crates/db/src/database.rs`
- [x] Add metadata dialog custom-field filter controls in `crates/gui/src/views.rs`
- [x] Add filtered rendering for metadata custom fields by label/name/datatype in `crates/gui/src/views.rs`
- [x] Extend `custom_field_editor_widget` with type-specific `date` validation/hint in `crates/gui/src/views.rs`

## Notes
- Completed all remaining unchecked items in `docs/roadmaps/gui/collections.md`.
- Column visibility preset behavior is now explicitly view-scoped using name prefixes (`table/`, `grid/`, `shelf/`) with optional global scope (`all/`).
