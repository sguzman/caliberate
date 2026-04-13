# Tranche 061

## Scope (30 items)
- [x] Add merge state fields (`merge_from`, `merge_to`) to tag manager dialog state in `crates/gui/src/views.rs`
- [x] Add merge state fields (`merge_from`, `merge_to`) to series manager dialog state in `crates/gui/src/views.rs`
- [x] Add bulk tag operation state (`bulk_tag`) to tag manager dialog state in `crates/gui/src/views.rs`
- [x] Add series renumbering state (`renumber_name`, `renumber_start`, `renumber_step`) to series manager dialog state in `crates/gui/src/views.rs`
- [x] Add custom column import/export path state to custom column manager dialog in `crates/gui/src/views.rs`
- [x] Add saved search folder list and folder creation state to virtual library manager dialog in `crates/gui/src/views.rs`
- [x] Add query builder state (`builder_field`, `builder_op`, `builder_value`) to virtual library manager dialog in `crates/gui/src/views.rs`
- [x] Add saved search import/export path state to virtual library manager dialog in `crates/gui/src/views.rs`
- [x] Add virtual library assign/unassign state (`assign_name`, `unassign_name`) in virtual library manager dialog in `crates/gui/src/views.rs`
- [x] Add tag merge action UI and execution path using `rename_tag` in `crates/gui/src/views.rs`
- [x] Add bulk assign tag action UI for selected books in `crates/gui/src/views.rs`
- [x] Add bulk remove tag action UI for selected books in `crates/gui/src/views.rs`
- [x] Implement `bulk_assign_tag` helper for selected-book tag assignment in `crates/gui/src/views.rs`
- [x] Implement `bulk_remove_tag` helper for selected-book tag removal in `crates/gui/src/views.rs`
- [x] Add series merge action UI and execution path using `rename_series` in `crates/gui/src/views.rs`
- [x] Add selected-book series renumber action UI with start/step controls in `crates/gui/src/views.rs`
- [x] Implement `renumber_selected_series` helper in `crates/gui/src/views.rs`
- [x] Add custom columns export action UI in `crates/gui/src/views.rs`
- [x] Add custom columns import action UI in `crates/gui/src/views.rs`
- [x] Implement `export_custom_columns` JSON writer in `crates/gui/src/views.rs`
- [x] Implement `import_custom_columns` JSON reader and creator in `crates/gui/src/views.rs`
- [x] Add saved search grouping-by-folder rendering in virtual library dialog in `crates/gui/src/views.rs`
- [x] Add saved search creation path with optional folder prefix in `crates/gui/src/views.rs`
- [x] Add query-builder append action for virtual library query input in `crates/gui/src/views.rs`
- [x] Implement `append_query_builder_clause` helper in `crates/gui/src/views.rs`
- [x] Add saved search export action UI in virtual library manager in `crates/gui/src/views.rs`
- [x] Add saved search import action UI in virtual library manager in `crates/gui/src/views.rs`
- [x] Implement `export_saved_searches` JSON writer in `crates/gui/src/views.rs`
- [x] Implement `import_saved_searches` JSON reader in `crates/gui/src/views.rs`
- [x] Implement virtual library selected-book assign/unassign actions via `vl:<name>` tag helpers in `crates/gui/src/views.rs`

## Notes
- Completed and checked off related items in `docs/roadmaps/gui/collections.md` for tags/series, virtual libraries, and custom column import/export.
- Added `serde_json` dependency to GUI crate for import/export flows.
