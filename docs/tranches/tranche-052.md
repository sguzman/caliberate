# Tranche 052

## Scope (30 items)
- [x] Add `GuiConfig` keys for browser/details/jobs pane visibility
- [x] Add `GuiConfig` keys for browser/details pane side placement
- [x] Add `GuiConfig` keys for pane split widths
- [x] Add `GuiConfig` key for active layout preset
- [x] Add config defaults for all new pane/layout keys
- [x] Add config validation for pane side values
- [x] Add config validation for pane width ranges
- [x] Add config validation for layout preset values
- [x] Add new pane/layout keys to `config/control-plane.toml`
- [x] Add new pane/layout keys to core config fixture
- [x] Introduce `PaneSide` type in GUI views
- [x] Introduce `ShellPaneLayout` transport struct in GUI views
- [x] Store pane visibility/side/width runtime state in `LibraryView`
- [x] Expose `LibraryView::set_shell_layout` to apply shell layout decisions
- [x] Expose `LibraryView::shell_layout` to read back user-resized pane widths
- [x] Render browser pane on right side when configured
- [x] Gate browser controls visibility by pane toggle
- [x] Render details pane on right side when configured
- [x] Render details pane on left rail when configured
- [x] Gate jobs panel rendering by pane toggle
- [x] Track left pane width from resizable panel response
- [x] Track right pane width from resizable panel response
- [x] Add shell state for pane layout in `CaliberateApp`
- [x] Sync shell pane layout state into `LibraryView` each frame
- [x] Pull pane width state back from `LibraryView` for persistence
- [x] Add layout preset application helper (`classic`, `focus`, `minimal`, `wide`)
- [x] Persist pane layout config back to control-plane TOML
- [x] Expand menu bar with metadata actions (download/fetch/cover placeholders)
- [x] Expand menu bar with preferences and help subsection actions
- [x] Add File menu actions for view-book and random-book placeholders

## Notes
- This tranche closes the remaining shell roadmap items by implementing configurable pane layout, split presets, and expanded top-level menu surface.
