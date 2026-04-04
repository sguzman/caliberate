# GUI Roadmap

## Discovery + Parity Mapping
- [x] Inventory Calibre GUI feature set and major views in Calibre reference source
- [x] Define GUI parity targets and feature mapping

## App Shell + Navigation
- [x] Implement core window shell and application lifecycle
- [x] Implement top-level navigation between Library and Preferences
- [x] Implement status bar with error surfacing
- [x] Implement toolbar with Calibre-style actions (Add, Remove, Convert, Save to Disk, etc.)
- [x] Implement menu bar parity (File/Library/Edit/Convert/View/Preferences/Help)
- [x] Implement keyboard shortcuts for primary actions

## Library List (Books View)
- [x] Implement basic library list backed by DB
- [x] Implement basic list refresh
- [x] Implement basic search box (title/author search)
- [x] Implement basic sort (title/format/id)
- [x] Implement basic format filter
- [x] Implement column-based table view (Title, Authors, Series, Tags, Formats, Rating, etc.)
- [x] Implement column visibility management
- [x] Implement column sorting parity (single-column)
- [x] Implement column sorting parity (multi-column / stable sort)
- [x] Add secondary sort selector UI
- [x] Add stable tie-breaker sorting for multi-column
- [x] Implement column resizing and persistence
- [x] Persist column visibility to control-plane config
- [x] Persist column widths to control-plane config
- [x] Persist list view mode to control-plane config
- [x] Implement row selection with multi-select and range select
- [x] Implement inline quick-search with highlights
- [x] Implement virtualized list rendering for large libraries
- [x] Implement per-book cover thumbnails in list view
- [x] Render cover placeholders in grid view
- [x] Use gui.table_row_height for list rows
- [x] Implement list view mode toggles (cover grid vs table)
- [x] Implement library statistics footer (count, virtual libraries)

## Book Details Pane
- [x] Implement basic book details view (metadata + assets list)
- [x] Implement basic metadata edit flow (title/authors/tags/series/identifiers/comment)
- [x] Implement Calibre-style metadata editor dialog layout
- [ ] Implement cover preview/editing (set/remove/generate cover)
- [ ] Implement comments rich text editor (HTML/Markdown parity)
- [x] Add cover thumbnail column and placeholders
- [x] Add cover preview placeholder in details pane
- [x] Add cover action buttons (set/remove/generate)
- [x] Wire cover actions to has_cover flag updates
- [x] Add comment preview pane in metadata editor
- [x] Add comment preview formatting for headings/bullets
- [x] Implement identifiers editor with validation
- [x] Implement tags editor with autocomplete
- [x] Implement series editor with index controls parity
- [x] Implement ratings editor with star UI
- [x] Implement languages editor with locale picker
- [x] Implement publisher/ISBN/UUID fields parity
- [x] Implement formats list with per-format actions (open, remove, convert)
- [x] Implement book folder/path actions (open folder, open file)

## Library Operations
- [x] Implement add books workflow (files/folders, copy vs reference)
- [x] Add books dialog with file list input
- [x] Add books folder ingest with format filtering
- [x] Add books ingest mode selection (copy/reference)
- [x] Add books archive reference toggle
- [x] Add books DB insertion + refresh status
- [x] Implement remove books workflow (delete vs remove from db)
- [x] Remove books confirmation dialog with selection count
- [x] Remove books delete stored files option
- [x] Remove books delete reference files option
- [x] Remove books DB deletion + refresh
- [x] Implement edit metadata in bulk
- [x] Bulk edit dialog for multi-select
- [x] Bulk edit tags (append/replace)
- [x] Bulk edit series name/index
- [x] Bulk edit publisher set/clear
- [x] Bulk edit languages
- [x] Bulk edit rating
- [x] Implement convert books workflow and progress
- [x] Convert dialog output format + output dir
- [x] Convert handles compressed assets
- [x] Convert add-to-library option
- [x] Convert status/toast update
- [x] Implement save to disk/export workflow
- [x] Save to disk dialog output dir + export all formats toggle
- [x] Save to disk handles compressed assets
- [x] Save to disk status/toast update
- [x] Implement device sync workflow surface
- [x] Device sync dialog with detected devices
- [x] Device sync send selected assets with optional name override
- [x] Implement tags/series management dialogs
- [x] Tags management list + rename/delete actions
- [x] Series management list + rename/delete actions
- [x] Implement custom columns management UI
- [x] Custom columns management list/create/delete
- [x] Implement virtual library management UI
- [x] Virtual library saved searches list/add/remove

## Preferences
- [x] Add read-only preferences view from control-plane
- [x] Implement editable preferences and persistence
- [x] Implement preferences sections parity (behavior, look & feel, import/export, advanced)
- [x] Add preferences section tabs
- [x] Group preferences into section views (system, behavior, look & feel, import/export, advanced)
- [x] Implement validation + inline errors for editable preferences
- [x] Add GUI layout section in Preferences
- [x] Add inline validation messages for server/conversion/FTS fields
- [x] Implement preferences restart-required messaging
- [x] Show GUI cover/toast settings in Preferences

## Reader / Viewer
- [ ] Implement reader/viewer integration
- [x] Implement open-with-external-viewer option
- [ ] Implement in-app reader controls (font, theme, navigation)

## Jobs + Progress
- [x] Implement unified job queue UI (conversions, metadata, imports)
- [x] Add in-memory job queue model
- [x] Add job enqueue hooks for toolbar actions
- [x] Add job progress simulation
- [x] Add job pause/resume controls
- [x] Add job cancel controls
- [x] Implement per-job progress with cancel/pause
- [x] Implement background task notifications/toasts
- [x] Add toast model and queue
- [x] Render toast overlay notifications
- [x] Auto-dismiss toasts by duration

## Error + Telemetry UX
- [x] Implement user-facing error dialogs with copyable details
- [x] Add error dialog copy-to-clipboard action
- [x] Add error dialog dismiss clears active errors
- [x] Implement log viewer / open logs action in UI
