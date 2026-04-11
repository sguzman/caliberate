# GUI Shell Roadmap

## Window + Shell
- [ ] Implement dockable pane layout system (library list, details, browser, jobs)
- [ ] Add split view presets and persist active layout preset
- [x] Add global command palette / quick actions
- [x] Add command palette modal with query filter input
- [x] Add command palette action execution for core shell actions
- [x] Add command palette toggle action in View menu
- [x] Implement view-level navigation history (back/forward)
- [x] Add Navigate menu with Back/Forward actions
- [x] Track view history stack when switching Library/Preferences
- [x] Add app-wide busy indicator and background task badge
- [x] Show active background job count in top shell strip
- [x] Add recent libraries menu and library switcher dialog
- [x] Add create/open library workflows in UI
- [x] Persist window size/position and restore on launch
- [x] Add drag-and-drop file ingest target on main window
- [x] Add multi-library indicator in title bar/status

## Menus + Toolbars
- [ ] Expand menu bar to full Calibre parity (all actions present)
- [x] Add Device menu and wire Send to device action
- [x] Add Tools menu entries for tag/series/custom-column/virtual-library management
- [x] Add News menu section placeholder to anchor feed actions in shell
- [x] Implement toolbar customization (show/hide actions)
- [x] Add toolbar visibility toggles in View menu
- [x] Add toolbar overflow handling for small windows
- [x] Add toolbar overflow menu for compact widths
- [x] Add toolbar action tooltips with shortcut hints
- [x] Add toolbar separators and action groups
- [x] Add toolbar icon-only vs text+icon toggle
- [x] Persist toolbar icon-only and visible actions in control-plane config

## Global Search + Status
- [x] Add global search entry with scope selector (title/author/tags/series)
- [x] Add global search bar in shell header
- [x] Bridge global search scope and query into library view filtering
- [x] Add global search result count + clear action
- [x] Persist selected global search scope in control-plane config
- [x] Add status bar segments for jobs, selection count, and library stats
- [x] Add notification center panel for recent toasts
- [x] Add notification center toggle in shell controls
- [x] Add search history dropdown with recent queries
- [x] Add saved search quick access in status bar

## Shortcuts + Input
- [x] Implement shortcut editor UI with conflict detection
- [x] Add shortcut editor modal with per-action key/modifier controls
- [x] Add keyboard shortcut presets (default + Calibre-like)
- [x] Add shortcut preset actions and conflict status indicator
- [x] Persist selected shortcut preset in control-plane config
- [x] Add mouse gesture support for navigation (back/forward)
- [x] Add drag target hints for drop zones (add books, covers)
- [x] Add drag-hover overlay hint for ingest drop target
- [x] Persist drag-drop hint enablement in control-plane config
- [x] Persist mouse gesture and window restore toggles in control-plane config
