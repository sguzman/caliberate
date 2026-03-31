# Config Control Pane Roadmap

- [x] Bootstrap `config/control-plane.toml` with initial sections and defaults
- [x] Define `[app]` keys: `name`, `environment`, `mode`, `instance_id`
- [x] Define `[paths]` keys: `data_dir`, `cache_dir`, `log_dir`, `tmp_dir`
- [x] Define `[logging]` keys: `level`, `json`, `stdout`, `file_enabled`, `file_max_size_mb`, `file_max_backups`
- [x] Define `[db]` keys: `sqlite_path`, `pool_size`, `busy_timeout_ms`
- [x] Define `[runtime]` keys: `worker_threads`, `max_blocking_threads`, `shutdown_timeout_ms`
- [x] Define `[metrics]` keys: `enabled`, `endpoint`, `namespace`
