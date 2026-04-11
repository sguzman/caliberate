# Tranche 046

## Scope (30 items)
- [x] Replace single browser filter state with multi-filter state
- [x] Add browser filter include mode
- [x] Add browser filter exclude mode
- [x] Add browser filter click cycle include -> exclude -> clear
- [x] Apply all browser filters cumulatively in library filtering
- [x] Implement include-mode matching semantics
- [x] Implement exclude-mode matching semantics
- [x] Add browser filter chip rendering in filter summary
- [x] Add include/exclude badges in filter chip labels
- [x] Add per-chip browser filter removal action
- [x] Clear all browser filters from summary action
- [x] Add active virtual library selector in browser pane
- [x] Apply selected virtual library saved query to search input
- [x] Add virtual library indicator in status bar
- [x] Persist browser filters per active virtual library (session scope)
- [x] Restore virtual library filter set when switching libraries
- [x] Clear active virtual library filters when saved search is removed
- [x] Add hierarchical browser labels for tag-like paths
- [x] Add hierarchical browser labels for series-like paths
- [x] Add per-view density selector (compact/comfortable)
- [x] Scale table row height from density selection
- [x] Scale grid columns count from density selection
- [x] Add quick details panel toggle in layout controls
- [x] Render quick details panel for selected book metadata
- [x] Add grid cover zoom slider
- [x] Add per-row quick action buttons (edit/remove/convert) in table title column
- [x] Add detailed library stats panel (formats/languages/tags/authors/series)
- [x] Add library stats export to CSV
- [x] Add config key `gui.view_density`
- [x] Add config key `gui.quick_details_panel`

## Notes
- Virtual library browser filter persistence is currently in-memory for the running session; restart persistence remains tracked separately.
