# Config Control Pane Roadmap

- [x] Bootstrap `config/control-plane.toml` with initial sections and defaults
- [x] Define `[app]` keys: `name`, `environment`, `mode`, `instance_id`
- [x] Define `[paths]` keys: `data_dir`, `cache_dir`, `log_dir`, `tmp_dir`, `library_dir`
- [x] Define `[logging]` keys: `level`, `json`, `stdout`, `file_enabled`, `file_max_size_mb`, `file_max_backups`
- [x] Define `[db]` keys: `sqlite_path`, `pool_size`, `busy_timeout_ms`
- [x] Define `[runtime]` keys: `worker_threads`, `max_blocking_threads`, `shutdown_timeout_ms`
- [x] Define `[metrics]` keys: `enabled`, `endpoint`, `namespace`
- [x] Define `[formats]` keys: `supported`, `archive_formats`
- [x] Define `[ingest]` keys: `default_mode`, `archive_reference_enabled`, `duplicate_policy`
- [x] Define `[assets]` keys: `compress_raw_assets`, `compress_metadata_db`
- [x] Define `[assets]` keys: `hash_algorithm`, `hash_on_ingest`, `verify_checksum`, `compression_level`
- [x] Define `[server]` keys: `host`, `port`, `url_prefix`, `enable_auth`
