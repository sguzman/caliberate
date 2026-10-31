# 0022 — Rich browser library metadata

## Goal
Make the existing HTML `/library` browsing surface useful for identifying editions before downloading, especially when titles are shared across languages.

## Scope
- Display existing cover availability through the existing `/api/v1/books/{id}/cover` route.
- Show the stored authors, language codes as recognizable names, publication date, publisher, series, rating, tags, format(s), and sizes when available.
- Explicitly label missing language instead of guessing.
- Preserve free-text search across title, author, series and tags, including paginated search results.
- Keep existing named download routes, OPDS and HTTP API response schemas stable.
- Respect configured server URL prefix; provide System/Light/Dark theme selection with System default.
- Do not invent translator or edition fields; no DB migrations or metadata scraping.

## Validation
- `cargo fmt --check`
- `cargo check --workspace --locked`
- `cargo test --workspace --locked`
- Browser HTTP regression tests for attached Calibre and managed sources.
- Human iPhone/Voice Dream smoke test after merge and local service restart.

## Current state
Implementation complete on the task branch, pending GitHub Actions verification and local runtime acceptance.
