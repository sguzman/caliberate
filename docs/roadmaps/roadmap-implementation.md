# Implementation Roadmap

- [x] Establish workspace crate structure with core modules (`config`, `logging`, `error`)
- [x] Implement config loading for `config/control-plane.toml` with schema validation and defaults
- [x] Add tracing-based logging bootstrap driven by config
- [x] Implement a minimal CLI entrypoint with `--config` override and a `check-config` command
- [x] Wire `main.rs` to call the CLI and config bootstrap
- [ ] Add smoke tests for config parsing and logging initialization
