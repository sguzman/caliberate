# Tranche 048

## Scope (30 items)
- [x] Extend `BookRow` with `date_added`, `date_modified`, and `pubdate` fields
- [x] Populate row date fields from cached `BookExtras` metadata
- [x] Add `SortMode::DateAdded`
- [x] Add `SortMode::DateModified`
- [x] Add `SortMode::PubDate`
- [x] Add date sort options to primary sort selector
- [x] Add date sort options to secondary sort selector
- [x] Extend row compare logic to sort by added/modified/pubdate
- [x] Add `ColumnKey` model for unified table column ordering
- [x] Add column order decode helper from `gui.column_order`
- [x] Add default column order including new date columns
- [x] Add column chooser search input
- [x] Add column chooser move-up action
- [x] Add column chooser move-down action
- [x] Add column chooser reset order action
- [x] Refactor table rendering to use ordered visible column list
- [x] Add column visibility support for added date column
- [x] Add column visibility support for modified date column
- [x] Add column visibility support for pubdate column
- [x] Add width controls for added/modified/pubdate columns
- [x] Add sort preset model and in-memory state
- [x] Add save sort preset action in sort controls
- [x] Add apply sort preset action from combo selector
- [x] Add delete active sort preset action
- [x] Persist `gui.sort_presets` and `gui.active_sort_preset`
- [x] Add per-format stored-size aggregation in library stats
- [x] Render per-format stored-size chart rows in stats panel
- [x] Add per-author stats drilldown action into browser filters
- [x] Add per-series stats drilldown action into browser filters
- [x] Add `gui.stats_top_n` to cap chart/drilldown row counts

## Notes
- This tranche completes the checked `library.md` items for column chooser, sort presets, date columns, and stats drilldowns.
