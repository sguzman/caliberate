# DB Inventory (Calibre Reference)

## Schema files
- `calibre/resources/metadata_sqlite.sql`
- `calibre/resources/notes_sqlite.sql`
- `calibre/resources/fts_sqlite.sql`
- `calibre/resources/fts_triggers.sql`

## Schema upgrades
- `calibre/src/calibre/db/schema_upgrades.py`
- `calibre/src/calibre/db/fts/schema_upgrade.py`
- `calibre/src/calibre/db/notes/schema_upgrade.py`

## Core DB components
- `calibre/src/calibre/db/backend.py` (SQLite backend)
- `calibre/src/calibre/db/cache.py` (metadata cache)
- `calibre/src/calibre/db/tables.py` (table definitions)

Parity target: maintain schema compatibility with the SQL resources above, including versioned upgrades.
