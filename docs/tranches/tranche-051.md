# Tranche 051

## Scope (30 items)
- [x] Add `LibraryView::ingest_paths_now` helper for non-dialog ingest triggers
- [x] Wire dropped file paths in shell to immediate ingest execution
- [x] Add tracing event for dropped file ingest attempts
- [x] Add tracing warning for dropped file ingest failures
- [x] Add `Open library…` menu action in `File`
- [x] Add `Create library…` menu action in `File`
- [x] Add `Switch library…` menu action in `File`
- [x] Add `Recent libraries` submenu in `File`
- [x] Add library switcher modal listing recent library entries
- [x] Add open-library modal with direct sqlite-path input
- [x] Add create-library modal with directory input and db path derivation
- [x] Ensure library db parent directory exists before switching libraries
- [x] Reinitialize `LibraryView` after library switch
- [x] Track and persist active library label after switching
- [x] Keep MRU ordering for `recent_libraries`
- [x] Enforce configured cap for stored recent libraries
- [x] Show active library label in top shell strip
- [x] Show known-library count in top shell strip
- [x] Show active db path in status bar
- [x] Add `Device` menu with send-to-device dispatch action
- [x] Add `Tools` menu actions for tags/series/custom-columns/virtual-libraries
- [x] Add `News` shell menu placeholder for future feed actions
- [x] Add Alt+Left/Alt+Right navigation shortcuts (back/forward)
- [x] Add right-mouse gesture navigation handling (back/forward)
- [x] Add tracing for mouse-gesture navigation dispatch
- [x] Persist `mouse_gestures` toggle from shell controls
- [x] Capture runtime window geometry and persist width/height/position
- [x] Add `Restore window on launch` toggle in View menu
- [x] Apply persisted window geometry at GUI startup via `NativeOptions.viewport`
- [x] Centralize default db/recent-library/conversion/plugin/device paths under `./.cache/caliberate`

## Notes
- This tranche closes remaining shell fundamentals around multi-library workflows, drag-drop ingest entry points, and window/input persistence.
