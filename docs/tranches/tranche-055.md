# Tranche 055

## Scope (30 items)
- [x] Add `caliberate-metadata` dependency to GUI crate for provider-backed metadata workflows
- [x] Add `reqwest` (blocking + rustls + json) dependency to metadata crate for HTTP provider calls
- [x] Add `serde` dependency to metadata crate for API response models
- [x] Add `serde_json` dependency to metadata crate for provider payload decoding support
- [x] Add new `online` module to metadata crate and export it from `crates/metadata/src/lib.rs`
- [x] Implement provider configuration model (`ProviderConfig`) in metadata online module
- [x] Implement metadata query model (`MetadataQuery`) for provider lookups
- [x] Implement downloaded metadata model (`DownloadedMetadata`) shared by providers and GUI apply logic
- [x] Implement cover download model (`CoverDownload`) for downloaded image bytes
- [x] Implement provider-agnostic metadata fetch entrypoint (`fetch_metadata`)
- [x] Implement OpenLibrary provider fetch path with response parsing and candidate mapping
- [x] Implement Google Books provider fetch path with response parsing and candidate mapping
- [x] Implement provider-agnostic cover download entrypoint (`fetch_cover`) with max-size enforcement
- [x] Add metadata online helper functions for provider enablement/default source detection
- [x] Add metadata online unit tests for tag dedupe and source fallback behavior
- [x] Add `[metadata_download]` configuration model to `ControlPlane`
- [x] Add `[metadata_download]` validation rules (timeouts, limits, providers, endpoints, user agent)
- [x] Add defaults for all `[metadata_download]` fields in core config
- [x] Add `[metadata_download]` section to `config/control-plane.toml`
- [x] Add `[metadata_download]` section to `crates/core/tests/fixtures/control-plane.toml`
- [x] Add `metadata_download_config` state to `LibraryView`
- [x] Initialize metadata download dialog defaults from control-plane configuration
- [x] Add metadata queue row model and status enum for per-book queued execution states
- [x] Implement queue actions in metadata dialog (`Queue selected`, `Run queue`, `Clear queue`, `Retry failed`)
- [x] Implement provider-backed per-book metadata fetch execution in GUI (`fetch_metadata_for_book`)
- [x] Replace placeholder metadata results list with structured provider results and per-book selection
- [x] Implement downloaded metadata apply path with merge/replace logic for title/authors/tags/publisher/language/pubdate/comment/identifiers
- [x] Implement cover apply from downloaded URL bytes (real fetch + decode + persistence) replacing placeholder-only flow
- [x] Update GUI metadata roadmap checkbox states for provider pipeline, queue statuses, retry workflow, and cover download/replace
- [x] Update config control-pane roadmap to track `[metadata_download]` section coverage

## Notes
- This tranche upgrades metadata download from UI placeholder behavior to provider-backed queue execution with per-book status and apply workflows.
- Amazon and ISBNdb remain listed in UI/config for parity surface but are intentionally not API-backed yet because they require additional integration/auth policy work.
