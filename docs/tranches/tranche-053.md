# Tranche 053

## Scope (30 items)
- [x] Add `[network]` control-plane model to `ControlPlane`
- [x] Add `[server]` TLS config keys (`tls_enabled`, `tls_cert_path`, `tls_key_path`)
- [x] Add `[gui]` key `app_theme`
- [x] Add `[gui]` key `icon_set`
- [x] Add `[gui]` key `startup_open_last_library`
- [x] Add `[gui]` key `startup_restore_tabs`
- [x] Add `[gui]` key `last_active_view`
- [x] Add `[gui]` key `system_tray_mode`
- [x] Add `[gui]` key `confirm_exit_with_jobs`
- [x] Add default values for all new network/server/gui settings keys
- [x] Add config validation for `[network].dns_mode`
- [x] Add config validation for `gui.app_theme`, `gui.icon_set`, `gui.system_tray_mode`, `gui.last_active_view`
- [x] Add `[network]` section to `config/control-plane.toml`
- [x] Add `[network]` section to `crates/core/tests/fixtures/control-plane.toml`
- [x] Add server TLS keys to `config/control-plane.toml`
- [x] Add server TLS keys to `crates/core/tests/fixtures/control-plane.toml`
- [x] Add new GUI behavior keys to `config/control-plane.toml`
- [x] Add new GUI behavior keys to `crates/core/tests/fixtures/control-plane.toml`
- [x] Add preferences search box for filtering preference sections
- [x] Filter visible preference tabs based on preferences search query
- [x] Add `Reset section` action in preferences UI (edit mode)
- [x] Implement per-section reset-to-defaults behavior in preferences state
- [x] Add preferences export workflow (writes config TOML to path)
- [x] Add preferences import workflow (loads config TOML from path)
- [x] Add editable paths/database controls in System preferences section
- [x] Add logging verbosity combo control in System preferences section
- [x] Add proxy/network controls in System preferences section
- [x] Add TLS controls in Advanced server preferences panel
- [x] Add Look & Feel controls for theme/icon/startup/tray/confirm-exit with preview swatch
- [x] Wire startup/view behavior in app shell (`startup_open_last_library`, `startup_restore_tabs`, `last_active_view`) and apply app theme visuals

## Notes
- This tranche closes the currently defined `gui/settings` checkbox items except the broad top-level parity umbrella item, which remains open pending deeper Calibre preference-tree decomposition.
