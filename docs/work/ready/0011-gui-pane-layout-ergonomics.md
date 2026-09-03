# 0011 — Make the library shell panes resizable, collapsible, and center-safe

## Why

Runtime testing with real books exposed a severe usability defect in the main library shell: the Details pane can consume most of the window and the user cannot conveniently collapse it or recover enough space for the library browser.

The current layout also renders the Library itself as a left `egui::Panel` while the right Details pane is another side panel; the remaining `CentralPanel` is largely a placeholder. This makes auxiliary pane widths compete directly with the primary library surface and allows stale persisted widths to starve the library.

This task fixes the shell geometry before any further query/pagination work.

## Product rule

The **Library is the primary surface and owns the center**.

Browser and Details are auxiliary panes. Every major auxiliary side pane must be:

- directly resizable by dragging its separator;
- directly collapsible from its own header;
- restorable from the existing layout controls;
- bounded so it cannot starve the central library surface;
- persisted at its last useful width;
- robust against absurd/stale persisted width values.

Do not solve this only by changing a default width.

## Current baseline

In `crates/gui/src/views.rs`, `LibraryView::ui` currently:

- computes `left_width` from `self.left_pane_width`;
- may create right Browser/Details panels with `resizable(true)` and `default_size(self.right_pane_width...)`;
- creates the **Library** with `egui::Panel::left("library_list")` and `default_size(left_width)`;
- uses the remaining `egui::CentralPanel` mostly for placeholder text;
- persists `left_pane_width` / `right_pane_width` to GUI config.

The tracked config currently permits very large widths and runtime state may contain values much larger than are reasonable for the current window.

`details_view` also renders long values such as the full source path with ordinary labels, so details content must not be allowed to force the side pane wider than the user requested.

## Scope

### 1. Put the Library in the center

Refactor the shell geometry in `LibraryView::ui` so the table/grid/shelf library surface is rendered in the remaining central area rather than inside a persistent left side panel.

The central library area must contain the existing library-facing content that is currently inside the `library_list` panel, including the normal library heading/controls/view/status behavior as appropriate.

Do not duplicate the library view into two render paths.

The final geometry should conceptually be:

```text
+---------------------------------------------------------------+
| existing top-level application chrome                         |
+------------------+---------------------------+----------------+
| optional Browser | CENTRAL LIBRARY           | optional       |
| auxiliary pane   | table / grid / shelf      | Details pane   |
| resizable        |                           | resizable       |
| collapsible      |                           | collapsible     |
+------------------+---------------------------+----------------+
| existing jobs/status behavior as applicable                   |
+---------------------------------------------------------------+
```

Browser/Details may still honor the existing configured left/right docking side. Do not remove docking-side preferences.

### 2. Add direct collapse controls to auxiliary pane headers

For visible Browser and Details side panes, add an obvious compact collapse/close affordance in the pane header itself.

Examples are acceptable:

- `×`
- `‹` / `›`
- a small text/icon button with tooltip

Clicking it must set the corresponding `browser_visible` / `details_visible` state false and mark layout/config state dirty so the change persists through the existing config-save path.

The user must not have to hunt through Preferences or a layout menu merely to reclaim space.

The existing layout controls/toggles must remain capable of restoring a hidden pane.

### 3. Make side-pane resizing actually useful

Keep Browser/Details side panes resizable.

Use explicit runtime min/max bounds appropriate for auxiliary panes. Suggested targets:

- auxiliary pane minimum: roughly 180–220 px;
- normal default Details width: roughly 320–420 px;
- normal default Browser width: roughly 220–320 px.

Exact constants may be named helpers/constants rather than magic values spread across the function.

Do **not** retain a runtime minimum as large as 280/320 if that prevents the user from reclaiming the library area on a normal laptop-sized window.

### 4. Guarantee central-library space

Introduce a small pure helper or clearly isolated calculation that clamps auxiliary pane widths against current available width.

A stale persisted width such as 900, 1600, or 2000 px must not leave the central library with only a sliver.

On a normal 1400 px-wide window, the central library should retain a substantial usable width (target at least ~480 px when physically possible).

When the window is narrower, degrade gracefully: shrink auxiliary panes toward their minimum before sacrificing the central library.

Do not permanently overwrite the user's preferred stored width merely because the window was temporarily small unless the pane was actually resized by the user. Runtime clamping and persisted preference should be conceptually distinct where practical.

### 5. Persist user resizing

When a Browser or Details pane is visibly resized by the user, update the corresponding stored width state and route it through the existing layout/config persistence mechanism.

Do not continuously rewrite config every frame if the existing `layout_dirty` / config-dirty machinery can be used.

The saved width should be the useful pane width, not a central-library width.

Existing `ShellPaneLayout` behavior should remain coherent. If `left_pane_width` / `right_pane_width` semantics need to be clarified so they mean auxiliary left/right pane widths, make the smallest consistent change and document it in the report.

### 6. Prevent Details content from forcing width

Audit the immediately visible Details content for long unbroken/wide fields, especially:

- full filesystem path;
- title;
- authors/tags;
- identifiers/comments if rendered as single-line labels.

Use wrapping, truncation-with-hover, selectable wrapped text, or another standard egui approach so content respects the pane width.

Do not hide the underlying value permanently; it should remain inspectable/copyable where reasonable.

Do not implement a full details-panel redesign in this task.

### 7. Keep existing behavior intact

Preserve:

- table/grid/shelf views;
- book selection;
- details loading/actions;
- Browser include/exclude controls;
- quick-details behavior;
- jobs rendering;
- layout presets and side-selection controls;
- config persistence;
- all library-service read-path work from task 0010.

## Tests

Add focused tests for any pure width-clamping/layout helper introduced.

At minimum prove:

1. an absurd persisted auxiliary width is clamped so a 1400 px window retains the configured central minimum;
2. a normal requested pane width is preserved when there is enough room;
3. a narrow window shrinks auxiliary width toward its minimum without negative/invalid geometry;
4. left/right calculations are deterministic and do not depend on filesystem/database state.

Do not attempt brittle pixel-perfect egui snapshot tests if the project does not already have infrastructure for them.

## Acceptance criteria

1. The primary library table/grid/shelf occupies the central panel/remaining central area.
2. Details can be collapsed directly from the Details pane header.
3. Browser can be collapsed directly from the Browser pane header.
4. Details and Browser separators can be dragged to resize.
5. Long Details text does not force the panel wider than its configured/runtime maximum.
6. Huge stale persisted widths cannot starve the central library view.
7. User resizing is persisted through the existing config/layout mechanism.
8. Existing library browsing, selection, details actions, Browser filtering, and view modes still compile and tests pass.
9. No unrelated query/service/schema work is included.

## Explicit non-goals

Do **not**:

- implement task 0011's previously planned library sort-parity work;
- add service-backed GUI pagination;
- change query/filter/facet semantics;
- redesign the entire Details presentation;
- redesign Preferences;
- redesign jobs UI;
- add docking libraries or a new GUI framework;
- add dependencies;
- split the entire `views.rs` god file in this task;
- change reader/TTS/conversion/server behavior;
- clean unrelated warnings.

The library-query sort-parity task remains next in the roadmap after this usability fix and will be re-queued under the next available task ID.

## Files expected to change

Expected primarily:

- `crates/gui/src/views.rs`
- possibly `crates/core/src/config.rs` only if pane-width validation/defaults require a narrowly justified correction
- possibly `config/control-plane.toml` only for sane default pane widths
- focused GUI/unit tests if applicable
- `docs/work/reports/0011.md`
- move this task from `docs/work/ready/` to `docs/work/done/`

Avoid unrelated files.

## Validation

Run on native Windows:

```text
cargo fmt --check
cargo test -p caliberate-gui
cargo check --workspace --locked
cargo test --workspace --locked
```

If a focused GUI test target exists that is more appropriate, run it in addition to the above.

Existing unrelated warning backlog may remain.

## Human verification required

The implementation worker may not have desktop-control capability. If interactive GUI control is unavailable, say so explicitly in the report rather than claiming runtime validation.

The human will verify after merge:

1. launch `caliberate-gui` with real books;
2. drag Details narrower and wider;
3. collapse Details from its own header;
4. restore it using existing layout controls;
5. resize/collapse Browser;
6. confirm the library remains usable at normal and narrower window sizes;
7. restart and confirm pane visibility/size persistence.

## Repository handoff

Write `docs/work/reports/0011.md` with:

- summary;
- exact layout geometry changes;
- width/min/max constants or helper semantics;
- persistence behavior;
- files changed;
- validation actually run and results;
- interactive GUI behavior not verified by the worker, if applicable;
- deviations/blockers.

Move this task to:

- `docs/work/done/0011-gui-pane-layout-ergonomics.md`

Commit and push exactly one bounded implementation branch:

- `codex/0011-gui-pane-layout-ergonomics`

Do not work on any other task.
