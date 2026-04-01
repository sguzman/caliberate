# CLI Inventory (Calibre Reference)

## Entry points
- `calibre/src/calibre/linux.py` registers console scripts for `calibredb`, `calibre-server`, and `ebook-convert`.
- `calibre/src/calibre/db/cli/main.py` is the main entry for `calibredb`.
- `calibre/src/calibre/srv/standalone.py` is the main entry for `calibre-server`.
- `calibre/src/calibre/ebooks/conversion/cli.py` is the main entry for `ebook-convert`.

## Command surface (parity targets)
- `calibredb`: library CRUD, search, list, add/remove formats, metadata updates, backups, FTS commands.
- `calibre-server`: server options, authentication, user management (`manage_users_cli.py`).
- `ebook-convert`: conversion options and format-specific flags.

Primary documentation sources:
- `calibre/manual/custom.py` (generated CLI help)
- `calibre/manual/server.rst`
- `calibre/manual/conversion.rst`
