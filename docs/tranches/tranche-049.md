# Tranche 049

## Scope (30 items)
- [x] Add `ViewMode::Shelf` for library rendering
- [x] Add shelf mode option in layout view selector
- [x] Implement shelf view renderer with cover-first cards
- [x] Add shelf row quick actions (edit/remove/convert)
- [x] Add configurable shelf column count control in layout panel
- [x] Persist `gui.shelf_columns` to control-plane runtime config
- [x] Add `GroupMode` enum for table grouping
- [x] Add group selector control (none/series/authors/tags)
- [x] Parse and persist `gui.group_mode`
- [x] Add grouped table display model with header rows
- [x] Add grouped table row builder from filtered books
- [x] Add group-aware comparator before primary/secondary sort
- [x] Add in-table inline edit state for title/authors/tags
- [x] Add inline edit entry action per table row
- [x] Add inline edit save flow to DB (`title`, `authors`, `tags`)
- [x] Add inline edit cancel flow
- [x] Add conditional row coloring rule for missing cover
- [x] Add conditional row coloring rule for low rating
- [x] Add configurable low-rating threshold handling
- [x] Add hex color parser for configurable row colors
- [x] Add control-plane keys for conditional formatting toggles/colors
- [x] Add control-plane keys for column presets and active preset
- [x] Add `ColumnPreset` model (order + visibility + widths)
- [x] Add encode/decode helpers for `gui.column_presets`
- [x] Load column presets and active preset at GUI startup
- [x] Add column preset save action in column chooser
- [x] Add column preset apply action
- [x] Add column preset delete action
- [x] Persist column presets and active preset during runtime config sync
- [x] Apply active column preset on startup when configured

## Notes
- This tranche completes remaining `library.md` items for grouping, inline editing, conditional formatting, shelf view, and column presets.
