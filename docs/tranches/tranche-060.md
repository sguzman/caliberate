# Tranche 060

## Scope (30 items)
- [x] Extend `DeviceConfig` with send behavior controls (`send_auto_convert`, `send_overwrite`) in `crates/core/src/config.rs`
- [x] Extend `DeviceConfig` with sync controls (`sync_metadata`, `sync_cover`) in `crates/core/src/config.rs`
- [x] Extend `DeviceConfig` with driver/timeout controls (`scan_recursive`, `driver_backend`, `connection_timeout_ms`) in `crates/core/src/config.rs`
- [x] Add validation for `device.driver_backend` allowed values in `crates/core/src/config.rs`
- [x] Add validation for `device.connection_timeout_ms` bounds in `crates/core/src/config.rs`
- [x] Add top-level `NewsConfig` to control plane with serde defaults in `crates/core/src/config.rs`
- [x] Add `NewsConfig` validation for retention and fetch limits in `crates/core/src/config.rs`
- [x] Add `Default` implementation for `NewsConfig` in `crates/core/src/config.rs`
- [x] Add news default providers/paths/source maps in `crates/core/src/config.rs`
- [x] Ensure runtime path setup creates news recipes/download directories in `crates/core/src/paths.rs`
- [x] Ensure runtime path setup creates news history parent directory in `crates/core/src/paths.rs`
- [x] Add new `[device]` and `[news]` keys to `config/control-plane.toml`
- [x] Update device detection test fixture for expanded `DeviceConfig` fields in `crates/device/tests/detection.rs`
- [x] Extend `DeviceSyncDialogState` with send options and queue rows in `crates/gui/src/views.rs`
- [x] Add send options controls (auto convert/overwrite/sync metadata/sync cover) to send-to-device dialog in `crates/gui/src/views.rs`
- [x] Add send queue/progress rendering to send-to-device dialog in `crates/gui/src/views.rs`
- [x] Add device manager dialog state and open workflow in `crates/gui/src/views.rs`
- [x] Add device sidebar list and selection workflow in device manager dialog in `crates/gui/src/views.rs`
- [x] Add device library file list with search/filter in device manager dialog in `crates/gui/src/views.rs`
- [x] Add device storage usage stats and collections shelf summary in `crates/gui/src/views.rs`
- [x] Add fetch-from-device dialog and import action using ingest pipeline in `crates/gui/src/views.rs`
- [x] Add device-side delete confirmation dialog and file deletion workflow in `crates/gui/src/views.rs`
- [x] Add connection troubleshooting panel (mount/library existence + writable checks) in `crates/gui/src/views.rs`
- [x] Add device driver configuration UI controls in device manager dialog in `crates/gui/src/views.rs`
- [x] Add news manager dialog state/open workflow and wire into operations UI in `crates/gui/src/views.rs`
- [x] Add news source list UI with enable toggles, scheduling labels, and source filter in `crates/gui/src/views.rs`
- [x] Add custom recipe import workflow into configured recipe directory in `crates/gui/src/views.rs`
- [x] Add news download execution flow with per-source status/history logging in `crates/gui/src/views.rs`
- [x] Add downloaded news list + retry action + open-in-reader action in `crates/gui/src/views.rs`
- [x] Add news collection grouping support via `news-only` library filter and retention settings UI with auto-prune in `crates/gui/src/views.rs`

## Notes
- Completed all open checkboxes in `docs/roadmaps/gui/news.md` and `docs/roadmaps/gui/device.md`.
- Added control-plane coverage for new device/news operational knobs and persisted them through runtime GUI config sync.
