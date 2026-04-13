# Tranche 066 - Shell Help + Reader Launch Parity

## Scope
- Roadmap source: `docs/roadmaps/gui/shell.md`
- Focus: close remaining shell action placeholders and harden help/about integration through control-plane config.

## Completed Items (30)
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

## Notes
- The new help URLs are centralized in the control plane (`[gui]`), not hardcoded in app logic.
- Shell Help actions now execute concrete behavior with explicit tracing and error propagation.
