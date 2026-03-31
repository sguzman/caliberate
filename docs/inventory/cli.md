# CLI Inventory (Calibre Reference)

## Entry points
- `tmp/calibre/src/calibre/linux.py` registers console scripts for `calibredb`, `calibre-server`, and `ebook-convert`.
- `tmp/calibre/src/calibre/db/cli/main.py` is the main entry for `calibredb`.
- `tmp/calibre/src/calibre/srv/standalone.py` is the main entry for `calibre-server`.
- `tmp/calibre/src/calibre/ebooks/conversion/cli.py` is the main entry for `ebook-convert`.

## Command surface (parity targets)
- `calibredb`: library CRUD, search, list, add/remove formats, metadata updates, backups, FTS commands.
- `calibre-server`: server options, authentication, user management (`manage_users_cli.py`).
- `ebook-convert`: conversion options and format-specific flags.

Primary documentation sources:
- `tmp/calibre/manual/custom.py` (generated CLI help)
- `tmp/calibre/manual/server.rst`
- `tmp/calibre/manual/conversion.rst`
