# Tranche 047

## Scope (30 items)
- [x] Add config key `gui.show_format_badges`
- [x] Add config key `gui.show_language_badges`
- [x] Add config key `gui.active_virtual_library`
- [x] Add config key `gui.virtual_library_filters`
- [x] Validate `gui.active_virtual_library` when set
- [x] Validate `gui.virtual_library_filters` keys are non-empty
- [x] Validate `gui.virtual_library_filters` entries are non-empty
- [x] Load `show_format_badges` from control-plane into GUI state
- [x] Load `show_language_badges` from control-plane into GUI state
- [x] Decode persisted virtual-library filters from control-plane
- [x] Initialize active virtual library from control-plane
- [x] Restore active virtual-library filters at startup
- [x] Sync active virtual library back into control-plane runtime config
- [x] Sync virtual-library filters back into control-plane runtime config
- [x] Auto-save control-plane when GUI runtime config is dirty
- [x] Mark GUI config dirty when cycling browser filters
- [x] Mark GUI config dirty when removing browser filter chips
- [x] Mark GUI config dirty when changing active virtual library
- [x] Persist browser filter map when clearing filters
- [x] Add active virtual-library selector UI in browser pane
- [x] Apply selected virtual-library saved search query into library search
- [x] Add virtual-library indicator segment in status bar
- [x] Update status-bar saved-search quick access to switch virtual library
- [x] Add helper to encode browser filter entries for persistence
- [x] Add helper to decode browser filter entries for persistence
- [x] Add helper to encode virtual-library filter map for config
- [x] Add helper to decode virtual-library filter map from config
- [x] Add layout toggle for format badges
- [x] Add layout toggle for language badges
- [x] Render format/language badges in table list view

## Notes
- Virtual-library filter sets now persist across app restarts via `control-plane.toml`.
