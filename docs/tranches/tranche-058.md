# Tranche 058

## Scope (30 items)
- [x] Add metadata edit state fields for publication date parts (`pubdate_year`, `pubdate_month`, `pubdate_day`)
- [x] Populate publication date helper fields from existing `pubdate` during `EditState::from_details`
- [x] Add publication date helper sync/reset behavior tied to metadata baseline state
- [x] Add keyboard shortcut handling in metadata editor for save (`Ctrl/Cmd+S`)
- [x] Add keyboard shortcut handling in metadata editor for cancel (`Esc`)
- [x] Add keyboard shortcut handling in metadata editor for undo (`Ctrl/Cmd+R`)
- [x] Add keyboard shortcut handling in metadata editor for UUID generation (`Ctrl/Cmd+G`)
- [x] Add keyboard shortcut handling in metadata editor for identifier copy (`Ctrl/Cmd+I`)
- [x] Surface metadata editor shortcut cheat-sheet in dialog header
- [x] Add tooltip help text to title field label
- [x] Add tooltip help text to authors field label
- [x] Add tooltip help text to title-sort field label
- [x] Add tooltip help text to publication helper controls
- [x] Add duplicate-author inline warning hint in metadata editor
- [x] Add identifier conflict inline warning hint in metadata editor
- [x] Add language token inline warning hint in metadata editor
- [x] Add publication helper row with Y/M/D drag editors and apply action
- [x] Add publication helper action to sync Y/M/D fields from free-form publication text
- [x] Add publication helper action to update Y/M/D when using `Today` shortcut
- [x] Add validation summary optimization to avoid recomputing issues twice per frame
- [x] Add shared duplicate CSV detection helper for metadata hint/validation paths
- [x] Add shared identifier conflict detection helper for metadata hint/validation paths
- [x] Add shared language token hint helper for metadata hint/validation paths
- [x] Add shared pubdate parsing helper supporting RFC3339 and `YYYY-MM-DD`
- [x] Extend validation issue collection to include duplicate author conflicts
- [x] Extend validation issue collection to include duplicate tag conflicts
- [x] Extend validation issue collection to include language token warnings
- [x] Extend validation issue collection to include identifier conflict warnings
- [x] Refine metadata roadmap by checking off keyboard shortcut parity and field-level hint coverage
- [x] Refine metadata roadmap with new remaining items for publish-slot persistence and conflict auto-resolution

## Notes
- This tranche completed keyboard navigation + inline guidance/hints for metadata editing and added publication-date helper mechanics.
- Full metadata parity umbrella remains open and now has narrower remaining sub-items in `docs/roadmaps/gui/metadata.md`.
