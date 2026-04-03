# GUI Roadmap

## Discovery + Parity Mapping
- [x] Inventory Calibre GUI feature set and major views in Calibre reference source
- [x] Define GUI parity targets and feature mapping

## App Shell + Navigation
- [x] Implement core window shell and application lifecycle
- [x] Implement top-level navigation between Library and Preferences
- [x] Implement status bar with error surfacing
- [ ] Implement toolbar with Calibre-style actions (Add, Remove, Convert, Save to Disk, etc.)
- [ ] Implement menu bar parity (File/Library/Edit/Convert/View/Preferences/Help)
- [ ] Implement keyboard shortcuts for primary actions

## Library List (Books View)
- [x] Implement basic library list backed by DB
- [x] Implement basic list refresh
- [x] Implement basic search box (title/author search)
- [x] Implement basic sort (title/format/id)
- [x] Implement basic format filter
- [ ] Implement column-based table view (Title, Authors, Series, Tags, Formats, Rating, etc.)
- [ ] Implement column visibility management
- [ ] Implement column sorting parity (multi-column / stable sort)
- [ ] Implement column resizing and persistence
- [ ] Implement row selection with multi-select and range select
- [ ] Implement inline quick-search with highlights
- [ ] Implement virtualized list rendering for large libraries
- [ ] Implement per-book cover thumbnails in list view
- [ ] Implement list view mode toggles (cover grid vs table)
- [ ] Implement library statistics footer (count, virtual libraries)

## Book Details Pane
- [x] Implement basic book details view (metadata + assets list)
- [x] Implement basic metadata edit flow (title/authors/tags/series/identifiers/comment)
- [ ] Implement Calibre-style metadata editor dialog layout
- [ ] Implement cover preview/editing (set/remove/generate cover)
- [ ] Implement comments rich text editor (HTML/Markdown parity)
- [ ] Implement identifiers editor with validation
- [ ] Implement tags editor with autocomplete
- [ ] Implement series editor with index controls parity
- [ ] Implement ratings editor with star UI
- [ ] Implement languages editor with locale picker
- [ ] Implement publisher/ISBN/UUID fields parity
- [ ] Implement formats list with per-format actions (open, remove, convert)
- [ ] Implement book folder/path actions (open folder, open file)

## Library Operations
- [ ] Implement add books workflow (files/folders, copy vs reference)
- [ ] Implement remove books workflow (delete vs remove from db)
- [ ] Implement edit metadata in bulk
- [ ] Implement convert books workflow and progress
- [ ] Implement save to disk/export workflow
- [ ] Implement device sync workflow surface
- [ ] Implement tags/series management dialogs
- [ ] Implement custom columns management UI
- [ ] Implement virtual library management UI

## Preferences
- [x] Add read-only preferences view from control-plane
- [x] Implement editable preferences and persistence
- [ ] Implement preferences sections parity (behavior, look & feel, import/export, advanced)
- [ ] Implement validation + inline errors for editable preferences
- [ ] Implement preferences restart-required messaging

## Reader / Viewer
- [ ] Implement reader/viewer integration
- [ ] Implement open-with-external-viewer option
- [ ] Implement in-app reader controls (font, theme, navigation)

## Jobs + Progress
- [ ] Implement unified job queue UI (conversions, metadata, imports)
- [ ] Implement per-job progress with cancel/pause
- [ ] Implement background task notifications/toasts

## Error + Telemetry UX
- [ ] Implement user-facing error dialogs with copyable details
- [ ] Implement log viewer / open logs action in UI
