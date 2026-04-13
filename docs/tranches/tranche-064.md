# Tranche 064

## Scope (30 items)
- [x] Add publish-slot fields (`imprint`, `edition`, `rights`) to metadata edit state in `crates/gui/src/views.rs`
- [x] Initialize publish-slot fields in `EditState::default` in `crates/gui/src/views.rs`
- [x] Initialize publish-slot fields in `EditState::from_details` in `crates/gui/src/views.rs`
- [x] Add helper `ensure_publish_slot_columns` for metadata slot persistence strategy via custom columns in `crates/gui/src/views.rs`
- [x] Add helper `load_publish_slots` for loading publish metadata slots in `crates/gui/src/views.rs`
- [x] Add helper `save_publish_slots` for persisting publish metadata slots in `crates/gui/src/views.rs`
- [x] Load publish slots when opening metadata editor (`begin_edit`) in `crates/gui/src/views.rs`
- [x] Load publish slots after loading details in `load_details` in `crates/gui/src/views.rs`
- [x] Load publish slots during edit cancel reset in `cancel_edit` in `crates/gui/src/views.rs`
- [x] Load publish slots during Undo-all and shortcut undo flows in `edit_dialog` in `crates/gui/src/views.rs`
- [x] Persist publish slots during save flow in `save_edit` in `crates/gui/src/views.rs`
- [x] Add imprint editor row with reset control in metadata dialog in `crates/gui/src/views.rs`
- [x] Add edition editor row with reset control in metadata dialog in `crates/gui/src/views.rs`
- [x] Add rights editor row with reset control in metadata dialog in `crates/gui/src/views.rs`
- [x] Expand metadata diff rows to include `imprint`/`edition`/`rights` in `crates/gui/src/views.rs`
- [x] Add metadata shortcut `Ctrl/Cmd+Shift+N` for normalize actions in `crates/gui/src/views.rs`
- [x] Add metadata shortcut `Ctrl/Cmd+Shift+F` for conflict auto-fix actions in `crates/gui/src/views.rs`
- [x] Add normalize action button in validation summary in `crates/gui/src/views.rs`
- [x] Add auto-fix conflicts action button in validation summary in `crates/gui/src/views.rs`
- [x] Add helper `dedupe_identifier_lines` used by auto-fix action in `crates/gui/src/views.rs`
- [x] Add `active_pane` state to preferences view in `crates/gui/src/preferences.rs`
- [x] Add `PrefPane` enum to model preferences tree subpanes in `crates/gui/src/preferences.rs`
- [x] Add `PrefPane::section` mapping for pane-to-section navigation in `crates/gui/src/preferences.rs`
- [x] Add `PrefPane::label` pane naming for preferences tree in `crates/gui/src/preferences.rs`
- [x] Add `PrefPane::for_section` pane listing for each section in `crates/gui/src/preferences.rs`
- [x] Add `preferences_tree` renderer in `crates/gui/src/preferences.rs`
- [x] Refactor preferences UI layout to two-column tree + pane content in `crates/gui/src/preferences.rs`
- [x] Sync tab clicks and open-section methods to update focused pane in `crates/gui/src/preferences.rs`
- [x] Add pane-aware default-open behavior for section subpanes in `render_behavior_section`, `render_look_feel_section`, `render_import_export_section`, `render_advanced_section`, and `render_system_section` in `crates/gui/src/preferences.rs`
- [x] Resolve remaining unchecked GUI roadmap items and mark complete in `docs/roadmaps/gui/metadata.md` and `docs/roadmaps/gui/settings.md`

## Notes
- Persistence strategy for additional publish metadata slots is implemented using dedicated custom columns (`imprint`, `edition`, `rights`) created and maintained by GUI-side helpers.
- GUI roadmap parity files are now fully checked for `metadata` and `settings`.
