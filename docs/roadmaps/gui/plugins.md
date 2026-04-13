# GUI Plugins Roadmap

## Plugin Manager
- [x] Add plugin list with enable/disable toggles
- [x] Add plugin install/remove workflow UI
- [x] Add plugin update check UI
- [x] Add plugin search/filter box
- [x] Add plugin details panel with version/author/description

## Plugin Settings
- [x] Add per-plugin settings panels
- [x] Add plugin error/status view
- [x] Add plugin dependency conflict warnings
- [x] Add plugin log viewer

## Parity Gap Backlog
- [ ] Replace in-memory default plugin list with filesystem-backed plugin registry loading from `plugins_dir`
- [ ] Persist plugin enable/disable state in control-plane and restore it on startup
- [ ] Implement plugin install from package file (`.zip`) with manifest validation
- [ ] Implement plugin uninstall that removes plugin artifacts from `plugins_dir`
- [ ] Add plugin metadata index client for update discovery (remote latest version checks)
- [ ] Implement dependency resolver with version constraints for install/update operations
- [ ] Add plugin startup failure isolation so one bad plugin does not abort GUI boot
- [ ] Add plugin compatibility checks against current app/plugin API version
