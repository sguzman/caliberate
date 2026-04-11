# Tranche 050

## Scope (30 items)
- [x] Add shell command palette toggle action
- [x] Add command palette modal window
- [x] Add command palette query filter input
- [x] Add command palette command list for core shell actions
- [x] Wire command palette command execution to `AppAction`
- [x] Add view navigation actions (`Back`, `Forward`)
- [x] Add navigation menu entries for back/forward
- [x] Track navigation history stacks on view switch
- [x] Add top-level busy indicator using active job count
- [x] Expose active background job count from `LibraryView`
- [x] Add shell notification center toggle button
- [x] Add notification center panel listing recent toasts
- [x] Expose recent toast messages from `LibraryView`
- [x] Add global search bar in shell header
- [x] Add global search scope selector (`all/title/authors/tags/series`)
- [x] Bridge global search query/scope into library filtering API
- [x] Add global search result count segment
- [x] Add global search clear action
- [x] Add scoped search support inside library search path
- [x] Add toolbar customization flags (show/hide per action)
- [x] Add toolbar overflow menu for compact widths
- [x] Add icon-only toolbar rendering mode
- [x] Add shortcut editor toggle action and modal
- [x] Add per-action shortcut editing controls
- [x] Add shortcut conflict detection indicator
- [x] Add shortcut presets (`default`, `calibre_like`)
- [x] Add drag-drop hover hint overlay in main shell
- [x] Add control-plane keys for shell toolbar and search preferences
- [x] Add control-plane keys for command palette/notification center/drag-drop hints
- [x] Persist shell UI settings back to `control-plane.toml`

## Notes
- This tranche advances `gui/shell` parity for command workflows, global search shell controls, shortcuts, notifications, and toolbar behavior.
