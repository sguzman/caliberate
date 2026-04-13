# GUI Shell Roadmap

## Window + Shell
- [x] Implement dockable pane layout system (library list, details, browser, jobs)
- [x] Add split view presets and persist active layout preset
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
- [x] Expand menu bar to full Calibre parity (all actions present)
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

## Shell Parity Refinement: Help + Reader Launch
- [x] Replace shell fetch-metadata placeholder action with wired metadata-download dialog entry point
- [x] Add shell action handler that opens selected book directly in reader
- [x] Add shell action handler that opens random visible book directly in reader
- [x] Make random-book handler select the chosen row before opening reader
- [x] Add explicit validation error message when shell view-book is invoked with no selection
- [x] Add explicit validation error message when shell random-book is invoked with zero visible books
- [x] Route shell view-book/random-book failures into preferences error surface
- [x] Add tracing info event for selected-book reader launch from shell action
- [x] Add tracing info event for random-book reader launch from shell action
- [x] Remove legacy shell "not implemented" toast path for fetch metadata / view / random actions

## Shell Parity Refinement: About + Help Surface
- [x] Add About dialog state to shell app state model
- [x] Add About dialog rendering entry point in the app frame render loop
- [x] Add Help menu action wiring to open About dialog instead of placeholder toast
- [x] Render About dialog with app version sourced from crate metadata
- [x] Render About dialog with current runtime mode (dev/prod) for support diagnostics
- [x] Render About dialog with active database path for support diagnostics
- [x] Render About dialog with cache root path for support diagnostics
- [x] Render About dialog with log directory path for support diagnostics
- [x] Add About dialog quick action button for opening user manual URL
- [x] Add About dialog quick action buttons for opening project homepage and issue tracker URLs

## Shell Parity Refinement: Config + Test Hardening
- [x] Add control-plane GUI property for user manual URL
- [x] Add control-plane GUI property for project homepage URL
- [x] Add control-plane GUI property for report-issue URL
- [x] Add default values for new GUI help/link URL properties in core config model
- [x] Add dev control-plane entries for new GUI help/link URL properties
- [x] Add library helper to open external URLs with status/toast feedback
- [x] Validate external URL open helper rejects empty URLs explicitly
- [x] Refactor random-book selection to deterministic helper function with seed input
- [x] Add unit test coverage for random index helper empty-input behavior
- [x] Add unit test coverage for random index helper modulo behavior
