# Config Control Pane Roadmap

- [x] Bootstrap `config/control-plane.toml` with initial sections and defaults
- [ ] Define `[app]` keys: `name`, `environment`, `instance_id`
- [ ] Define `[paths]` keys: `data_dir`, `cache_dir`, `log_dir`, `tmp_dir`
- [ ] Define `[logging]` keys: `level`, `json`, `stdout`, `file_enabled`, `file_max_size_mb`, `file_max_backups`
- [ ] Define `[db]` keys: `sqlite_path`, `pool_size`, `busy_timeout_ms`
- [ ] Define `[runtime]` keys: `worker_threads`, `max_blocking_threads`, `shutdown_timeout_ms`
