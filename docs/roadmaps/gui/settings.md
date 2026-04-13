# GUI Settings Roadmap

## Preferences Parity
- [x] Add full Calibre preferences tree (all sections + subpanes)
- [x] Add section tabs for `Behavior`, `Look & Feel`, `Import/Export`, `Advanced`, `System`
- [x] Add search within preferences
- [x] Add per-section reset to defaults
- [x] Add export/import preferences to file

## Appearance + Behavior
- [x] Add theme chooser with preview
- [x] Add icon set selection
- [x] Add startup behaviors (open last library, restore tabs)
- [x] Add system tray behavior settings (when supported)
- [x] Add confirm-on-exit toggles for long jobs

## Advanced
- [x] Add advanced database/fts settings UI
- [x] Add server settings panel (bind, auth, TLS)
- [x] Add cache and storage settings UI
- [x] Add logging verbosity controls
- [x] Add proxy/network settings UI

## Preferences Hardening
- [x] Add explicit preferences tree pane model
- [x] Add pane-to-section mapping helpers
- [x] Add per-section pane listing helpers
- [x] Render preferences tree navigation sidebar
- [x] Split preferences layout into tree + content columns
- [x] Synchronize section tabs with active pane state
- [x] Synchronize open-section API helpers with active pane state
- [x] Add focused-pane status indicator in preferences UI
- [x] Make section pane collapsers open based on selected pane
- [x] Add unit tests for pane mapping and pane coverage across sections

## Preferences Regression Hardening
- [x] Add unit test coverage for pane labels not-empty invariant
- [x] Ensure section-tab selection updates active pane consistently
- [x] Ensure open-section helper methods update active pane defaults
- [x] Ensure tree selection updates both pane and section state
- [x] Ensure tree filtering considers section and pane labels
- [x] Keep reset-section action in content pane after tree refactor
- [x] Keep search-clear action functional after tree refactor
- [x] Keep import/export preferences controls functional in content pane layout
- [x] Keep editable/read-only mode transitions intact after two-column layout split
- [x] Preserve restart-required banner behavior after UI layout changes
- [x] Preserve section validation and save flow behavior after tree navigation wiring
- [x] Preserve tracing on section reset actions
- [x] Keep pane-aware default-open behavior for behavior section subpanes
- [x] Keep pane-aware default-open behavior for advanced/system subpanes
- [x] Keep pane-aware default-open behavior for look-and-feel/import-export subpanes
